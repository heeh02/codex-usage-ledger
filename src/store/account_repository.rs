use super::*;

impl LedgerStore {
    /// Opens a new temporal auth epoch, or reuses the current open epoch when
    /// only the file watcher emitted a duplicate notification. No credential or
    /// raw account identifier is stored.
    pub fn append_auth_epoch(
        &mut self,
        machine_id: &str,
        source_id: &str,
        identity: &AuthIdentity,
        observed_at: DateTime<Utc>,
    ) -> StoreResult<AuthEpochRecord> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = transaction
            .query_row(
                "SELECT epoch_id, generation, observed_from, account_fingerprint,
                        workspace_fingerprint, confidence
                 FROM auth_epochs
                 WHERE machine_id = ?1 AND source_id = ?2 AND observed_to IS NULL
                 ORDER BY epoch_id DESC LIMIT 1",
                params![machine_id, source_id],
                |row| {
                    let observed_from: String = row.get(2)?;
                    let confidence: String = row.get(5)?;
                    Ok(AuthEpochRecord {
                        epoch_id: row.get(0)?,
                        machine_id: machine_id.to_owned(),
                        source_id: source_id.to_owned(),
                        generation: row.get(1)?,
                        observed_from: parse_timestamp_column(observed_from, 2)?,
                        observed_to: None,
                        account_fingerprint: row.get(3)?,
                        workspace_fingerprint: row.get(4)?,
                        confidence: parse_confidence_column(&confidence, 5)?,
                    })
                },
            )
            .optional()?;
        if let Some(current) = current {
            if current.generation == identity.auth_epoch {
                transaction.commit()?;
                return Ok(current);
            }
            transaction.execute(
                "UPDATE auth_epochs SET observed_to = ?1 WHERE epoch_id = ?2",
                params![timestamp(observed_at), current.epoch_id],
            )?;
        }

        transaction.execute(
            "INSERT INTO auth_epochs(
                 machine_id, source_id, generation, observed_from, observed_to,
                 account_fingerprint, workspace_fingerprint, confidence
             ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7)",
            params![
                machine_id,
                source_id,
                identity.auth_epoch,
                timestamp(observed_at),
                identity.account_fingerprint,
                identity.workspace_fingerprint,
                confidence_name(identity.confidence),
            ],
        )?;
        let epoch_id = transaction.last_insert_rowid();
        transaction.commit()?;
        Ok(AuthEpochRecord {
            epoch_id,
            machine_id: machine_id.to_owned(),
            source_id: source_id.to_owned(),
            generation: identity.auth_epoch.clone(),
            observed_from: observed_at,
            observed_to: None,
            account_fingerprint: identity.account_fingerprint.clone(),
            workspace_fingerprint: identity.workspace_fingerprint.clone(),
            confidence: identity.confidence,
        })
    }

    pub fn list_auth_epochs(
        &self,
        machine_id: &str,
        source_id: &str,
    ) -> StoreResult<Vec<AuthEpochRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT epoch_id, generation, observed_from, observed_to,
                    account_fingerprint, workspace_fingerprint, confidence
             FROM auth_epochs
             WHERE machine_id = ?1 AND source_id = ?2
             ORDER BY observed_from, epoch_id",
        )?;
        let rows = statement.query_map(params![machine_id, source_id], |row| {
            let observed_from: String = row.get(2)?;
            let observed_to: Option<String> = row.get(3)?;
            let confidence: String = row.get(6)?;
            Ok(AuthEpochRecord {
                epoch_id: row.get(0)?,
                machine_id: machine_id.to_owned(),
                source_id: source_id.to_owned(),
                generation: row.get(1)?,
                observed_from: parse_timestamp_column(observed_from, 2)?,
                observed_to: observed_to
                    .map(|value| parse_timestamp_column(value, 3))
                    .transpose()?,
                account_fingerprint: row.get(4)?,
                workspace_fingerprint: row.get(5)?,
                confidence: parse_confidence_column(&confidence, 6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn active_account_fingerprint(&self) -> StoreResult<Option<String>> {
        self.connection
            .query_row(
                "SELECT account_fingerprint FROM auth_epochs
                 WHERE observed_to IS NULL AND account_fingerprint IS NOT NULL
                 ORDER BY observed_from DESC, epoch_id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn close_current_auth_epoch(
        &mut self,
        machine_id: &str,
        source_id: &str,
        observed_at: DateTime<Utc>,
    ) -> StoreResult<bool> {
        let changed = self.connection.execute(
            "UPDATE auth_epochs SET observed_to = ?1
             WHERE machine_id = ?2 AND source_id = ?3 AND observed_to IS NULL",
            params![timestamp(observed_at), machine_id, source_id],
        )?;
        Ok(changed > 0)
    }

    pub fn canonical_account_for_workspace(
        &self,
        workspace_fingerprint: &str,
    ) -> StoreResult<Option<String>> {
        let alias = self
            .connection
            .query_row(
                "SELECT account_fingerprint
                 FROM account_workspace_aliases
                 WHERE workspace_fingerprint = ?1 AND canonical = 1",
                [workspace_fingerprint],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::from)?;
        if alias.is_some() {
            return Ok(alias);
        }
        self.connection
            .query_row(
                "SELECT account_fingerprint FROM auth_epochs
                 WHERE workspace_fingerprint = ?1
                   AND account_fingerprint IS NOT NULL
                   AND confidence = 'verified'
                 ORDER BY observed_from DESC, epoch_id DESC LIMIT 1",
                [workspace_fingerprint],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Links a workspace-only historical identity to the stronger active
    /// account identity. Returns the replaced provisional account, if any.
    pub fn upsert_workspace_account_alias(
        &mut self,
        workspace_fingerprint: &str,
        account_fingerprint: &str,
        canonical: bool,
        observed_at: DateTime<Utc>,
    ) -> StoreResult<Option<String>> {
        let previous = self
            .connection
            .query_row(
                "SELECT account_fingerprint FROM account_workspace_aliases
                 WHERE workspace_fingerprint = ?1",
                [workspace_fingerprint],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        self.connection.execute(
            "INSERT INTO account_workspace_aliases(
                 workspace_fingerprint, account_fingerprint, canonical,
                 first_seen_at, last_seen_at
             ) VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(workspace_fingerprint) DO UPDATE SET
                 account_fingerprint = CASE
                     WHEN excluded.canonical = 1 THEN excluded.account_fingerprint
                     ELSE account_workspace_aliases.account_fingerprint END,
                 canonical = MAX(account_workspace_aliases.canonical, excluded.canonical),
                 last_seen_at = MAX(account_workspace_aliases.last_seen_at, excluded.last_seen_at)",
            params![
                workspace_fingerprint,
                account_fingerprint,
                i64::from(canonical),
                timestamp(observed_at),
            ],
        )?;
        Ok(previous.filter(|value| value != account_fingerprint))
    }

    pub fn remap_account_fingerprint(
        &mut self,
        from_account: &str,
        to_account: &str,
    ) -> StoreResult<usize> {
        if from_account == to_account {
            return Ok(0);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut statement = transaction.prepare(&format!(
            "SELECT {} FROM usage_events WHERE account_fingerprint = ?1",
            EVENT_SELECT_COLUMNS
        ))?;
        let events = statement
            .query_map([from_account], row_to_event)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for mut event in events.iter().cloned() {
            event.account_fingerprint = Some(to_account.to_owned());
            upsert_event_in(&transaction, &event)?;
        }
        transaction.execute(
            "INSERT INTO hourly_usage_rollups(
                 local_hour, thread_key, account_key, project_key, model_key, quality,
                 event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                 cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                 total_tokens
             )
             SELECT local_hour, thread_key, ?1, project_key, model_key, quality,
                    event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                    cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                    total_tokens
             FROM hourly_usage_rollups WHERE account_key = ?2
             ON CONFLICT(local_hour, thread_key, account_key, project_key, model_key, quality)
             DO UPDATE SET
                 event_count = event_count + excluded.event_count,
                 input_tokens = input_tokens + excluded.input_tokens,
                 cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
                 cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
                 cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
                 output_tokens = output_tokens + excluded.output_tokens,
                 reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
                 total_tokens = total_tokens + excluded.total_tokens",
            params![to_account, from_account],
        )?;
        transaction.execute(
            "DELETE FROM hourly_usage_rollups WHERE account_key = ?1",
            [from_account],
        )?;
        transaction.execute(
            "INSERT INTO daily_usage_rollups(
                 local_day, thread_key, account_key, project_key, model_key, quality,
                 event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                 cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                 total_tokens
             )
             SELECT local_day, thread_key, ?1, project_key, model_key, quality,
                    event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                    cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                    total_tokens
             FROM daily_usage_rollups WHERE account_key = ?2
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
            params![to_account, from_account],
        )?;
        transaction.execute(
            "DELETE FROM daily_usage_rollups WHERE account_key = ?1",
            [from_account],
        )?;
        transaction.execute(
            "UPDATE reconstruction_usage_events SET account_fingerprint = ?1
             WHERE account_fingerprint = ?2",
            params![to_account, from_account],
        )?;
        transaction.execute(
            "INSERT INTO reconstruction_hourly_rollups(
                 local_hour, thread_key, account_key, project_key, model_key,
                 event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                 cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                 total_tokens
             )
             SELECT local_hour, thread_key, ?1, project_key, model_key,
                    event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                    cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                    total_tokens
             FROM reconstruction_hourly_rollups WHERE account_key = ?2
             ON CONFLICT(local_hour, thread_key, account_key, project_key, model_key)
             DO UPDATE SET
                 event_count = event_count + excluded.event_count,
                 input_tokens = input_tokens + excluded.input_tokens,
                 cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
                 cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
                 cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
                 output_tokens = output_tokens + excluded.output_tokens,
                 reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
                 total_tokens = total_tokens + excluded.total_tokens",
            params![to_account, from_account],
        )?;
        transaction.execute(
            "DELETE FROM reconstruction_hourly_rollups WHERE account_key = ?1",
            [from_account],
        )?;
        transaction.execute(
            "INSERT INTO reconstruction_daily_rollups(
                 local_day, thread_key, account_key, project_key, model_key,
                 event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                 cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                 total_tokens
             )
             SELECT local_day, thread_key, ?1, project_key, model_key,
                    event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                    cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                    total_tokens
             FROM reconstruction_daily_rollups WHERE account_key = ?2
             ON CONFLICT(local_day, thread_key, account_key, project_key, model_key)
             DO UPDATE SET
                 event_count = event_count + excluded.event_count,
                 input_tokens = input_tokens + excluded.input_tokens,
                 cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
                 cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
                 cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
                 output_tokens = output_tokens + excluded.output_tokens,
                 reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
                 total_tokens = total_tokens + excluded.total_tokens",
            params![to_account, from_account],
        )?;
        transaction.execute(
            "DELETE FROM reconstruction_daily_rollups WHERE account_key = ?1",
            [from_account],
        )?;
        transaction.execute(
            "UPDATE auth_epochs SET account_fingerprint = ?1
             WHERE account_fingerprint = ?2",
            params![to_account, from_account],
        )?;
        transaction.execute(
            "UPDATE account_workspace_aliases SET account_fingerprint = ?1, canonical = 1
             WHERE account_fingerprint = ?2",
            params![to_account, from_account],
        )?;
        transaction.commit()?;
        Ok(events.len())
    }

    pub fn upsert_auth_log_markers_and_cursor(
        &mut self,
        machine_id: &str,
        source_id: &str,
        markers: &[AuthLogMarkerRecord],
        cursor: &FileCursor,
    ) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for marker in markers {
            transaction.execute(
                "INSERT OR IGNORE INTO auth_log_markers(
                     machine_id, source_id, log_id, observed_at, kind,
                     workspace_fingerprint
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    machine_id,
                    source_id,
                    sql_u64(marker.log_id, "auth_log_marker.log_id")?,
                    timestamp(marker.observed_at),
                    marker.kind,
                    marker.workspace_fingerprint,
                ],
            )?;
        }
        advance_cursor_in(&transaction, cursor)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn auth_log_markers(&self, machine_id: &str) -> StoreResult<Vec<AuthLogMarkerRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT log_id, observed_at, kind, workspace_fingerprint
             FROM auth_log_markers WHERE machine_id = ?1
             ORDER BY observed_at, log_id",
        )?;
        let rows = statement.query_map([machine_id], |row| {
            let observed_at: String = row.get(1)?;
            Ok(AuthLogMarkerRecord {
                log_id: u64_from_sql(row.get(0)?, 0)?,
                observed_at: parse_timestamp_column(observed_at, 1)?,
                kind: row.get(2)?,
                workspace_fingerprint: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Replaces the inferred Codex-login timeline and applies it only to usage
    /// that still lacks a verified account. Token dimensions never change.
    pub fn replace_historical_auth_epochs(
        &mut self,
        machine_id: &str,
        source_id: &str,
        epochs: &[HistoricalAuthEpochInput],
    ) -> StoreResult<usize> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "DELETE FROM auth_epochs WHERE machine_id = ?1 AND source_id = ?2",
            params![machine_id, source_id],
        )?;
        for (index, epoch) in epochs.iter().enumerate() {
            transaction.execute(
                "INSERT INTO auth_epochs(
                     machine_id, source_id, generation, observed_from, observed_to,
                     account_fingerprint, workspace_fingerprint, confidence
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    machine_id,
                    source_id,
                    format!("historical-{}", index + 1),
                    timestamp(epoch.observed_from),
                    epoch.observed_to.map(timestamp),
                    epoch.account_fingerprint,
                    epoch.workspace_fingerprint,
                    confidence_name(epoch.confidence),
                ],
            )?;
        }

        let mut changed = 0_usize;
        let mut reconstruction_changed = 0_usize;
        for epoch in epochs {
            let mut statement = transaction.prepare(&format!(
                "SELECT {} FROM usage_events
                 WHERE machine_id = ?1
                   AND (account_fingerprint IS NULL OR account_confidence = 'unknown')
                   AND COALESCE(source_timestamp, observed_at) >= ?2
                   AND (?3 IS NULL OR COALESCE(source_timestamp, observed_at) < ?3)",
                EVENT_SELECT_COLUMNS
            ))?;
            let events = statement
                .query_map(
                    params![
                        machine_id,
                        timestamp(epoch.observed_from),
                        epoch.observed_to.map(timestamp),
                    ],
                    row_to_event,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            for mut event in events {
                event.account_fingerprint = Some(epoch.account_fingerprint.clone());
                event.account_confidence = epoch.confidence;
                if upsert_event_in(&transaction, &event)? != UpsertOutcome::Unchanged {
                    changed += 1;
                }
            }
            let changed_reconstruction_rows = transaction.execute(
                "UPDATE reconstruction_usage_events
                 SET account_fingerprint = ?1, account_confidence = ?2
                 WHERE machine_id = ?3
                   AND (account_fingerprint IS NULL OR account_confidence = 'unknown')
                   AND source_timestamp >= ?4
                   AND (?5 IS NULL OR source_timestamp < ?5)",
                params![
                    epoch.account_fingerprint,
                    confidence_name(epoch.confidence),
                    machine_id,
                    timestamp(epoch.observed_from),
                    epoch.observed_to.map(timestamp),
                ],
            )?;
            reconstruction_changed += changed_reconstruction_rows;
            changed += changed_reconstruction_rows;
        }

        // Raw events older than the retention window have already been folded
        // into hourly rows. Move only complete hours; switch-boundary hours stay
        // unassigned unless their raw events above resolved them exactly.
        for (epoch_index, epoch) in epochs.iter().enumerate() {
            let start = ceil_local_hour(epoch.observed_from);
            let end = epoch.observed_to.map(floor_local_hour);
            transaction.execute(
                "INSERT INTO hourly_usage_rollups(
                     local_hour, thread_key, account_key, project_key, model_key, quality,
                     event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                     cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                     total_tokens
                 )
                 SELECT local_hour, thread_key, ?1, project_key, model_key, quality,
                        event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                        cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                        total_tokens
                 FROM hourly_usage_rollups
                 WHERE account_key = '' AND local_hour >= ?2
                   AND (?3 IS NULL OR local_hour < ?3)
                 ON CONFLICT(local_hour, thread_key, account_key, project_key, model_key, quality)
                 DO UPDATE SET
                     event_count = event_count + excluded.event_count,
                     input_tokens = input_tokens + excluded.input_tokens,
                     cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
                     cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
                     cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
                     output_tokens = output_tokens + excluded.output_tokens,
                     reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
                     total_tokens = total_tokens + excluded.total_tokens",
                params![epoch.account_fingerprint, start, end],
            )?;
            transaction.execute(
                "DELETE FROM hourly_usage_rollups
                 WHERE account_key = '' AND local_hour >= ?1
                   AND (?2 IS NULL OR local_hour < ?2)",
                params![start, end],
            )?;

            let start_day = if epoch_index == 0 {
                floor_local_day(epoch.observed_from)
            } else {
                ceil_local_day(epoch.observed_from)
            };
            let end_day = epoch.observed_to.map(floor_local_day);
            transaction.execute(
                "INSERT INTO daily_usage_rollups(
                     local_day, thread_key, account_key, project_key, model_key, quality,
                     event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                     cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                     total_tokens
                 )
                 SELECT local_day, thread_key, ?1, project_key, model_key, quality,
                        event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
                        cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
                        total_tokens
                 FROM daily_usage_rollups
                 WHERE account_key = '' AND local_day >= ?2
                   AND (?3 IS NULL OR local_day < ?3)
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
                params![epoch.account_fingerprint, start_day, end_day],
            )?;
            transaction.execute(
                "DELETE FROM daily_usage_rollups
                 WHERE account_key = '' AND local_day >= ?1
                   AND (?2 IS NULL OR local_day < ?2)",
                params![start_day, end_day],
            )?;
        }
        if reconstruction_changed > 0 {
            rebuild_reconstruction_rollups_in(&transaction)?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    /// Appends an idempotent normalized quota snapshot. The identifier is a
    /// digest of the pseudonymous account scope, auth epoch, time and normalized
    /// payload; no token, email or raw account id is accepted by this API.
    pub fn append_quota_snapshot(
        &mut self,
        account_fingerprint: &str,
        auth_epoch: &str,
        observed_at: DateTime<Utc>,
        snapshot: &QuotaSnapshot,
    ) -> StoreResult<String> {
        let normalized_json = serde_json::to_string(snapshot)?;
        let mut digest = Sha256::new();
        for value in [
            account_fingerprint.as_bytes(),
            auth_epoch.as_bytes(),
            timestamp(observed_at).as_bytes(),
            normalized_json.as_bytes(),
        ] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value);
        }
        let snapshot_id = hex::encode(digest.finalize());
        self.connection.execute(
            "INSERT OR IGNORE INTO quota_snapshots(
                 snapshot_id, account_fingerprint, auth_epoch, observed_at, source, normalized_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                snapshot_id,
                account_fingerprint,
                auth_epoch,
                timestamp(observed_at),
                quota_source_name(snapshot.source),
                normalized_json,
            ],
        )?;
        Ok(snapshot_id)
    }

    pub fn latest_quota_snapshot(
        &self,
        account_fingerprint: &str,
    ) -> StoreResult<Option<StoredQuotaSnapshot>> {
        self.connection
            .query_row(
                "SELECT snapshot_id, account_fingerprint, auth_epoch, observed_at, normalized_json
                 FROM quota_snapshots
                 WHERE account_fingerprint = ?1
                 ORDER BY observed_at DESC, rowid DESC LIMIT 1",
                params![account_fingerprint],
                stored_quota_from_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn list_quota_snapshots(
        &self,
        account_fingerprint: &str,
        limit: usize,
    ) -> StoreResult<Vec<StoredQuotaSnapshot>> {
        let limit = limit.clamp(1, 1_000) as i64;
        let mut statement = self.connection.prepare(
            "SELECT snapshot_id, account_fingerprint, auth_epoch, observed_at, normalized_json
             FROM quota_snapshots
             WHERE account_fingerprint = ?1
             ORDER BY observed_at DESC, rowid DESC LIMIT ?2",
        )?;
        let rows =
            statement.query_map(params![account_fingerprint, limit], stored_quota_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Stores a credential-free snapshot from Codex `account/usage/read` and
    /// revises the durable daily buckets in place when the backend corrects a
    /// historical day.
    pub fn upsert_official_account_usage(
        &mut self,
        account_fingerprint: &str,
        observed_at: DateTime<Utc>,
        usage: &OfficialAccountUsage,
    ) -> StoreResult<String> {
        let normalized_json = serde_json::to_string(usage)?;
        let mut digest = Sha256::new();
        digest.update(account_fingerprint.as_bytes());
        digest.update(normalized_json.as_bytes());
        let snapshot_id = hex::encode(digest.finalize());
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO official_account_usage_snapshots(
                 snapshot_id, account_fingerprint, observed_at, normalized_json
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(snapshot_id) DO UPDATE SET observed_at = excluded.observed_at",
            params![
                snapshot_id,
                account_fingerprint,
                timestamp(observed_at),
                normalized_json,
            ],
        )?;
        for bucket in &usage.daily_usage_buckets {
            transaction.execute(
                "INSERT INTO official_daily_usage(
                     account_fingerprint, local_day, total_tokens, observed_at
                 ) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(account_fingerprint, local_day) DO UPDATE SET
                     total_tokens = excluded.total_tokens,
                     observed_at = excluded.observed_at",
                params![
                    account_fingerprint,
                    bucket.start_date,
                    sql_u64(bucket.tokens, "official daily tokens")?,
                    timestamp(observed_at),
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO official_usage_sync_state(
                 account_fingerprint, last_attempt_at, last_success_at, last_error
             ) VALUES (?1, ?2, ?2, NULL)
             ON CONFLICT(account_fingerprint) DO UPDATE SET
                 last_attempt_at = excluded.last_attempt_at,
                 last_success_at = excluded.last_success_at,
                 last_error = NULL",
            params![account_fingerprint, timestamp(observed_at)],
        )?;
        transaction.commit()?;
        Ok(snapshot_id)
    }

    pub fn record_official_usage_error(
        &mut self,
        account_fingerprint: &str,
        observed_at: DateTime<Utc>,
        message: &str,
    ) -> StoreResult<()> {
        self.connection.execute(
            "INSERT INTO official_usage_sync_state(
                 account_fingerprint, last_attempt_at, last_success_at, last_error
             ) VALUES (?1, ?2, NULL, ?3)
             ON CONFLICT(account_fingerprint) DO UPDATE SET
                 last_attempt_at = excluded.last_attempt_at,
                 last_error = excluded.last_error",
            params![account_fingerprint, timestamp(observed_at), message],
        )?;
        Ok(())
    }

    pub fn latest_official_account_usage(
        &self,
        account_fingerprint: &str,
    ) -> StoreResult<Option<StoredOfficialAccountUsage>> {
        self.connection
            .query_row(
                "SELECT snapshot_id, account_fingerprint, observed_at, normalized_json
                 FROM official_account_usage_snapshots
                 WHERE account_fingerprint = ?1
                 ORDER BY observed_at DESC, rowid DESC LIMIT 1",
                params![account_fingerprint],
                stored_official_usage_from_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn list_official_accounts(&self) -> StoreResult<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT account_fingerprint
             FROM official_account_usage_snapshots
             ORDER BY account_fingerprint",
        )?;
        statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn official_daily_usage(
        &self,
        account_fingerprint: &str,
        start_day_inclusive: Option<&str>,
        end_day_exclusive: Option<&str>,
    ) -> StoreResult<Vec<OfficialDailyUsageBucket>> {
        let mut predicates = vec!["account_fingerprint = ?".to_owned()];
        let mut parameters = vec![SqlValue::Text(account_fingerprint.to_owned())];
        if let Some(start) = start_day_inclusive {
            predicates.push("local_day >= ?".to_owned());
            parameters.push(SqlValue::Text(start.to_owned()));
        }
        if let Some(end) = end_day_exclusive {
            predicates.push("local_day < ?".to_owned());
            parameters.push(SqlValue::Text(end.to_owned()));
        }
        let sql = format!(
            "SELECT local_day, total_tokens FROM official_daily_usage
             WHERE {} ORDER BY local_day",
            predicates.join(" AND ")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(parameters), |row| {
            Ok(OfficialDailyUsageBucket {
                start_date: row.get(0)?,
                tokens: u64_from_sql(row.get(1)?, 1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn official_usage_sync_state(
        &self,
        account_fingerprint: &str,
    ) -> StoreResult<Option<OfficialUsageSyncState>> {
        self.connection
            .query_row(
                "SELECT last_attempt_at, last_success_at, last_error
                 FROM official_usage_sync_state WHERE account_fingerprint = ?1",
                params![account_fingerprint],
                |row| {
                    let attempt: String = row.get(0)?;
                    let success: Option<String> = row.get(1)?;
                    Ok(OfficialUsageSyncState {
                        last_attempt_at: parse_timestamp_column(attempt, 0)?,
                        last_success_at: success
                            .map(|value| parse_timestamp_column(value, 1))
                            .transpose()?,
                        last_error: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn upsert_official_thread_usage(
        &mut self,
        account_fingerprint: &str,
        observed_at: DateTime<Utc>,
        usage: &OfficialThreadUsage,
    ) -> StoreResult<()> {
        self.connection.execute(
            "INSERT INTO official_thread_usage(
                 account_fingerprint, thread_id, observed_at, normalized_json
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(account_fingerprint, thread_id) DO UPDATE SET
                 observed_at = excluded.observed_at,
                 normalized_json = excluded.normalized_json",
            params![
                account_fingerprint,
                usage.thread_id,
                timestamp(observed_at),
                serde_json::to_string(usage)?,
            ],
        )?;
        Ok(())
    }

    pub fn latest_official_thread_usage(
        &self,
        account_fingerprint: &str,
        thread_id: &str,
    ) -> StoreResult<Option<StoredOfficialThreadUsage>> {
        self.connection
            .query_row(
                "SELECT account_fingerprint, thread_id, observed_at, normalized_json
                 FROM official_thread_usage
                 WHERE account_fingerprint = ?1 AND thread_id = ?2",
                params![account_fingerprint, thread_id],
                |row| {
                    let observed_at: String = row.get(2)?;
                    let normalized_json: String = row.get(3)?;
                    let usage = serde_json::from_str(&normalized_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    Ok(StoredOfficialThreadUsage {
                        account_fingerprint: row.get(0)?,
                        thread_id: row.get(1)?,
                        observed_at: parse_timestamp_column(observed_at, 2)?,
                        usage,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }
}
