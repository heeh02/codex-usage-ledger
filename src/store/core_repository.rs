use super::*;

impl LedgerStore {
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
        Ok(Self { connection })
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
