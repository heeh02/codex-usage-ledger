use super::*;
use crate::source_union::{self, EvidenceSide, Measurement, UnionShadow};
use std::collections::BTreeSet;

const SAMPLING_SEED: &str = "SELECT event_id FROM retained_request_evidence
    WHERE thread_id=?1 AND effective_at>=?2 AND effective_at<?3 LIMIT ?4";
const RECONSTRUCTION_SEED: &str = "SELECT event_id FROM reconstruction_usage_events
    WHERE thread_id=?1 AND COALESCE(source_timestamp,observed_at)>=?2
      AND COALESCE(source_timestamp,observed_at)<?3 LIMIT ?4";
pub(super) const COUNTERPARTS: &str = "SELECT p.evidence_source,p.event_id FROM source_record_evidence p
    WHERE p.record_key=?1 AND (
      (p.evidence_source='sampling' AND EXISTS(SELECT 1 FROM retained_request_evidence k WHERE k.event_id=p.event_id)) OR
      (p.evidence_source='reconstruction' AND EXISTS(SELECT 1 FROM reconstruction_usage_events r WHERE r.event_id=p.event_id)))
    LIMIT ?2";

impl LedgerStore {
    /// Work inventory for identity repair, not another full history scan.
    pub(crate) fn missing_identity_threads(&self) -> StoreResult<BTreeSet<String>> {
        let mut query = self.connection.prepare(
            "SELECT r.thread_id FROM reconstruction_usage_events r
             LEFT JOIN source_record_evidence p ON p.evidence_source='reconstruction' AND p.event_id=r.event_id
             WHERE COALESCE(p.record_key,'')='' AND r.thread_id IS NOT NULL AND r.thread_id<>''
             UNION SELECT k.thread_id FROM retained_request_evidence k
             LEFT JOIN source_record_evidence p ON p.evidence_source='sampling' AND p.event_id=k.event_id
             WHERE k.quality='confirmed' AND COALESCE(p.record_key,'')=''
               AND k.thread_id IS NOT NULL AND k.thread_id<>''")?;
        Ok(query
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    /// One snapshot: indexed seeds then direct key closure before scope resolution.
    pub fn shadow_source_union(
        &self,
        thread: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: usize,
    ) -> StoreResult<UnionShadow> {
        if thread.is_empty() || start >= end || !(1..=10000).contains(&limit) {
            return Err(StoreError::InvalidRequestQuery(
                "require thread, ordered dates and limit 1..10000",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let mut seeds = Vec::new();
        for (side, sql) in [
            (EvidenceSide::Sampling, SAMPLING_SEED),
            (EvidenceSide::Reconstruction, RECONSTRUCTION_SEED),
        ] {
            let ids = transaction
                .prepare_cached(sql)?
                .query_map(
                    params![
                        thread,
                        timestamp(start),
                        timestamp(end),
                        (limit - seeds.len() + 1) as i64
                    ],
                    |row| row.get::<_, String>(0),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            seeds.extend(ids.into_iter().map(|id| (side, id)));
            if seeds.len() > limit {
                return Err(StoreError::UnionLimit);
            }
        }
        let mut rows = Vec::new();
        let mut visited = BTreeSet::new();
        let mut expanded = BTreeSet::new();
        for identity in seeds {
            if visited.contains(&identity) {
                continue;
            }
            let observation = load_measurement(&transaction, identity.0, &identity.1)?;
            let key = observation.record_key.clone();
            visited.insert(identity);
            rows.push(observation);
            if rows.len() > limit {
                return Err(StoreError::UnionLimit);
            }
            if let Some(key) = key.filter(|key| expanded.insert(key.clone())) {
                // Some members may already be visited: cap this lookup at the
                // total limit, rather than the remaining slots alone.
                let counterparts = transaction
                    .prepare_cached(COUNTERPARTS)?
                    .query_map(params![key, (limit + 1) as i64], |row| {
                        Ok((
                            if row.get::<_, String>(0)? == "sampling" {
                                EvidenceSide::Sampling
                            } else {
                                EvidenceSide::Reconstruction
                            },
                            row.get::<_, String>(1)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                if counterparts.len() > limit {
                    return Err(StoreError::UnionLimit);
                }
                for counterpart in counterparts {
                    if !visited.insert(counterpart.clone()) {
                        continue;
                    }
                    if rows.len() == limit {
                        return Err(StoreError::UnionLimit);
                    }
                    rows.push(load_measurement(
                        &transaction,
                        counterpart.0,
                        &counterpart.1,
                    )?);
                }
            }
        }
        let report = source_union::plan(rows, start, end)?;
        transaction.commit()?;
        Ok(report)
    }
}

pub(super) fn load_measurement(
    connection: &Connection,
    side: EvidenceSide,
    id: &str,
) -> StoreResult<Measurement> {
    let sql = match side {
        EvidenceSide::Sampling => "SELECT k.effective_at,k.thread_id,k.model,a.account_fingerprint,a.project_id,a.event_id IS NOT NULL,
            k.quality='confirmed',k.input_tokens,k.cached_input_tokens,k.cache_write_input_tokens,k.cache_write_observed_input_tokens,
            k.output_tokens,k.reasoning_output_tokens,k.total_tokens,p.record_key
            FROM retained_request_evidence k LEFT JOIN retained_request_assignments a USING(event_id)
            LEFT JOIN source_record_evidence p ON p.event_id=k.event_id AND p.evidence_source='sampling' WHERE k.event_id=?1",
        EvidenceSide::Reconstruction => "SELECT COALESCE(r.source_timestamp,r.observed_at),r.thread_id,r.model,r.account_fingerprint,r.project_id,1,1,
            r.input_tokens,r.cached_input_tokens,r.cache_write_input_tokens,r.cache_write_observed_input_tokens,r.output_tokens,
            r.reasoning_output_tokens,r.total_tokens,p.record_key FROM reconstruction_usage_events r
            LEFT JOIN source_record_evidence p ON p.event_id=r.event_id AND p.evidence_source='reconstruction' WHERE r.event_id=?1",
    };
    Ok(connection.prepare_cached(sql)?.query_row([id], |row| {
        Ok(Measurement {
            side,
            id: id.to_owned(),
            at: parse_timestamp_column(row.get(0)?, 0)?,
            thread: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            model: row.get(2)?,
            account: row.get(3)?,
            project: row.get(4)?,
            assignment_available: row.get(5)?,
            record_key: row.get(14)?,
            usage: if row.get::<_, bool>(6)? {
                Some(TokenUsage {
                    input_tokens: u64_from_sql(row.get(7)?, 7)?,
                    cached_input_tokens: u64_from_sql(row.get(8)?, 8)?,
                    cache_write_input_tokens: u64_from_sql(row.get(9)?, 9)?,
                    cache_write_observed_input_tokens: u64_from_sql(row.get(10)?, 10)?,
                    output_tokens: u64_from_sql(row.get(11)?, 11)?,
                    reasoning_output_tokens: u64_from_sql(row.get(12)?, 12)?,
                    total_tokens: u64_from_sql(row.get(13)?, 13)?,
                })
            } else {
                None
            },
        })
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::StatementStatus;

    #[test]
    fn seed_and_counterpart_queries_seek_without_scanning_unrelated_history() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE retained_request_evidence(event_id TEXT PRIMARY KEY,thread_id TEXT,effective_at TEXT);
            CREATE INDEX retained_request_thread_time_idx ON retained_request_evidence(thread_id,effective_at,event_id);
            CREATE TABLE reconstruction_usage_events(event_id TEXT PRIMARY KEY,thread_id TEXT,source_timestamp TEXT,observed_at TEXT);
            CREATE INDEX reconstruction_events_thread_effective_time_idx ON reconstruction_usage_events(thread_id,COALESCE(source_timestamp,observed_at));
            CREATE TABLE source_record_evidence(evidence_source TEXT,event_id TEXT,record_key TEXT,PRIMARY KEY(evidence_source,event_id)) WITHOUT ROWID;
            CREATE INDEX source_record_evidence_key ON source_record_evidence(record_key,evidence_source,event_id);
            WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<100000)
            INSERT INTO retained_request_evidence SELECT 'unrelated-'||i,'other','2026-01-01T00:00:00Z' FROM n;
            INSERT INTO reconstruction_usage_events SELECT event_id,thread_id,effective_at,effective_at FROM retained_request_evidence;
            INSERT INTO source_record_evidence SELECT 'sampling',event_id,event_id FROM retained_request_evidence;
            INSERT INTO retained_request_evidence VALUES('seed','target','2026-01-01T00:00:00Z');
            INSERT INTO reconstruction_usage_events VALUES('peer','target',NULL,'2026-01-01T00:00:00Z');
            INSERT INTO source_record_evidence VALUES('sampling','seed','shared'),('reconstruction','peer','shared');").unwrap();
        for sql in [SAMPLING_SEED, RECONSTRUCTION_SEED] {
            let mut statement = connection.prepare(sql).unwrap();
            let found = statement
                .query_map(params!["target", "2025-12-31", "2026-01-02", 2], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(found.len(), 1);
            assert_eq!(statement.get_status(StatementStatus::FullscanStep), 0);
            assert!(statement.get_status(StatementStatus::VmStep) < 100);
        }
        let mut statement = connection.prepare(COUNTERPARTS).unwrap();
        let found = statement
            .query_map(params!["shared", 3], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(statement.get_status(StatementStatus::FullscanStep), 0);
        assert!(statement.get_status(StatementStatus::VmStep) < 150);
    }
}
