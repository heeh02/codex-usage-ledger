//! Re-run the live association policy using persisted sampling anchors.
//! No guessed value-based join, account reassignment or historical write.
use super::*;
use crate::store::RetainedRequestObservation;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    records: usize,
    proposed_records: usize,
    write_coverage_differences: usize,
    stored_usage: TokenUsage,
    proposed_usage: Option<TokenUsage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub(crate) event_id: String,
    pub(crate) at: DateTime<Utc>,
    pub(crate) status: &'static str,
    pub(crate) byte_offset: Option<u64>,
    pub(crate) record_digest: Option<String>,
    pub(crate) stored_usage: TokenUsage,
    proposed_usage: Option<TokenUsage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    policy: &'static str,
    thread: String,
    scope_start: DateTime<Utc>,
    scope_end: DateTime<Utc>,
    pub(crate) source_identity: String,
    bytes_read: u64,
    pub(crate) source_changed: bool,
    pub(crate) source_extended: bool,
    loaded_anchors: usize,
    loaded_candidates: usize,
    groups: BTreeMap<&'static str, Group>,
    pub(crate) links: Option<Vec<Link>>,
    history_complete: bool,
    migration_authorized: bool,
}

#[derive(Clone, Copy)]
pub struct LegacySamplingAuditOptions {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub limit: usize,
    pub max_bytes: u64,
    pub include_links: bool,
}

pub fn audit_legacy_sampling(
    db: &Path,
    home: &Path,
    thread: &str,
    options: LegacySamplingAuditOptions,
) -> Result<Report> {
    let LegacySamplingAuditOptions {
        start,
        end,
        limit,
        max_bytes,
        include_links,
    } = options;
    if thread.is_empty()
        || start >= end
        || !(1..=100_000).contains(&limit)
        || !(1..=1_073_741_824).contains(&max_bytes)
    {
        return Err(anyhow!(
            "require thread, ordered interval, limit 1..100000 and byte cap 1..1073741824"
        ));
    }
    let before = start
        .checked_sub_signed(chrono::Duration::milliseconds(500))
        .ok_or_else(|| anyhow!("unsupported date"))?;
    let after = end
        .checked_add_signed(chrono::Duration::milliseconds(500))
        .ok_or_else(|| anyhow!("unsupported date"))?;
    let store = LedgerStore::open_read_only(db)?;
    let anchors = store.with_source_audit_snapshot(|store| -> Result<_> {
        let mut anchors = Vec::new();
        let mut cursor = None;
        loop {
            let page = store.retained_request_page(thread, before, after, cursor.as_ref(), 500)?;
            anchors.extend(page.observations);
            if anchors.len() > limit {
                return Err(anyhow!("sampling context exceeds limit; narrow interval"));
            }
            cursor = page.next;
            if cursor.is_none() {
                break;
            }
        }
        Ok(anchors)
    })?;
    let (path, is_child) = crate::reconstruction::indexed_rollout_for_sampling_audit(home, thread)?;
    let metadata = path.metadata()?;
    if metadata.len() > max_bytes {
        return Err(anyhow!("rollout exceeds explicit byte budget"));
    }
    let identity = physical_file_identity(&path, &metadata)?;
    let mut counter = CandidateCounterCheckpoint::new(0);
    let mut bytes = 0;
    let (mut candidates, _) = read_usage_candidates_bounded(
        &path,
        0,
        &mut bytes,
        CandidateReadWindow {
            safe_before: after,
            max_bytes: Some(max_bytes),
            max_candidates: Some(limit),
        },
        &mut counter,
        thread,
        is_child,
    )?;
    if candidates.len() > limit {
        return Err(anyhow!("rollout candidate prefix exceeds limit"));
    }
    // Prefix records establish counter/replay state, but only nearby records
    // participate in association. Invalid/replayed/unchanged candidates remain
    // in this window so skipping them cannot manufacture a farther match.
    candidates
        .retain(|candidate| candidate.observed_at >= before && candidate.observed_at <= after);
    candidates.sort_by_key(|candidate| (candidate.observed_at, candidate.byte_offset));
    let current = path.metadata()?;
    let changed = metadata.len() != current.len()
        || metadata.modified()? != current.modified()?
        || identity != physical_file_identity(&path, &current)?;
    let mut report = compare(&anchors, &candidates, thread, start, end, include_links)?;
    report.source_identity = identity;
    report.source_changed = changed;
    report.source_extended = current.len() > metadata.len()
        && report.source_identity == physical_file_identity(&path, &current)?;
    report.bytes_read = bytes;
    Ok(report)
}

fn compare(
    anchors: &[RetainedRequestObservation],
    candidates: &[UsageCandidate],
    thread: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    include_links: bool,
) -> Result<Report> {
    let times = anchors
        .iter()
        .map(|row| {
            DateTime::parse_from_rfc3339(&row.cursor.effective_at)
                .map(|at| timestamp_nanos(at.with_timezone(&Utc)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let candidate_times = candidates
        .iter()
        .map(|row| timestamp_nanos(row.observed_at))
        .collect::<Vec<_>>();
    let matches = mutual_matches(&times, &candidate_times);
    let mut groups = BTreeMap::<&'static str, Group>::new();
    let mut links = Vec::new();
    for (row, matched) in anchors.iter().zip(matches) {
        let at = DateTime::parse_from_rfc3339(&row.cursor.effective_at)?.with_timezone(&Utc);
        if at < start || at >= end {
            continue;
        }
        let candidate = if let Nearest::Unique(index) = matched {
            Some(&candidates[index])
        } else {
            None
        };
        let status = if !row
            .cursor
            .event_id
            .strip_prefix("logs2-post-sampling:")
            .is_some_and(|suffix| {
                suffix
                    .parse::<u64>()
                    .is_ok_and(|id| id > 0 && id.to_string() == suffix)
            }) {
            "unsupported_legacy_namespace"
        } else if row.quality != DataQuality::Confirmed {
            "stored_unconfirmed"
        } else {
            match (matched, candidate.and_then(|row| row.usage)) {
                (Nearest::Missing, _) => "no_candidate",
                (Nearest::Ambiguous, _) => "ambiguous",
                (_, None) => "candidate_unavailable",
                (_, Some(usage)) if crate::source_union::same_token_amounts(row.usage, usage) => {
                    "amounts_match"
                }
                (_, Some(_)) => "amounts_changed",
            }
        };
        let proposed = candidate.and_then(|candidate| candidate.usage);
        let group = groups.entry(status).or_default();
        group.records += 1;
        add(&mut group.stored_usage, row.usage)?;
        if let Some(usage) = proposed {
            group.proposed_records += 1;
            group.write_coverage_differences += usize::from(
                usage.cache_write_observed_input_tokens
                    != row.usage.cache_write_observed_input_tokens,
            );
            add(group.proposed_usage.get_or_insert_default(), usage)?;
        }
        if include_links {
            links.push(Link {
                event_id: row.cursor.event_id.clone(),
                at,
                status,
                byte_offset: candidate.map(|row| row.byte_offset),
                record_digest: candidate.map(|row| row.record_digest.clone()),
                stored_usage: row.usage,
                proposed_usage: proposed,
            });
        }
    }
    Ok(Report {
        policy: "retained_anchor_requalification_v1",
        thread: thread.into(),
        scope_start: start,
        scope_end: end,
        source_identity: String::new(),
        bytes_read: 0,
        source_changed: false,
        source_extended: false,
        loaded_anchors: anchors.len(),
        loaded_candidates: candidates.len(),
        groups,
        links: include_links.then_some(links),
        history_complete: false,
        migration_authorized: false,
    })
}

fn add(total: &mut TokenUsage, usage: TokenUsage) -> Result<()> {
    crate::source_union::add(total, usage)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn usage(total: u64) -> TokenUsage {
        TokenUsage {
            input_tokens: total,
            total_tokens: total,
            ..Default::default()
        }
    }

    #[test]
    fn archived_anchors_requalify_without_original_log_db_and_do_not_rewrite_facts() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::create_dir(home.join("sessions")).unwrap();
        let rollout = home.join("sessions/root.jsonl");
        let mut out = File::create(&rollout).unwrap();
        let start: DateTime<Utc> = "2026-01-01T00:00:00Z".parse().unwrap();
        writeln!(
            out,
            "{}",
            serde_json::json!({"type":"session_meta","timestamp":start,"payload":{"id":"root"}})
        )
        .unwrap();
        let db = home.join("ledger.sqlite3");
        let mut store = LedgerStore::open(&db).unwrap();
        for (id, seconds, total) in [(1, 10, 100), (2, 20, 150), (3, 21, 150)] {
            let at = start + chrono::Duration::seconds(seconds);
            writeln!(out,"{}",serde_json::json!({"type":"event_msg","timestamp":at,"payload":{"type":"token_count","info":{"total_token_usage":usage(total),"last_token_usage":usage(100)}}})).unwrap();
            let row = event_from_observation(
                Observation {
                    anchor_key: format!("synthetic-{id}"),
                    receipt_key: None,
                    log_id: id,
                    observed_at: at,
                    thread_id: "root".into(),
                    turn_id: None,
                    model: None,
                },
                "root",
                &ThreadInfo::default(),
                "synthetic-machine",
                usage(100),
                DataQuality::Confirmed,
                None,
                &[],
                POST_SAMPLING_SOURCE_ID,
                None,
            );
            store.upsert_event(&row).unwrap();
        }
        drop(store);
        drop(out);
        let index = Connection::open(home.join("state_5.sqlite")).unwrap();
        index
            .execute_batch(
                "CREATE TABLE threads(id TEXT,rollout_path TEXT,cwd TEXT,model TEXT,source TEXT)",
            )
            .unwrap();
        index
            .execute(
                "INSERT INTO threads VALUES('root',?1,NULL,NULL,NULL)",
                [rollout.to_str().unwrap()],
            )
            .unwrap();
        drop(index);
        let original = std::fs::read(&db).unwrap();
        let source = std::fs::read(&rollout).unwrap();
        let report = audit_legacy_sampling(
            &db,
            home,
            "root",
            LegacySamplingAuditOptions {
                start,
                end: start + chrono::Duration::days(1),
                limit: 100,
                max_bytes: 10000,
                include_links: true,
            },
        )
        .unwrap();
        assert_eq!(report.groups["amounts_match"].records, 1);
        assert_eq!(
            report.groups["amounts_changed"]
                .proposed_usage
                .unwrap()
                .total_tokens,
            50
        );
        assert_eq!(report.groups["candidate_unavailable"].records, 1);
        assert!(!report.source_changed);
        assert!(!report.migration_authorized);
        assert_eq!(std::fs::read(&db).unwrap(), original);
        assert_eq!(std::fs::read(&rollout).unwrap(), source);
        assert!(!home.join("logs_2.sqlite").exists());
        assert!(
            audit_legacy_sampling(
                &db,
                home,
                "root",
                LegacySamplingAuditOptions {
                    start,
                    end: start + chrono::Duration::days(1),
                    limit: 1,
                    max_bytes: 10000,
                    include_links: false
                }
            )
            .is_err()
        );
        assert!(
            audit_legacy_sampling(
                &db,
                home,
                "root",
                LegacySamplingAuditOptions {
                    start,
                    end: start + chrono::Duration::days(1),
                    limit: 100,
                    max_bytes: 1,
                    include_links: false
                }
            )
            .is_err()
        );
    }

    #[test]
    fn unknown_competitors_and_invalid_nearest_candidates_cannot_be_skipped() {
        let at: DateTime<Utc> = "2026-01-01T00:00:00Z".parse().unwrap();
        let anchor = |id: u64, ms: i64, quality| RetainedRequestObservation {
            cursor: crate::store::RetainedRequestCursor {
                event_id: format!("logs2-post-sampling:{id}"),
                effective_at: (at + chrono::Duration::milliseconds(ms)).to_rfc3339(),
            },
            turn_id: None,
            model: None,
            observed_account: None,
            observed_project: None,
            observed_account_confidence: AttributionConfidence::Unknown,
            observed_project_confidence: AttributionConfidence::Unknown,
            quality,
            usage: usage(100),
        };
        let candidate = |ms: i64, amount| UsageCandidate {
            observed_at: at + chrono::Duration::milliseconds(ms),
            byte_offset: ms as u64,
            record_digest: "synthetic".into(),
            usage: amount,
            unavailable_reason: None,
            claimed: false,
        };
        let report = compare(
            &[
                anchor(1, 0, DataQuality::Confirmed),
                anchor(2, 100, DataQuality::Unknown),
            ],
            &[candidate(90, Some(usage(100)))],
            "root",
            at,
            at + chrono::Duration::seconds(1),
            true,
        )
        .unwrap();
        assert_eq!(report.groups["ambiguous"].records, 1);
        let report = compare(
            &[anchor(1, 0, DataQuality::Confirmed)],
            &[candidate(10, None), candidate(100, Some(usage(100)))],
            "root",
            at,
            at + chrono::Duration::seconds(1),
            true,
        )
        .unwrap();
        assert_eq!(report.groups["candidate_unavailable"].records, 1);
        assert_eq!(report.links.unwrap()[0].byte_offset, Some(10));
    }
}
