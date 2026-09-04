use super::*;

impl LedgerStore {
    pub fn rollup_progress(&self) -> StoreResult<RollupProgress> {
        self.connection
            .query_row(
                "SELECT last_backfilled_rowid, target_rowid, complete, updated_at
                 FROM rollup_state WHERE id = 1",
                [],
                |row| {
                    let last: i64 = row.get(0)?;
                    let target: i64 = row.get(1)?;
                    let updated_at: String = row.get(3)?;
                    Ok(RollupProgress {
                        last_backfilled_rowid: u64_from_sql(last, 0)?,
                        target_rowid: u64_from_sql(target, 1)?,
                        complete: row.get(2)?,
                        updated_at: parse_timestamp_column(updated_at, 3)?,
                    })
                },
            )
            .map_err(StoreError::from)
    }

    pub fn backfill_rollup_chunk(&mut self, batch_size: usize) -> StoreResult<RollupProgress> {
        let progress = self.rollup_progress()?;
        if progress.complete {
            return Ok(progress);
        }
        let batch_size = batch_size.clamp(1, 250_000) as i64;
        let end_rowid: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(rowid), ?1) FROM (
                 SELECT rowid FROM usage_events
                 WHERE rowid > ?1 AND rowid <= ?2
                 ORDER BY rowid LIMIT ?3
             )",
            params![
                sql_u64(progress.last_backfilled_rowid, "last_backfilled_rowid")?,
                sql_u64(progress.target_rowid, "target_rowid")?,
                batch_size,
            ],
            |row| row.get(0),
        )?;
        let end_rowid = u64_from_sql(end_rowid, 0)?;
        if end_rowid <= progress.last_backfilled_rowid {
            self.connection.execute(
                "UPDATE rollup_state SET complete = 1, updated_at = ?1 WHERE id = 1",
                params![timestamp(Utc::now())],
            )?;
            return self.rollup_progress();
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO daily_usage_rollups(
                 local_day, thread_key, account_key, project_key, model_key, quality,
                 event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                 cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                 total_tokens
             )
             SELECT date(COALESCE(source_timestamp, observed_at), '+8 hours'),
                    COALESCE(thread_id, ''), COALESCE(account_fingerprint, ''),
                    COALESCE(project_id, ''), COALESCE(model, ''), quality,
                    COUNT(*), SUM(input_tokens), SUM(cached_input_tokens),
                    SUM(cache_write_input_tokens), SUM(cache_write_observed_input_tokens),
                    SUM(output_tokens), SUM(reasoning_output_tokens), SUM(total_tokens)
             FROM usage_events
             WHERE rowid > ?1 AND rowid <= ?2
             GROUP BY 1, 2, 3, 4, 5, 6
             ON CONFLICT(local_day, thread_key, account_key, project_key, model_key, quality)
             DO UPDATE SET
                 event_count = event_count + excluded.event_count,
                 input_tokens = input_tokens + excluded.input_tokens,
                 cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
                 cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
                 cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
                 output_tokens = output_tokens + excluded.output_tokens,
                 reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
                 total_tokens = total_tokens + excluded.total_tokens",
            params![
                sql_u64(progress.last_backfilled_rowid, "last_backfilled_rowid")?,
                sql_u64(end_rowid, "end_rowid")?,
            ],
        )?;
        transaction.execute(
            "UPDATE rollup_state
             SET last_backfilled_rowid = ?1,
                 complete = CASE WHEN ?1 >= target_rowid THEN 1 ELSE 0 END,
                 updated_at = ?2
             WHERE id = 1",
            params![sql_u64(end_rowid, "end_rowid")?, timestamp(Utc::now())],
        )?;
        transaction.commit()?;
        self.rollup_progress()
    }

    pub fn set_collector_status(&mut self, status: &CollectorStatus) -> StoreResult<()> {
        self.connection.execute(
            "INSERT INTO collector_status(
                 id, mode, phase, items_total, items_completed, bytes_read,
                 events_inserted, message, updated_at
             ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                 mode = excluded.mode,
                 phase = excluded.phase,
                 items_total = excluded.items_total,
                 items_completed = excluded.items_completed,
                 bytes_read = excluded.bytes_read,
                 events_inserted = excluded.events_inserted,
                 message = excluded.message,
                 updated_at = excluded.updated_at",
            params![
                status.mode,
                status.phase,
                sql_u64(status.items_total, "items_total")?,
                sql_u64(status.items_completed, "items_completed")?,
                sql_u64(status.bytes_read, "bytes_read")?,
                sql_u64(status.events_inserted, "events_inserted")?,
                status.message,
                timestamp(status.updated_at),
            ],
        )?;
        Ok(())
    }

    pub fn verify_rollup_before_compaction(&self) -> StoreResult<()> {
        let progress = self.rollup_progress()?;
        if !progress.complete {
            return Err(StoreError::RollupMismatch);
        }
        let already_verified: bool = self.connection.query_row(
            "SELECT verified_at IS NOT NULL FROM rollup_state WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        if already_verified {
            return Ok(());
        }
        let all = AggregateFilter {
            quality: None,
            ..AggregateFilter::default()
        };
        let raw = self.aggregate_usage(&all)?;
        let rollup = self.aggregate_rollup_usage(&all)?;
        if raw != rollup {
            return Err(StoreError::RollupMismatch);
        }
        self.connection.execute(
            "UPDATE rollup_state SET verified_at = ?1, updated_at = ?1 WHERE id = 1",
            params![timestamp(Utc::now())],
        )?;
        Ok(())
    }

    pub fn collector_status(&self) -> StoreResult<CollectorStatus> {
        self.connection
            .query_row(
                "SELECT mode, phase, items_total, items_completed, bytes_read,
                        events_inserted, message, updated_at
                 FROM collector_status WHERE id = 1",
                [],
                |row| {
                    let updated_at: String = row.get(7)?;
                    Ok(CollectorStatus {
                        mode: row.get(0)?,
                        phase: row.get(1)?,
                        items_total: u64_from_sql(row.get(2)?, 2)?,
                        items_completed: u64_from_sql(row.get(3)?, 3)?,
                        bytes_read: u64_from_sql(row.get(4)?, 4)?,
                        events_inserted: u64_from_sql(row.get(5)?, 5)?,
                        message: row.get(6)?,
                        updated_at: parse_timestamp_column(updated_at, 7)?,
                    })
                },
            )
            .map_err(StoreError::from)
    }

    pub fn checkpoint_wal(&self) -> StoreResult<()> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    /// Rebinds usage facts to the current Codex project directory without
    /// changing any token dimension. This is needed when older thread rows had
    /// no native project_id but can now be resolved through a configured root.
    pub fn reproject_usage_from_catalog(&mut self) -> StoreResult<()> {
        let all = AggregateFilter {
            quality: None,
            ..AggregateFilter::default()
        };
        let before = self.aggregate_rollup_usage(&all)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE usage_events
             SET project_id = (
                     SELECT catalog.project_id FROM thread_catalog catalog
                     WHERE catalog.thread_id = usage_events.thread_id
                 ),
                 project_name = (
                     SELECT catalog.project_name FROM thread_catalog catalog
                     WHERE catalog.thread_id = usage_events.thread_id
                 )
             WHERE thread_id IS NOT NULL
               AND EXISTS (
                   SELECT 1 FROM thread_catalog catalog
                   WHERE catalog.thread_id = usage_events.thread_id
                     AND catalog.project_id IS NOT NULL
                     AND catalog.project_id <> COALESCE(usage_events.project_id, '')
               )",
            [],
        )?;
        transaction.execute_batch(
            "DROP TABLE IF EXISTS temp.reprojected_daily_usage;
             CREATE TEMP TABLE reprojected_daily_usage AS
             SELECT rollup.local_day,
                    rollup.thread_key,
                    rollup.account_key,
                    COALESCE(catalog.project_id, NULLIF(rollup.project_key, ''), '') AS project_key,
                    rollup.model_key,
                    rollup.quality,
                    SUM(rollup.event_count) AS event_count,
                    SUM(rollup.input_tokens) AS input_tokens,
                    SUM(rollup.cached_input_tokens) AS cached_input_tokens,
                    SUM(rollup.cache_write_input_tokens) AS cache_write_input_tokens,
                    SUM(rollup.cache_write_observed_input_tokens) AS cache_write_observed_input_tokens,
                    SUM(rollup.output_tokens) AS output_tokens,
                    SUM(rollup.reasoning_output_tokens) AS reasoning_output_tokens,
                    SUM(rollup.total_tokens) AS total_tokens
             FROM daily_usage_rollups rollup
             LEFT JOIN thread_catalog catalog ON catalog.thread_id = rollup.thread_key
             GROUP BY 1, 2, 3, 4, 5, 6;",
        )?;
        let rebuilt = transaction.query_row(
            "SELECT COALESCE(SUM(event_count), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM reprojected_daily_usage",
            [],
            aggregate_from_row,
        )?;
        if rebuilt != before {
            return Err(StoreError::RollupMismatch);
        }
        transaction.execute("DELETE FROM daily_usage_rollups", [])?;
        transaction.execute_batch(
            "INSERT INTO daily_usage_rollups(
                 local_day, thread_key, account_key, project_key, model_key, quality,
                 event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                 cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                 total_tokens
             )
             SELECT local_day, thread_key, account_key, project_key, model_key, quality,
                    event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                    cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                    total_tokens
             FROM reprojected_daily_usage;
             DROP TABLE reprojected_daily_usage;

             DROP TABLE IF EXISTS temp.reprojected_hourly_usage;
             CREATE TEMP TABLE reprojected_hourly_usage AS
             SELECT rollup.local_hour,
                    rollup.thread_key,
                    rollup.account_key,
                    COALESCE(catalog.project_id, NULLIF(rollup.project_key, ''), '') AS project_key,
                    rollup.model_key,
                    rollup.quality,
                    SUM(rollup.event_count) AS event_count,
                    SUM(rollup.input_tokens) AS input_tokens,
                    SUM(rollup.cached_input_tokens) AS cached_input_tokens,
                    SUM(rollup.cache_write_input_tokens) AS cache_write_input_tokens,
                    SUM(rollup.cache_write_observed_input_tokens) AS cache_write_observed_input_tokens,
                    SUM(rollup.output_tokens) AS output_tokens,
                    SUM(rollup.reasoning_output_tokens) AS reasoning_output_tokens,
                    SUM(rollup.total_tokens) AS total_tokens
             FROM hourly_usage_rollups rollup
             LEFT JOIN thread_catalog catalog ON catalog.thread_id = rollup.thread_key
             GROUP BY 1, 2, 3, 4, 5, 6;
             DELETE FROM hourly_usage_rollups;
             INSERT INTO hourly_usage_rollups(
                 local_hour, thread_key, account_key, project_key, model_key, quality,
                 event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                 cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                 total_tokens
             )
             SELECT local_hour, thread_key, account_key, project_key, model_key, quality,
                    event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                    cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                    total_tokens
             FROM reprojected_hourly_usage;
             DROP TABLE reprojected_hourly_usage;",
        )?;
        let reconstruction_changed = transaction.execute(
            "UPDATE reconstruction_usage_events
             SET project_id = (
                     SELECT catalog.project_id FROM thread_catalog catalog
                     WHERE catalog.thread_id = reconstruction_usage_events.thread_id
                 ),
                 project_name = (
                     SELECT catalog.project_name FROM thread_catalog catalog
                     WHERE catalog.thread_id = reconstruction_usage_events.thread_id
                 )
             WHERE EXISTS (
                 SELECT 1 FROM thread_catalog catalog
                 WHERE catalog.thread_id = reconstruction_usage_events.thread_id
                   AND COALESCE(catalog.project_id, '') <>
                       COALESCE(reconstruction_usage_events.project_id, '')
             )",
            [],
        )?;
        if reconstruction_changed > 0 {
            rebuild_reconstruction_rollups_in(&transaction)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn compact_raw_events_chunk(
        &mut self,
        retain_since: DateTime<Utc>,
        limit: usize,
    ) -> StoreResult<usize> {
        if !self.rollup_progress()?.complete {
            return Ok(0);
        }
        let verified: bool = self.connection.query_row(
            "SELECT verified_at IS NOT NULL FROM rollup_state WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        if !verified {
            return Err(StoreError::RollupNotVerified);
        }
        let limit = limit.clamp(1, 250_000) as i64;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS compaction_candidates(
                 event_id TEXT PRIMARY KEY
             ) WITHOUT ROWID;
             DELETE FROM compaction_candidates;",
        )?;
        let selected = transaction.execute(
            "INSERT INTO compaction_candidates(event_id)
             SELECT event_id FROM usage_events INDEXED BY usage_events_effective_time_idx
             WHERE COALESCE(source_timestamp, observed_at) < ?1
             ORDER BY COALESCE(source_timestamp, observed_at), event_id
             LIMIT ?2",
            params![timestamp(retain_since), limit],
        )?;
        if selected == 0 {
            transaction.commit()?;
            return Ok(0);
        }
        transaction.execute(
            "UPDATE rollup_control SET suppress_delete = 1 WHERE id = 1",
            [],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO compacted_event_keys(event_id, event_hash, compacted_at)
             SELECT usage.event_id, usage.event_hash, ?1
             FROM usage_events usage
             JOIN compaction_candidates candidate ON candidate.event_id = usage.event_id",
            params![timestamp(Utc::now())],
        )?;
        let deleted = transaction.execute(
            "DELETE FROM usage_events
             WHERE event_id IN (SELECT event_id FROM compaction_candidates)",
            [],
        )?;
        transaction.execute(
            "UPDATE rollup_control SET suppress_delete = 0 WHERE id = 1",
            [],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn vacuum(&self) -> StoreResult<()> {
        self.connection.execute_batch("VACUUM;")?;
        Ok(())
    }
}
