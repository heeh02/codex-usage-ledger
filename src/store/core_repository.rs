use super::*;

impl LedgerStore {
    /// Audit-only snapshot: no selector refresh, migration or cache preparation.
    pub(crate) fn with_source_audit_snapshot<T, E: From<StoreError>>(
        &self,
        read: impl FnOnce(&Self) -> Result<T, E>,
    ) -> Result<T, E> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(StoreError::from)?;
        let result = read(self)?;
        transaction.commit().map_err(StoreError::from)?;
        Ok(result)
    }

    pub(crate) fn reconstruction_audit_fact(
        &self,
        event_id: &str,
    ) -> StoreResult<Option<ReconstructionAuditFact>> {
        self.connection.prepare_cached("SELECT COALESCE(r.source_timestamp,r.observed_at),r.thread_id,r.model,
            r.account_fingerprint,r.project_id,p.record_key,r.input_tokens,r.cached_input_tokens,r.cache_write_input_tokens,
            r.cache_write_observed_input_tokens,r.output_tokens,r.reasoning_output_tokens,r.total_tokens,r.event_hash
            FROM reconstruction_usage_events r LEFT JOIN source_record_evidence p ON p.evidence_source='reconstruction' AND p.event_id=r.event_id
            WHERE r.event_id=?1")?.query_row([event_id], |row| Ok(ReconstructionAuditFact {
                event_id:event_id.into(),stored_hash:Some(row.get(13)?),at:parse_timestamp_column(row.get(0)?,0)?,thread:row.get(1)?,model:row.get(2)?,account:row.get(3)?,project:row.get(4)?,record_key:row.get(5)?,
                usage:TokenUsage { input_tokens:u64_from_sql(row.get(6)?,6)?,cached_input_tokens:u64_from_sql(row.get(7)?,7)?,cache_write_input_tokens:u64_from_sql(row.get(8)?,8)?,
                    cache_write_observed_input_tokens:u64_from_sql(row.get(9)?,9)?,output_tokens:u64_from_sql(row.get(10)?,10)?,reasoning_output_tokens:u64_from_sql(row.get(11)?,11)?,total_tokens:u64_from_sql(row.get(12)?,12)? },
            })).optional().map_err(StoreError::from)
    }

    /// Refresh the derived selector before opening a read snapshot. If another
    /// writer dirties it in that gap, retry preparation rather than refreshing
    /// (writing/nesting a transaction) from inside the frozen read view.
    pub(crate) fn with_usage_snapshot<T>(
        &self,
        read: impl FnOnce(&Self) -> StoreResult<T>,
    ) -> StoreResult<T> {
        for _ in 0..3 {
            self.refresh_effective_source_selection()?;
            let transaction = self.connection.unchecked_transaction()?;
            let dirty: bool = transaction.query_row(
                "SELECT dirty FROM effective_source_selection_state WHERE id=1",
                [],
                |row| row.get(0),
            )?;
            if dirty {
                transaction.rollback()?;
                continue;
            }
            let _memo = snapshot_memo::MemoScope::begin(&self.exact_series_memo);
            let result = read(self)?;
            transaction.commit()?;
            return Ok(result);
        }
        Err(StoreError::SnapshotUnavailable)
    }

    /// Does not create a database, migrate, optimize, or refresh projections.
    pub fn open_read_only(path: impl AsRef<Path>) -> StoreResult<Self> {
        let connection =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "query_only", "ON")?;
        connection.pragma_update(None, "trusted_schema", "OFF")?;
        let found: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if found != migrations::CURRENT_SCHEMA_VERSION {
            return Err(StoreError::UnsupportedAuditSchema {
                found,
                supported: migrations::CURRENT_SCHEMA_VERSION,
            });
        }
        Ok(Self {
            connection,
            exact_series_memo: Default::default(),
        })
    }

    pub fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection, true)
    }

    pub fn open_in_memory() -> StoreResult<Self> {
        let connection = Connection::open_in_memory()?;
        Self::from_connection(connection, false)
    }

    fn from_connection(mut connection: Connection, require_wal: bool) -> StoreResult<Self> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "trusted_schema", "OFF")?;
        if require_wal {
            let journal_mode: String =
                connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
            if !journal_mode.eq_ignore_ascii_case("wal") {
                return Err(StoreError::Sqlite(rusqlite::Error::InvalidQuery));
            }
        }
        connection.pragma_update(None, "wal_autocheckpoint", 1_000_i64)?;
        connection.pragma_update(None, "journal_size_limit", 67_108_864_i64)?;

        migrations::migrate(&mut connection)?;
        Ok(Self {
            connection,
            exact_series_memo: Default::default(),
        })
    }

    pub fn schema_version(&self) -> StoreResult<i64> {
        Ok(self
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    /// Returns the total number of Codex accounts the user says they have used
    /// on this ledger. This is a completeness target, not an observed identity:
    /// missing accounts never receive synthetic IDs or synthetic usage.
    pub fn user_confirmed_account_count(&self) -> StoreResult<Option<u64>> {
        let value = self
            .connection
            .query_row(
                "SELECT user_confirmed_total FROM account_registry_settings WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        value
            .map(|value| u64_from_sql(value, 0).map_err(StoreError::from))
            .transpose()
    }

    /// Persists or clears the user-declared completeness target. Observed
    /// identities remain authoritative for attribution and are never deleted.
    pub fn set_user_confirmed_account_count(&mut self, count: Option<u64>) -> StoreResult<()> {
        match count {
            Some(count) => {
                let count = sql_u64(count, "user_confirmed_account_count")?;
                self.connection.execute(
                    "INSERT INTO account_registry_settings(id, user_confirmed_total, updated_at)
                     VALUES (1, ?1, ?2)
                     ON CONFLICT(id) DO UPDATE SET
                        user_confirmed_total = excluded.user_confirmed_total,
                        updated_at = excluded.updated_at",
                    params![count, timestamp(Utc::now())],
                )?;
            }
            None => {
                self.connection
                    .execute("DELETE FROM account_registry_settings WHERE id = 1", [])?;
            }
        }
        Ok(())
    }

    pub fn ledger_table_counts(&self) -> StoreResult<LedgerTableCounts> {
        self.connection
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM usage_events),
                     (SELECT COUNT(*) FROM compacted_event_keys),
                     (SELECT COUNT(*) FROM file_cursors)",
                [],
                |row| {
                    Ok(LedgerTableCounts {
                        raw_events: u64_from_sql(row.get(0)?, 0)?,
                        compacted_event_keys: u64_from_sql(row.get(1)?, 1)?,
                        file_cursors: u64_from_sql(row.get(2)?, 2)?,
                    })
                },
            )
            .map_err(StoreError::from)
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }
}
