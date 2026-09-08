//! Incremental candidate projection. Never read by production usage views.
use super::*;
use crate::source_union::{self, EvidenceSide};
use union_repository::{COUNTERPARTS, load_measurement};

const SOURCES: [(&str, &str); 2] = [
    ("sampling", "retained_request_evidence"),
    ("reconstruction", "reconstruction_usage_events"),
];

#[cfg(test)]
#[path = "union_projection_tests.rs"]
mod tests;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnionProjectionProgress {
    pub policy_version: u32,
    pub scope: &'static str,
    pub scanned_records: usize,
    pub recomputed_groups: usize,
    pub backfill_complete: bool,
    pub pending_groups: u64,
    pub selected_groups: u64,
    pub unresolved_groups: u64,
    pub projection_ready: bool,
    pub production_policy_changed: bool,
    pub history_complete: bool,
}

pub(super) fn migrate(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE measurement_union_counts (
            id INTEGER PRIMARY KEY CHECK(id=1), pending INTEGER NOT NULL CHECK(pending>=0),
            selected INTEGER NOT NULL CHECK(selected>=0), unresolved INTEGER NOT NULL CHECK(unresolved>=0)
        );
        INSERT INTO measurement_union_counts VALUES(1,0,0,0);
        CREATE TABLE measurement_union_backfill (
            evidence_source TEXT PRIMARY KEY, last_id TEXT, target_id TEXT,
            complete INTEGER NOT NULL CHECK(complete IN (0,1))
        ) WITHOUT ROWID;
        CREATE TABLE measurement_union_dirty (
            kind TEXT NOT NULL CHECK(kind IN ('key','sampling','reconstruction')),
            identity TEXT NOT NULL, PRIMARY KEY(kind,identity)
        ) WITHOUT ROWID;
        CREATE TABLE measurement_union_groups (
            kind TEXT NOT NULL, identity TEXT NOT NULL,
            member_count INTEGER NOT NULL CHECK(member_count>0),
            unresolved_reason TEXT,
            PRIMARY KEY(kind,identity)
        ) WITHOUT ROWID;
        CREATE TABLE measurement_union_selected (
            kind TEXT NOT NULL, identity TEXT NOT NULL,
            evidence_source TEXT NOT NULL, event_id TEXT NOT NULL,
            effective_at TEXT NOT NULL, thread_id TEXT NOT NULL,
            model TEXT, account_fingerprint TEXT, project_id TEXT,
            input_tokens INTEGER NOT NULL, cached_input_tokens INTEGER NOT NULL,
            cache_write_input_tokens INTEGER NOT NULL,
            cache_write_observed_input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL, reasoning_output_tokens INTEGER NOT NULL,
            total_tokens INTEGER NOT NULL,
            PRIMARY KEY(kind,identity),
            FOREIGN KEY(kind,identity) REFERENCES measurement_union_groups(kind,identity) ON DELETE CASCADE
        ) WITHOUT ROWID;
        CREATE INDEX measurement_union_selected_thread_time
            ON measurement_union_selected(thread_id,effective_at);
        CREATE INDEX measurement_union_selected_account_time
            ON measurement_union_selected(account_fingerprint,effective_at);
        CREATE INDEX measurement_union_selected_project_time
            ON measurement_union_selected(project_id,effective_at);
        CREATE INDEX measurement_union_selected_model_time
            ON measurement_union_selected(model,effective_at);",
    )?;
    for (table, counter, condition) in [
        ("measurement_union_dirty", "pending", "1"),
        ("measurement_union_selected", "selected", "1"),
        (
            "measurement_union_groups",
            "unresolved",
            "ROW.unresolved_reason IS NOT NULL",
        ),
    ] {
        for (operation, alias, delta) in [("INSERT", "NEW", "+1"), ("DELETE", "OLD", "-1")] {
            connection.execute_batch(&format!(
                "CREATE TRIGGER {table}_count_{operation} AFTER {operation} ON {table}
                 WHEN {} BEGIN UPDATE measurement_union_counts SET {counter}={counter}{delta} WHERE id=1; END;",
                condition.replace("ROW", alias)
            ))?;
        }
    }
    for (side, table) in SOURCES {
        // Index seek only: the migration never scans/imports historical facts.
        connection.execute(
            &format!("INSERT INTO measurement_union_backfill SELECT ?1,NULL,target,target IS NULL
                FROM (SELECT (SELECT event_id FROM {table} ORDER BY event_id DESC LIMIT 1) AS target)"),
            [side],
        )?;
        for target in if side == "sampling" {
            vec![table, "retained_request_assignments"]
        } else {
            vec![table]
        } {
            for operation in ["INSERT", "UPDATE", "DELETE"] {
                let aliases = match operation {
                    "UPDATE" => vec!["OLD", "NEW"],
                    "DELETE" => vec!["OLD"],
                    _ => vec!["NEW"],
                };
                let mut body = String::new();
                for alias in aliases {
                    body.push_str(&format!(
                        "INSERT INTO measurement_union_dirty
                        SELECT CASE WHEN p.record_key IS NULL OR p.record_key='' THEN '{side}' ELSE 'key' END,
                            CASE WHEN p.record_key IS NULL OR p.record_key='' THEN {alias}.event_id ELSE p.record_key END
                        FROM (SELECT 1) LEFT JOIN source_record_evidence p
                            ON p.evidence_source='{side}' AND p.event_id={alias}.event_id
                        WHERE 1 ON CONFLICT(kind,identity) DO NOTHING;"
                    ));
                }
                connection.execute_batch(&format!(
                    "CREATE TRIGGER measurement_union_{target}_{operation}
                        AFTER {operation} ON {target} BEGIN {body} END;"
                ))?;
            }
        }
    }
    // Rekeying must invalidate BOTH the old and the new groups, including a
    // previously unkeyed singleton. Source evidence is otherwise untouched.
    for operation in ["INSERT", "UPDATE", "DELETE"] {
        let aliases = match operation {
            "UPDATE" => vec!["OLD", "NEW"],
            "DELETE" => vec!["OLD"],
            _ => vec!["NEW"],
        };
        let mut body = String::new();
        for alias in aliases {
            body.push_str(&format!(
                "INSERT INTO measurement_union_dirty VALUES({alias}.evidence_source,{alias}.event_id) ON CONFLICT(kind,identity) DO NOTHING;
                 INSERT INTO measurement_union_dirty SELECT 'key',{alias}.record_key WHERE {alias}.record_key<>'' ON CONFLICT(kind,identity) DO NOTHING;"
            ));
        }
        connection.execute_batch(&format!(
            "CREATE TRIGGER measurement_union_key_{operation}
                AFTER {operation} ON source_record_evidence BEGIN {body} END;"
        ))?;
    }
    Ok(())
}

impl LedgerStore {
    /// Explicit staging write. No ingestion, raw compaction or active-policy switch.
    /// Limits include all counterparts, without filtering away account conflicts.
    pub fn stage_source_union_batch(
        &mut self,
        scan_limit: usize,
        group_limit: usize,
        member_limit: usize,
    ) -> StoreResult<UnionProjectionProgress> {
        if !(1..=1000).contains(&scan_limit)
            || !(1..=1000).contains(&group_limit)
            || !(1..=10000).contains(&member_limit)
        {
            return Err(StoreError::InvalidRequestQuery(
                "union scan/group limits must be 1..1000; member limit 1..10000",
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut scanned = 0;
        for (side, table) in SOURCES {
            if scanned == scan_limit {
                break;
            }
            scanned += backfill(&transaction, side, table, scan_limit - scanned)?;
        }
        let dirty = transaction
            .prepare_cached(
                "SELECT kind,identity FROM measurement_union_dirty ORDER BY kind,identity LIMIT ?1",
            )?
            .query_map([group_limit as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut recomputed = 0;
        let mut members = 0;
        for (kind, identity) in &dirty {
            if members == member_limit {
                break;
            }
            let next = match recompute(&transaction, kind, identity, member_limit - members) {
                Ok(count) => count,
                // Finish completed groups; retry the intact larger group with a
                // full budget next time. An oversized first group fails atomically.
                Err(StoreError::UnionLimit) if recomputed > 0 => break,
                Err(error) => return Err(error),
            };
            members += next;
            recomputed += 1;
            transaction.execute(
                "DELETE FROM measurement_union_dirty WHERE kind=?1 AND identity=?2",
                params![kind, identity],
            )?;
        }
        // Status belongs to the same snapshot as the cursor and rows committed.
        let result = progress(&transaction, scanned, recomputed)?;
        transaction.commit()?;
        Ok(result)
    }

    /// Diagnostic metadata, not a claim that all history/request identities exist.
    pub fn source_union_projection_progress(&self) -> StoreResult<UnionProjectionProgress> {
        let transaction = self.connection.unchecked_transaction()?;
        let result = progress(&transaction, 0, 0)?;
        transaction.commit()?;
        Ok(result)
    }
}

fn backfill(connection: &Connection, side: &str, table: &str, limit: usize) -> StoreResult<usize> {
    let (last, target, complete): (Option<String>,Option<String>,bool) = connection.query_row(
        "SELECT last_id,target_id,complete FROM measurement_union_backfill WHERE evidence_source=?1",
        [side], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))
    )?;
    if complete {
        return Ok(0);
    }
    let target = target.ok_or(StoreError::InvalidRequestQuery(
        "invalid union backfill target",
    ))?;
    // Separate seek forms keep nullable first-page logic out of the range predicate.
    let (sql, parameters) = if let Some(last) = last {
        (
            format!(
                "SELECT event_id FROM {table} WHERE event_id>?1 AND event_id<=?2 ORDER BY event_id LIMIT ?3"
            ),
            vec![
                SqlValue::Text(last),
                SqlValue::Text(target),
                SqlValue::Integer((limit + 1) as i64),
            ],
        )
    } else {
        (
            format!("SELECT event_id FROM {table} WHERE event_id<=?1 ORDER BY event_id LIMIT ?2"),
            vec![
                SqlValue::Text(target),
                SqlValue::Integer((limit + 1) as i64),
            ],
        )
    };
    let ids = connection
        .prepare(&sql)?
        .query_map(params_from_iter(parameters), |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for id in ids.iter().take(limit) {
        connection.execute(
            "INSERT OR IGNORE INTO measurement_union_dirty
             SELECT CASE WHEN p.record_key IS NULL OR p.record_key='' THEN ?1 ELSE 'key' END,
                    CASE WHEN p.record_key IS NULL OR p.record_key='' THEN ?2 ELSE p.record_key END
             FROM (SELECT 1) LEFT JOIN source_record_evidence p ON p.evidence_source=?1 AND p.event_id=?2",
            params![side,id]
        )?;
    }
    connection.execute(
        "UPDATE measurement_union_backfill SET last_id=COALESCE(?2,last_id),complete=?3 WHERE evidence_source=?1",
        params![side,ids.iter().take(limit).next_back(),ids.len()<=limit]
    )?;
    Ok(ids.len().min(limit))
}

fn recompute(
    connection: &Connection,
    kind: &str,
    identity: &str,
    limit: usize,
) -> StoreResult<usize> {
    let members: Vec<(EvidenceSide, String)> = if kind == "key" {
        connection
            .prepare_cached(COUNTERPARTS)?
            .query_map(params![identity, (limit + 1) as i64], |row| {
                Ok((
                    if row.get::<_, String>(0)? == "sampling" {
                        EvidenceSide::Sampling
                    } else {
                        EvidenceSide::Reconstruction
                    },
                    row.get(1)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let (side, table) = if kind == "sampling" {
            (EvidenceSide::Sampling, SOURCES[0].1)
        } else {
            (EvidenceSide::Reconstruction, SOURCES[1].1)
        };
        // Match scoped-query eligibility: unkeyed unknown observations remain
        // visible as unknown, but are not candidates for confirmed usage.
        let eligibility = if side == EvidenceSide::Sampling {
            " AND e.quality='confirmed'"
        } else {
            ""
        };
        let exists: bool = connection.query_row(&format!(
            "SELECT EXISTS(SELECT 1 FROM {table} e LEFT JOIN source_record_evidence p
             ON p.evidence_source=?1 AND p.event_id=e.event_id WHERE e.event_id=?2 AND (p.record_key IS NULL OR p.record_key=''){eligibility})"
        ),params![kind,identity],|row| row.get(0))?;
        if exists {
            vec![(side, identity.into())]
        } else {
            vec![]
        }
    };
    if members.len() > limit {
        return Err(StoreError::UnionLimit);
    }
    let records = members
        .iter()
        .map(|(side, id)| load_measurement(connection, *side, id))
        .collect::<StoreResult<Vec<_>>>()?;
    let report = source_union::plan(records, DateTime::<Utc>::MIN_UTC, DateTime::<Utc>::MAX_UTC)?;
    // Never discard a valid observation because an artificial diagnostic bound
    // happens to coincide with its canonical timestamp.
    if report.canonical_records_outside_window != 0 {
        return Err(StoreError::InvalidRequestQuery(
            "union timestamp outside supported projection range",
        ));
    }
    connection.execute(
        "DELETE FROM measurement_union_selected WHERE kind=?1 AND identity=?2",
        params![kind, identity],
    )?;
    connection.execute(
        "DELETE FROM measurement_union_groups WHERE kind=?1 AND identity=?2",
        params![kind, identity],
    )?;
    if members.is_empty() {
        return Ok(0);
    }
    let reason = report
        .unresolved
        .first()
        .map(|group| serde_json::to_value(&group.reason))
        .transpose()?
        .and_then(|value| value.as_str().map(str::to_owned));
    connection.execute(
        "INSERT INTO measurement_union_groups VALUES(?1,?2,?3,?4)",
        params![kind, identity, members.len() as i64, reason],
    )?;
    if let Some(selected) = report.selected.first() {
        let usage = selected
            .usage
            .expect("union planner validated selected usage");
        connection.execute(
            "INSERT INTO measurement_union_selected VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            params![kind,identity,if selected.side==EvidenceSide::Sampling {"sampling"} else {"reconstruction"},
                selected.id,timestamp(selected.at),selected.thread,selected.model,selected.account,selected.project,
                sql_u64(usage.input_tokens)?,sql_u64(usage.cached_input_tokens)?,sql_u64(usage.cache_write_input_tokens)?,
                sql_u64(usage.cache_write_observed_input_tokens)?,sql_u64(usage.output_tokens)?,
                sql_u64(usage.reasoning_output_tokens)?,sql_u64(usage.total_tokens)?]
        )?;
    }
    Ok(members.len())
}

fn sql_u64(value: u64) -> StoreResult<i64> {
    i64::try_from(value).map_err(|_| StoreError::IntegerOverflow {
        field: "measurement union token",
    })
}

pub(super) fn progress(
    connection: &Connection,
    scanned: usize,
    recomputed: usize,
) -> StoreResult<UnionProjectionProgress> {
    let backfill_complete: bool = connection.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM measurement_union_backfill WHERE complete=0)",
        [],
        |row| row.get(0),
    )?;
    let (pending, selected, unresolved): (i64, i64, i64) = connection.query_row(
        "SELECT pending,selected,unresolved FROM measurement_union_counts WHERE id=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    Ok(UnionProjectionProgress {
        policy_version: connection.query_row(
            "SELECT policy_version FROM measurement_union_counts WHERE id=1",
            [],
            |row| row.get(0),
        )?,
        scope: "staged_local_measurements_not_inference_usage",
        scanned_records: scanned,
        recomputed_groups: recomputed,
        backfill_complete,
        pending_groups: pending as u64,
        selected_groups: selected as u64,
        unresolved_groups: unresolved as u64,
        projection_ready: backfill_complete && pending == 0,
        production_policy_changed: false,
        history_complete: false,
    })
}
