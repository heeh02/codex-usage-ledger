use super::*;

impl LedgerStore {
    pub fn upsert_event(&mut self, event: &UsageEvent) -> StoreResult<UpsertOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = upsert_event_in(&transaction, event)?;
        transaction.commit()?;
        Ok(outcome)
    }

    pub fn upsert_thread_catalog(&mut self, record: &ThreadCatalogRecord) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        upsert_thread_catalog_in(&transaction, record)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn upsert_thread_catalog_batch(
        &mut self,
        records: &[ThreadCatalogRecord],
    ) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for record in records {
            upsert_thread_catalog_in(&transaction, record)?;
        }
        rebuild_standalone_membership_in(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    /// Reconciles the current Codex thread index without deleting historical
    /// labels or usage. Rows absent from the new native snapshot remain in the
    /// ledger with `present_in_codex = 0` so project/session history survives
    /// Codex-side cleanup.
    pub fn sync_native_thread_catalog_batch(
        &mut self,
        records: &[ThreadCatalogRecord],
    ) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute("UPDATE thread_catalog SET present_in_codex = 0", [])?;
        for record in records {
            upsert_thread_catalog_in(&transaction, record)?;
        }
        rebuild_standalone_membership_in(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    /// Applies events and advances their source cursor in one transaction. A
    /// failed event never leaves the cursor ahead of durable data.
    pub fn upsert_events_and_cursor(
        &mut self,
        events: &[UsageEvent],
        cursor: &FileCursor,
    ) -> StoreResult<BatchOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut outcome = BatchOutcome::default();
        let raw_retention_cutoff = Utc::now() - ChronoDuration::days(RAW_EVENT_RETENTION_DAYS);
        for event in events {
            let effective_at = event.source_timestamp.unwrap_or(event.observed_at);
            if effective_at < raw_retention_cutoff {
                outcome.observe(upsert_compact_event_in(&transaction, event)?);
            } else {
                outcome.observe(upsert_event_in(&transaction, event)?);
            }
        }
        advance_cursor_in(&transaction, cursor)?;
        transaction.commit()?;
        Ok(outcome)
    }

    /// Applies source-verified events without age-based compaction. Used by a
    /// fresh post-sampling rebuild so raw and rollup totals can be reconciled
    /// before the ordinary retention policy is allowed to remove old details.
    pub fn upsert_verified_events_and_cursor(
        &mut self,
        events: &[UsageEvent],
        cursor: &FileCursor,
    ) -> StoreResult<BatchOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut outcome = BatchOutcome::default();
        for event in events {
            outcome.observe(upsert_event_in(&transaction, event)?);
        }
        advance_cursor_in(&transaction, cursor)?;
        transaction.commit()?;
        Ok(outcome)
    }

    /// Persists replay-safe reconstruction facts, source progress and the
    /// matching byte/parser cursor atomically. Reconstruction is deliberately
    /// isolated from `usage_events`; the effective views choose one source per
    /// thread/day instead of adding Sampling and Reconstruction together.
    pub fn upsert_reconstruction_events_and_cursor(
        &mut self,
        events: &[ReconstructionEvent],
        source: &ReconstructionSourceStatus,
        cursor: &FileCursor,
    ) -> StoreResult<BatchOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut outcome = BatchOutcome::default();
        for event in events {
            outcome.observe(upsert_reconstruction_event_in(&transaction, event)?);
        }
        upsert_reconstruction_source_in(&transaction, source)?;
        advance_cursor_in(&transaction, cursor)?;
        transaction.commit()?;
        Ok(outcome)
    }

    pub fn upsert_reconstruction_source(
        &mut self,
        source: &ReconstructionSourceStatus,
    ) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        upsert_reconstruction_source_in(&transaction, source)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn upsert_reconstruction_sources(
        &mut self,
        sources: &[ReconstructionSourceStatus],
    ) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for source in sources {
            upsert_reconstruction_source_in(&transaction, source)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Replaces derived reconstruction facts when Codex has replaced the
    /// physical rollout behind a logical thread. Replaying the new file on top
    /// of the old identity would double count, so the replacement and rollup
    /// rebuild happen in one transaction.
    pub fn replace_reconstruction_sources(
        &mut self,
        sources: &[ReconstructionSourceStatus],
    ) -> StoreResult<usize> {
        if sources.is_empty() {
            return Ok(0);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut deleted_events = 0_usize;
        for source in sources {
            deleted_events += transaction.execute(
                "DELETE FROM reconstruction_usage_events
                 WHERE machine_id = ?1 AND source_id = ?2",
                params![source.machine_id, source.source_id],
            )?;
            transaction.execute(
                "DELETE FROM file_cursors WHERE machine_id = ?1 AND source_id = ?2",
                params![source.machine_id, source.source_id],
            )?;
            upsert_reconstruction_source_in(&transaction, source)?;
        }
        if deleted_events > 0 {
            rebuild_reconstruction_rollups_in(&transaction)?;
        }
        transaction.commit()?;
        Ok(deleted_events)
    }

    pub fn reconstruction_sources(&self) -> StoreResult<Vec<ReconstructionSourceStatus>> {
        let mut statement = self.connection.prepare(
            "SELECT machine_id, source_id, thread_id, file_identity, status,
                    bytes_total, bytes_processed, prefix_events, unchanged_events,
                    counter_resets, last_error, updated_at
             FROM reconstruction_sources ORDER BY updated_at, source_id",
        )?;
        let rows = statement.query_map([], |row| {
            let status: String = row.get(4)?;
            let updated_at: String = row.get(11)?;
            Ok(ReconstructionSourceStatus {
                machine_id: row.get(0)?,
                source_id: row.get(1)?,
                thread_id: row.get(2)?,
                file_identity: row.get(3)?,
                status: reconstruction_status_from_name(&status, 4)?,
                bytes_total: u64_from_sql(row.get(5)?, 5)?,
                bytes_processed: u64_from_sql(row.get(6)?, 6)?,
                prefix_events: u64_from_sql(row.get(7)?, 7)?,
                unchanged_events: u64_from_sql(row.get(8)?, 8)?,
                counter_resets: u64_from_sql(row.get(9)?, 9)?,
                last_error: row.get(10)?,
                updated_at: parse_timestamp_column(updated_at, 11)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn advance_cursor(&mut self, cursor: &FileCursor) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        advance_cursor_in(&transaction, cursor)?;
        transaction.commit()?;
        Ok(())
    }

    /// Explicitly resets a cursor after the caller has independently verified a
    /// same-identity truncate or replacement. Ordinary `advance_cursor` remains
    /// monotonic and rejects accidental regressions.
    pub fn reset_cursor(&mut self, cursor: &FileCursor) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        write_cursor_in(&transaction, cursor)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_cursor(&self, machine_id: &str, source_id: &str) -> StoreResult<Option<FileCursor>> {
        self.connection
            .query_row(
                "SELECT machine_id, source_id, file_identity, byte_offset, line_number,
                        parser_state_json, updated_at
                 FROM file_cursors
                 WHERE machine_id = ?1 AND source_id = ?2",
                params![machine_id, source_id],
                |row| {
                    let byte_offset: i64 = row.get(3)?;
                    let line_number: i64 = row.get(4)?;
                    let updated_at: String = row.get(6)?;
                    Ok(FileCursor {
                        machine_id: row.get(0)?,
                        source_id: row.get(1)?,
                        file_identity: row.get(2)?,
                        byte_offset: u64_from_sql(byte_offset, 3)?,
                        line_number: u64_from_sql(line_number, 4)?,
                        parser_state_json: row.get(5)?,
                        updated_at: parse_timestamp_column(updated_at, 6)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Drops only resumable parser state for a source that can no longer be
    /// continued safely. Already materialized usage facts remain immutable.
    pub fn remove_cursor(&mut self, machine_id: &str, source_id: &str) -> StoreResult<()> {
        self.connection.execute(
            "DELETE FROM file_cursors WHERE machine_id = ?1 AND source_id = ?2",
            params![machine_id, source_id],
        )?;
        Ok(())
    }

    pub fn remove_unrecoverable_reconstruction_cursors(
        &mut self,
        machine_id: &str,
    ) -> StoreResult<usize> {
        self.connection
            .execute(
                "DELETE FROM file_cursors
                 WHERE machine_id = ?1
                   AND source_id IN (
                       SELECT source_id FROM reconstruction_sources
                       WHERE machine_id = ?1 AND status = 'unrecoverable'
                   )",
                [machine_id],
            )
            .map_err(StoreError::from)
    }

    /// Finds the latest checkpoint for a stable file identity even when the
    /// source path changed, such as a rollout moved into `archived_sessions`.
    pub fn get_cursor_by_file_identity(
        &self,
        machine_id: &str,
        file_identity: &str,
    ) -> StoreResult<Option<FileCursor>> {
        self.connection
            .query_row(
                "SELECT machine_id, source_id, file_identity, byte_offset, line_number,
                        parser_state_json, updated_at
                 FROM file_cursors
                 WHERE machine_id = ?1 AND file_identity = ?2
                 ORDER BY updated_at DESC, rowid DESC LIMIT 1",
                params![machine_id, file_identity],
                |row| {
                    let byte_offset: i64 = row.get(3)?;
                    let line_number: i64 = row.get(4)?;
                    let updated_at: String = row.get(6)?;
                    Ok(FileCursor {
                        machine_id: row.get(0)?,
                        source_id: row.get(1)?,
                        file_identity: row.get(2)?,
                        byte_offset: u64_from_sql(byte_offset, 3)?,
                        line_number: u64_from_sql(line_number, 4)?,
                        parser_state_json: row.get(5)?,
                        updated_at: parse_timestamp_column(updated_at, 6)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn get_event(&self, event_id: &str) -> StoreResult<Option<UsageEvent>> {
        self.connection
            .query_row(
                &format!(
                    "SELECT {} FROM usage_events WHERE event_id = ?1",
                    EVENT_SELECT_COLUMNS
                ),
                params![event_id],
                row_to_event,
            )
            .optional()
            .map_err(StoreError::from)
    }
}
