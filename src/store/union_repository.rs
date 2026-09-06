use super::*;
use crate::source_union::{self, EvidenceSide, Measurement, UnionShadow};

impl LedgerStore {
    /// Bounded read-only shadow. Load counterpart closure before planning so
    /// a boundary or attribution difference cannot hide one half of a pair.
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
        let mut statement = transaction.prepare(
            "WITH observations AS (
                SELECT 'sampling' side,k.event_id,k.effective_at at,k.thread_id,k.model,
                    a.account_fingerprint account,a.project_id project,a.event_id IS NOT NULL assigned,
                    k.quality,k.input_tokens,k.cached_input_tokens,k.cache_write_input_tokens,
                    k.cache_write_observed_input_tokens,k.output_tokens,k.reasoning_output_tokens,k.total_tokens,
                    p.record_key
                FROM retained_request_evidence k
                LEFT JOIN retained_request_assignments a USING(event_id)
                LEFT JOIN source_record_evidence p ON p.event_id=k.event_id AND p.evidence_source='sampling'
                UNION ALL
                SELECT 'reconstruction',r.event_id,COALESCE(r.source_timestamp,r.observed_at),r.thread_id,r.model,
                    r.account_fingerprint,r.project_id,1,'confirmed',r.input_tokens,r.cached_input_tokens,
                    r.cache_write_input_tokens,r.cache_write_observed_input_tokens,r.output_tokens,
                    r.reasoning_output_tokens,r.total_tokens,p.record_key
                FROM reconstruction_usage_events r
                LEFT JOIN source_record_evidence p ON p.event_id=r.event_id AND p.evidence_source='reconstruction'
             ), seed AS (
                SELECT * FROM observations WHERE thread_id=?1 AND at>=?2 AND at<?3
             ) SELECT * FROM observations
               WHERE (thread_id=?1 AND at>=?2 AND at<?3)
                  OR record_key IN (SELECT record_key FROM seed WHERE record_key IS NOT NULL)
               ORDER BY at,side,event_id LIMIT ?4"
        )?;
        let rows = statement
            .query_map(
                params![thread, timestamp(start), timestamp(end), (limit + 1) as i64],
                |row| {
                    let at: String = row.get(2)?;
                    let confirmed = row.get::<_, String>(8)? == "confirmed";
                    Ok(Measurement {
                        side: if row.get::<_, String>(0)? == "sampling" {
                            EvidenceSide::Sampling
                        } else {
                            EvidenceSide::Reconstruction
                        },
                        id: row.get(1)?,
                        at: parse_timestamp_column(at, 2)?,
                        thread: row.get(3)?,
                        model: row.get(4)?,
                        account: row.get(5)?,
                        project: row.get(6)?,
                        assignment_available: row.get(7)?,
                        record_key: row.get(16)?,
                        usage: if confirmed {
                            Some(TokenUsage {
                                input_tokens: u64_from_sql(row.get(9)?, 9)?,
                                cached_input_tokens: u64_from_sql(row.get(10)?, 10)?,
                                cache_write_input_tokens: u64_from_sql(row.get(11)?, 11)?,
                                cache_write_observed_input_tokens: u64_from_sql(row.get(12)?, 12)?,
                                output_tokens: u64_from_sql(row.get(13)?, 13)?,
                                reasoning_output_tokens: u64_from_sql(row.get(14)?, 14)?,
                                total_tokens: u64_from_sql(row.get(15)?, 15)?,
                            })
                        } else {
                            None
                        },
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        if rows.len() > limit {
            return Err(StoreError::UnionLimit);
        }
        let report = source_union::plan(rows, start, end)?;
        transaction.commit()?;
        Ok(report)
    }
}
