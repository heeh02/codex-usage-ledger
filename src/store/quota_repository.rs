use super::*;

#[derive(Debug)]
pub(crate) struct IndexedQuotaWindow {
    pub snapshot_id: String,
    pub observed_at: DateTime<Utc>,
    pub stream_key: String,
    pub pool_key: String,
    pub limit_id: String,
    pub limit_name: Option<String>,
    pub role: String,
    pub used_percent: Option<f64>,
    pub window_seconds: Option<u64>,
    pub resets_at_unix: Option<i64>,
}

pub(super) fn project_quota_snapshot_in(
    connection: &Connection,
    stored: &StoredQuotaSnapshot,
) -> StoreResult<()> {
    let mut ordinal = 0_i64;
    for pool in &stored.snapshot.pools {
        for window in &pool.windows {
            let stream = crate::quota::window_stream_key(pool, window);
            let id = format!("{}:{ordinal}", stored.snapshot_id);
            let inserted = connection.execute(
                "INSERT OR IGNORE INTO quota_window_observations(
                    observation_id,snapshot_id,account_fingerprint,auth_epoch,observed_at,
                    stream_key,pool_key,limit_id,limit_name,role,used_percent,window_seconds,resets_at_unix,window_ordinal
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                params![id,stored.snapshot_id,stored.account_fingerprint,stored.auth_epoch,timestamp(stored.observed_at),
                    stream,pool.pool_key,pool.limit_id.as_deref().unwrap_or(&pool.pool_key),pool.limit_name,
                    window.role.as_str(),window.used_percent.filter(|value| value.is_finite() && (0.0..=100.0).contains(value)),
                    window.window_seconds.map(|value| value.to_string()),window.resets_at_unix,ordinal],
            )?;
            if inserted == 0 {
                let same: bool = connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM quota_window_observations WHERE
                     observation_id=?1 AND snapshot_id=?2 AND account_fingerprint=?3 AND auth_epoch=?4
                     AND observed_at=?5 AND stream_key=?6 AND pool_key=?7 AND limit_id=?8
                     AND limit_name IS ?9 AND role=?10 AND used_percent IS ?11
                     AND window_seconds IS ?12 AND resets_at_unix IS ?13 AND window_ordinal=?14)",
                    params![id,stored.snapshot_id,stored.account_fingerprint,stored.auth_epoch,timestamp(stored.observed_at),
                        stream,pool.pool_key,pool.limit_id.as_deref().unwrap_or(&pool.pool_key),pool.limit_name,
                        window.role.as_str(),window.used_percent.filter(|value| value.is_finite() && (0.0..=100.0).contains(value)),
                        window.window_seconds.map(|value| value.to_string()),window.resets_at_unix,ordinal], |row| row.get(0),
                )?;
                if !same {
                    return Err(StoreError::QuotaWindowConflict);
                }
            } else {
                super::quota_history_repository::observe_window_in(connection, &id)?;
            }
            ordinal += 1;
        }
    }
    Ok(())
}

impl LedgerStore {
    #[cfg(test)]
    pub(crate) fn prepare_quota_index_backfill_fixture(&self) -> StoreResult<()> {
        self.connection.execute("UPDATE quota_window_index_state SET last_rowid=0,target_rowid=(SELECT MAX(rowid) FROM quota_snapshots),complete=0", [])?;
        Ok(())
    }
    /// Process at most `limit` immutable pre-upgrade snapshots. Live appends
    /// project themselves, and never advance this historical watermark.
    pub fn backfill_quota_window_index_chunk(&mut self, limit: usize) -> StoreResult<bool> {
        if self.quota_window_index_complete()? {
            return Ok(true);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (last, target): (i64, i64) = transaction.query_row(
            "SELECT last_rowid,target_rowid FROM quota_window_index_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let batch = {
            let mut statement = transaction.prepare(
                "SELECT snapshot_id,account_fingerprint,auth_epoch,observed_at,normalized_json,rowid
                 FROM quota_snapshots WHERE rowid>?1 AND rowid<=?2 ORDER BY rowid LIMIT ?3",
            )?;
            statement
                .query_map(params![last, target, limit.clamp(1, 1000) as i64], |row| {
                    Ok((row.get::<_, i64>(5)?, stored_quota_from_row(row)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        for (_, stored) in &batch {
            project_quota_snapshot_in(&transaction, stored)?;
        }
        let next = batch.last().map_or(last, |(id, _)| *id);
        let complete: bool = transaction.query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM quota_snapshots WHERE rowid>?1 AND rowid<=?2)",
            params![next, target],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE quota_window_index_state SET last_rowid=?1,complete=?2 WHERE id=1",
            params![next, complete],
        )?;
        transaction.commit()?;
        Ok(complete)
    }

    pub(crate) fn quota_window_index_complete(&self) -> StoreResult<bool> {
        Ok(self.connection.query_row(
            "SELECT complete FROM quota_window_index_state WHERE id=1",
            [],
            |row| row.get(0),
        )?)
    }

    /// Current bounded preview, using indexed windows only after the historical
    /// projection is complete. Full-history seek paging builds on these indexes.
    pub(crate) fn indexed_quota_window_preview(
        &self,
        account: &str,
        limit: usize,
    ) -> StoreResult<Option<(Vec<IndexedQuotaWindow>, bool)>> {
        if !self.quota_window_index_complete()? {
            return Ok(None);
        }
        let read = |store: &LedgerStore| -> StoreResult<Option<(Vec<IndexedQuotaWindow>, bool)>> {
            let limit = limit.clamp(1, 1000) as i64;
            let count: i64 = store.connection.query_row(
                "SELECT COUNT(*) FROM (SELECT 1 FROM quota_snapshots WHERE account_fingerprint=?1 ORDER BY observed_at DESC,rowid DESC LIMIT ?2)",
                params![account,limit], |row| row.get(0),
            )?;
            let mut statement = store.connection.prepare(
                "SELECT w.snapshot_id,w.observed_at,w.stream_key,w.pool_key,w.limit_id,w.limit_name,w.role,w.used_percent,w.window_seconds,w.resets_at_unix
                 FROM quota_snapshots s JOIN quota_window_observations w ON w.snapshot_id=s.snapshot_id
                 WHERE s.snapshot_id IN (SELECT snapshot_id FROM quota_snapshots WHERE account_fingerprint=?1 ORDER BY observed_at DESC,rowid DESC LIMIT ?2)
                 ORDER BY w.observed_at,s.rowid,w.window_ordinal",
            )?;
            let rows = statement
                .query_map(params![account, limit], |row| {
                    let at: String = row.get(1)?;
                    let seconds: Option<String> = row.get(8)?;
                    Ok(IndexedQuotaWindow {
                        snapshot_id: row.get(0)?,
                        observed_at: parse_timestamp_column(at, 1)?,
                        stream_key: row.get(2)?,
                        pool_key: row.get(3)?,
                        limit_id: row.get(4)?,
                        limit_name: row.get(5)?,
                        role: row.get(6)?,
                        used_percent: row.get(7)?,
                        window_seconds: seconds
                            .map(|value| {
                                value.parse::<u64>().map_err(|error| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        8,
                                        rusqlite::types::Type::Text,
                                        Box::new(error),
                                    )
                                })
                            })
                            .transpose()?,
                        resets_at_unix: row.get(9)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some((rows, count == limit)))
        };
        if self.connection.is_autocommit() {
            self.with_source_audit_snapshot(read)
        } else {
            read(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quota::normalize_rate_limit_event;

    fn snapshot() -> QuotaSnapshot {
        normalize_rate_limit_event(&serde_json::json!({"limit_id":"pool-a",
            "primary":{"used_percent":20,"window_minutes":10080,"resets_at":1789000000},
            "secondary":{"used_percent":30,"window_minutes":300,"resets_at":1788800000}
        }))
        .unwrap()
    }

    fn seed_legacy(path: &Path, count: usize) {
        let mut connection = Connection::open(path).unwrap();
        migrations::create_legacy_schema(&mut connection, 35).unwrap();
        let tx = connection.transaction().unwrap();
        for index in 0..count {
            tx.execute("INSERT INTO quota_snapshots(snapshot_id,account_fingerprint,auth_epoch,observed_at,source,normalized_json) VALUES (?1,'account-a','epoch',?2,'token_count_event',?3)",
                params![format!("legacy-{index}"),timestamp(DateTime::from_timestamp(1788000000 + index as i64,0).unwrap()),serde_json::to_string(&snapshot()).unwrap()]).unwrap();
        }
        tx.commit().unwrap();
    }

    fn windows(store: &LedgerStore) -> i64 {
        store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM quota_window_observations",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn original_digest(store: &LedgerStore) -> String {
        let mut digest = Sha256::new();
        let mut statement=store.connection.prepare("SELECT snapshot_id,normalized_json FROM quota_snapshots WHERE snapshot_id LIKE 'legacy-%' ORDER BY snapshot_id").unwrap();
        for row in statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
        {
            let (id, json) = row.unwrap();
            digest.update(id.as_bytes());
            digest.update(json.as_bytes());
        }
        hex::encode(digest.finalize())
    }

    #[test]
    fn quota_index_resumes_beyond_preview_limit_and_live_writes_do_not_skip_old_rows() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("quota.sqlite3");
        seed_legacy(&path, 1005);
        let mut store = LedgerStore::open(&path).unwrap();
        assert_eq!(
            store.schema_version().unwrap(),
            migrations::CURRENT_SCHEMA_VERSION
        );
        let before = original_digest(&store);
        assert!(
            store
                .indexed_quota_window_preview("account-a", 1000)
                .unwrap()
                .is_none()
        );
        assert!(!store.backfill_quota_window_index_chunk(2).unwrap());
        assert_eq!(windows(&store), 4);
        drop(store);
        let mut store = LedgerStore::open(&path).unwrap();
        let at = DateTime::from_timestamp(1787000000, 0).unwrap();
        let id = store
            .append_quota_snapshot("account-a", "new-epoch", at, &snapshot())
            .unwrap();
        assert_eq!(
            store
                .append_quota_snapshot("account-a", "new-epoch", at, &snapshot())
                .unwrap(),
            id
        );
        assert_eq!(windows(&store), 6);
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT last_rowid FROM quota_window_index_state",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
        for _ in 0..10 {
            if store.backfill_quota_window_index_chunk(128).unwrap() {
                break;
            }
        }
        assert!(store.quota_window_index_complete().unwrap());
        assert_eq!(windows(&store), 2012);
        let changes = store.connection.total_changes();
        assert!(store.backfill_quota_window_index_chunk(128).unwrap());
        assert_eq!(store.connection.total_changes(), changes);
        assert_eq!(original_digest(&store), before);
        let (rows, limited) = store
            .indexed_quota_window_preview("account-a", 1000)
            .unwrap()
            .unwrap();
        assert_eq!(rows.len(), 2000);
        assert!(limited);
        assert_eq!(rows[0].role, "primary");
        assert_eq!(rows[0].used_percent, Some(20.0));
        assert_eq!(rows[0].window_seconds, Some(604800));
        assert_eq!(rows[1].role, "secondary");
        assert_eq!(rows[1].used_percent, Some(30.0));
        assert_eq!(rows[1].window_seconds, Some(18000));
        assert!(rows.iter().all(|row| row.snapshot_id != id));
        assert!(
            store
                .indexed_quota_window_preview("other-account", 1000)
                .unwrap()
                .unwrap()
                .0
                .is_empty()
        );
    }

    #[test]
    fn quota_index_and_source_append_are_atomic() {
        let mut store = LedgerStore::open_in_memory().unwrap();
        store.connection.execute_batch("CREATE TRIGGER refuse_projection BEFORE INSERT ON quota_window_observations BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(
            store
                .append_quota_snapshot("account-a", "epoch", Utc::now(), &snapshot())
                .is_err()
        );
        assert!(
            store
                .list_quota_snapshots("account-a", 1000)
                .unwrap()
                .is_empty()
        );
        assert_eq!(windows(&store), 0);
    }

    #[test]
    fn quota_index_chunk_failure_rolls_back_cursor_and_partial_rows() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("quota.sqlite3");
        seed_legacy(&path, 3);
        let mut store = LedgerStore::open(&path).unwrap();
        let before = original_digest(&store);
        store.connection.execute_batch("CREATE TRIGGER refuse_second BEFORE INSERT ON quota_window_observations WHEN NEW.role='secondary' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(store.backfill_quota_window_index_chunk(3).is_err());
        assert_eq!(windows(&store), 0);
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT last_rowid FROM quota_window_index_state",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(original_digest(&store), before);
        store
            .connection
            .execute_batch("DROP TRIGGER refuse_second")
            .unwrap();
        assert!(store.backfill_quota_window_index_chunk(3).unwrap());
        assert_eq!(windows(&store), 6);
        store.connection.execute("UPDATE quota_window_observations SET used_percent=99 WHERE observation_id='legacy-0:0'",[]).unwrap();
        store
            .connection
            .execute(
                "UPDATE quota_window_index_state SET last_rowid=0,complete=0",
                [],
            )
            .unwrap();
        assert!(matches!(
            store.backfill_quota_window_index_chunk(3),
            Err(StoreError::QuotaWindowConflict)
        ));
        assert!(!store.quota_window_index_complete().unwrap());
        assert_eq!(original_digest(&store), before);
    }
}
