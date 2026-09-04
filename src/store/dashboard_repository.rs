use super::*;

fn standalone_catalog_membership(thread_column: &str) -> String {
    format!(
        "EXISTS (
             SELECT 1 FROM standalone_thread_membership standalone
             WHERE standalone.thread_id = {thread_column}
         )"
    )
}

fn dashboard_catalog_thread_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<DashboardCatalogThread> {
    Ok(DashboardCatalogThread {
        thread_id: row.get(0)?,
        parent_thread_id: row.get(1)?,
        project_id: row.get(2)?,
        project_name: row.get(3)?,
        title: row.get(4)?,
        model: row.get(5)?,
        agent_nickname: row.get(6)?,
        agent_role: row.get(7)?,
        agent_path: row.get(8)?,
        depth: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        archived: row.get(12)?,
        has_user_event: row.get(13)?,
        source_kind: row.get(14)?,
        present_in_codex: row.get(15)?,
    })
}

fn dashboard_catalog_counts_from_row(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<DashboardCatalogCounts> {
    let value = |index| -> rusqlite::Result<usize> {
        Ok(row
            .get::<_, Option<i64>>(offset + index)?
            .unwrap_or_default()
            .max(0) as usize)
    };
    Ok(DashboardCatalogCounts {
        current_sessions: value(0)?,
        current_subagents: value(1)?,
        current_orphan_subagents: value(2)?,
        historical_sessions: value(3)?,
        historical_subagents: value(4)?,
    })
}

impl LedgerStore {
    pub(crate) fn database_path(&self) -> Option<PathBuf> {
        self.connection.path().map(PathBuf::from)
    }

    pub fn dashboard_revision(&self) -> StoreResult<String> {
        let event_row: Option<i64> =
            self.connection
                .query_row("SELECT MAX(rowid) FROM usage_events", [], |row| row.get(0))?;
        let (quota_count, quota_time): (i64, Option<String>) = self.connection.query_row(
            "SELECT COUNT(*), MAX(observed_at) FROM quota_snapshots",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let catalog_time: Option<String> =
            self.connection
                .query_row("SELECT MAX(updated_at) FROM thread_catalog", [], |row| {
                    row.get(0)
                })?;
        let collector_time: Option<String> = self.connection.query_row(
            "SELECT updated_at FROM collector_status WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        let rollup_marker: Option<String> = self.connection.query_row(
            "SELECT printf('%d:%d:%d', last_backfilled_rowid, target_rowid, complete)
             FROM rollup_state WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        let official_time: Option<String> = self.connection.query_row(
            "SELECT MAX(observed_at) FROM official_account_usage_snapshots",
            [],
            |row| row.get(0),
        )?;
        Ok(format!(
            "{}:{}:{}:{}:{}:{}:{}",
            event_row.unwrap_or_default(),
            quota_count,
            quota_time.unwrap_or_default(),
            catalog_time.unwrap_or_default(),
            collector_time.unwrap_or_default(),
            rollup_marker.unwrap_or_default(),
            official_time.unwrap_or_default(),
        ))
    }

    pub fn earliest_rollup_day(&self) -> StoreResult<Option<String>> {
        self.connection
            .query_row(
                "SELECT MIN(local_day) FROM daily_usage_rollups",
                [],
                |row| row.get(0),
            )
            .map_err(StoreError::from)
    }

    pub fn latest_confirmed_evidence_at(&self) -> StoreResult<Option<String>> {
        let raw: Option<String> = self.connection.query_row(
            "SELECT MAX(COALESCE(source_timestamp, observed_at))
             FROM usage_events WHERE quality = 'confirmed'",
            [],
            |row| row.get(0),
        )?;
        if raw.is_some() {
            return Ok(raw);
        }
        let day: Option<String> = self.connection.query_row(
            "SELECT MAX(local_day) FROM daily_usage_rollups WHERE quality = 'confirmed'",
            [],
            |row| row.get(0),
        )?;
        Ok(day.map(|value| format!("{value}T23:59:59+08:00")))
    }

    pub fn source_cursor_health(&self) -> StoreResult<Vec<SourceCursorHealth>> {
        let mut statement = self.connection.prepare(
            "SELECT machine_id, MAX(updated_at), COUNT(*)
             FROM file_cursors GROUP BY machine_id ORDER BY machine_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(SourceCursorHealth {
                machine_id: row.get(0)?,
                updated_at: row.get(1)?,
                file_count: u64_from_sql(row.get(2)?, 2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn auth_epoch_summary(&self, account: &str) -> StoreResult<AuthEpochSummary> {
        self.connection
            .query_row(
                "SELECT COUNT(*), MIN(observed_from), MAX(COALESCE(observed_to, observed_from))
                 FROM auth_epochs WHERE account_fingerprint = ?1",
                params![account],
                |row| {
                    Ok(AuthEpochSummary {
                        count: u64_from_sql(row.get(0)?, 0)?,
                        first_seen: row.get(1)?,
                        last_seen: row.get(2)?,
                    })
                },
            )
            .map_err(StoreError::from)
    }

    pub fn reconstruction_summary(&self) -> StoreResult<ReconstructionSummary> {
        let mut summary = self.connection.query_row(
            "SELECT
                 COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN status = 'reconstructing' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN status = 'reconstructed' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN status = 'unrecoverable' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(bytes_processed), 0), COALESCE(SUM(bytes_total), 0)
             FROM reconstruction_sources",
            [],
            |row| {
                Ok(ReconstructionSummary {
                    pending: u64_from_sql(row.get(0)?, 0)?,
                    reconstructing: u64_from_sql(row.get(1)?, 1)?,
                    reconstructed: u64_from_sql(row.get(2)?, 2)?,
                    unrecoverable: u64_from_sql(row.get(3)?, 3)?,
                    bytes_processed: u64_from_sql(row.get(4)?, 4)?,
                    bytes_total: u64_from_sql(row.get(5)?, 5)?,
                    selected_tokens: 0,
                })
            },
        )?;
        summary.selected_tokens = self.connection.query_row(
            "SELECT COALESCE(SUM(reconstruction_tokens), 0)
             FROM effective_thread_day_source WHERE evidence_source = 'reconstruction'",
            [],
            |row| u64_from_sql(row.get(0)?, 0),
        )?;
        Ok(summary)
    }

    pub fn quota_accounts(&self) -> StoreResult<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT account_fingerprint FROM quota_snapshots ORDER BY account_fingerprint",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn auth_timeline_rows(&self) -> StoreResult<Vec<AuthTimelineRow>> {
        let mut statement = self.connection.prepare(
            "SELECT epoch_id, machine_id, source_id, observed_from, account_fingerprint, confidence
             FROM auth_epochs ORDER BY machine_id, source_id, observed_from, epoch_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AuthTimelineRow {
                epoch_id: row.get(0)?,
                machine_id: row.get(1)?,
                source_id: row.get(2)?,
                observed_from: row.get(3)?,
                account_fingerprint: row.get(4)?,
                confidence: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn account_workspace_aliases(&self) -> StoreResult<Vec<(String, bool)>> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT account_fingerprint, canonical
             FROM account_workspace_aliases WHERE account_fingerprint <> ''",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn verified_auth_accounts(&self) -> StoreResult<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT account_fingerprint FROM auth_epochs
             WHERE account_fingerprint IS NOT NULL AND confidence = 'verified'",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn project_count(&self) -> StoreResult<u64> {
        self.connection
            .query_row("SELECT COUNT(*) FROM projects", [], |row| {
                u64_from_sql(row.get(0)?, 0)
            })
            .map_err(StoreError::from)
    }

    pub fn active_project_session_counts(
        &self,
        active_since: &str,
    ) -> StoreResult<BTreeMap<String, u64>> {
        let sql = format!(
            "WITH RECURSIVE standalone_threads(thread_id) AS (
                 SELECT thread_id FROM thread_catalog
                 WHERE parent_thread_id IS NULL AND COALESCE(depth, 0) = 0
                   AND project_id IS NULL AND source_kind = 'state_5'
                 UNION
                 SELECT child.thread_id FROM thread_catalog child
                 JOIN standalone_threads parent ON child.parent_thread_id = parent.thread_id
             )
             SELECT CASE WHEN standalone.thread_id IS NOT NULL THEN '{STANDALONE_CONVERSATIONS_PROJECT_ID}'
                         WHEN catalog.project_id IS NULL THEN '{UNASSIGNED_PROJECT_ID}'
                         ELSE catalog.project_id END,
                    COUNT(*)
             FROM thread_catalog catalog
             LEFT JOIN standalone_threads standalone ON standalone.thread_id = catalog.thread_id
             WHERE catalog.present_in_codex = 1 AND catalog.updated_at >= ?1
             GROUP BY 1"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![active_since], |row| {
            Ok((row.get(0)?, u64_from_sql(row.get(1)?, 1)?))
        })?;
        rows.collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(StoreError::from)
    }

    pub fn root_thread_member_counts(
        &self,
        root_ids: &[String],
    ) -> StoreResult<BTreeMap<String, usize>> {
        if root_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let placeholders = std::iter::repeat_n("?", root_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let mut statement = self.connection.prepare(&format!(
            "SELECT root_thread_id, COUNT(*) FROM thread_root_membership
             WHERE root_thread_id IN ({placeholders}) GROUP BY root_thread_id"
        ))?;
        let rows = statement.query_map(
            params_from_iter(root_ids.iter().cloned().map(SqlValue::Text)),
            |row| Ok((row.get(0)?, row.get::<_, i64>(1)?.max(0) as usize)),
        )?;
        rows.collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(StoreError::from)
    }

    pub fn dashboard_catalog_roots(
        &self,
        project_id: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<DashboardCatalogThread>> {
        let mut predicates = vec!["catalog.parent_thread_id IS NULL".to_owned()];
        let mut values = Vec::new();
        if let Some(project_id) = project_id.filter(|value| *value != "all") {
            if project_id == STANDALONE_CONVERSATIONS_PROJECT_ID {
                predicates.push("COALESCE(catalog.depth, 0) = 0".to_owned());
                predicates.push("catalog.project_id IS NULL".to_owned());
                predicates.push("catalog.source_kind = 'state_5'".to_owned());
            } else if project_id == UNASSIGNED_PROJECT_ID {
                predicates.push("0 = 1".to_owned());
            } else {
                predicates.push("catalog.project_id = ?".to_owned());
                values.push(SqlValue::Text(project_id.to_owned()));
            }
        }
        values.push(SqlValue::Integer(limit.clamp(1, 500) as i64));
        let sql = format!(
            "SELECT thread_id, parent_thread_id, project_id, project_name, title, model,
                    agent_nickname, agent_role, agent_path, COALESCE(depth, 0), created_at,
                    updated_at, archived, has_user_event, source_kind, present_in_codex
             FROM thread_catalog catalog WHERE {}
             ORDER BY catalog.present_in_codex DESC, catalog.updated_at DESC, catalog.thread_id DESC LIMIT ?",
            predicates.join(" AND ")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows =
            statement.query_map(params_from_iter(values), dashboard_catalog_thread_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn dashboard_catalog_descendants(
        &self,
        thread_id: &str,
    ) -> StoreResult<Vec<(DashboardCatalogThread, u32)>> {
        let mut statement = self.connection.prepare(
            "WITH RECURSIVE tree(thread_id, relative_depth) AS (
                 SELECT thread_id, 0 FROM thread_catalog WHERE thread_id = ?1
                 UNION
                 SELECT child.thread_id, tree.relative_depth + 1
                 FROM thread_catalog child JOIN tree ON child.parent_thread_id = tree.thread_id
                 WHERE tree.relative_depth < 32
             )
             SELECT catalog.thread_id, catalog.parent_thread_id, catalog.project_id,
                    catalog.project_name, catalog.title, catalog.model, catalog.agent_nickname,
                    catalog.agent_role, catalog.agent_path, COALESCE(catalog.depth, 0),
                    catalog.created_at, catalog.updated_at, catalog.archived,
                    catalog.has_user_event, catalog.source_kind, catalog.present_in_codex,
                    tree.relative_depth
             FROM tree JOIN thread_catalog catalog ON catalog.thread_id = tree.thread_id
             ORDER BY tree.relative_depth, catalog.updated_at DESC, catalog.thread_id",
        )?;
        let rows = statement.query_map(params![thread_id], |row| {
            Ok((dashboard_catalog_thread_from_row(row)?, row.get(16)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn dashboard_catalog_project_summaries(
        &self,
    ) -> StoreResult<BTreeMap<String, (DashboardCatalogCounts, Option<String>)>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT CASE WHEN standalone.thread_id IS NOT NULL THEN '{STANDALONE_CONVERSATIONS_PROJECT_ID}'
                         WHEN catalog.project_id IS NULL THEN '{UNASSIGNED_PROJECT_ID}'
                         ELSE catalog.project_id END AS project_key,
                    SUM(CASE WHEN catalog.present_in_codex = 1 AND catalog.parent_thread_id IS NULL AND COALESCE(catalog.depth, 0) = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN catalog.present_in_codex = 1 AND (catalog.parent_thread_id IS NOT NULL OR COALESCE(catalog.depth, 0) > 0) THEN 1 ELSE 0 END),
                    SUM(CASE WHEN catalog.present_in_codex = 1 AND catalog.parent_thread_id IS NULL AND COALESCE(catalog.depth, 0) > 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN catalog.present_in_codex = 0 AND catalog.parent_thread_id IS NULL AND COALESCE(catalog.depth, 0) = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN catalog.present_in_codex = 0 AND (catalog.parent_thread_id IS NOT NULL OR COALESCE(catalog.depth, 0) > 0) THEN 1 ELSE 0 END),
                    MAX(catalog.updated_at)
             FROM thread_catalog catalog
             LEFT JOIN standalone_thread_membership standalone ON standalone.thread_id = catalog.thread_id
             GROUP BY project_key"
        ))?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get(0)?,
                (dashboard_catalog_counts_from_row(row, 1)?, row.get(6)?),
            ))
        })?;
        rows.collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(StoreError::from)
    }

    pub fn dashboard_catalog_counts(
        &self,
        project_id: Option<&str>,
    ) -> StoreResult<DashboardCatalogCounts> {
        let mut predicates = Vec::new();
        let mut values = Vec::new();
        if let Some(project_id) = project_id.filter(|value| *value != "all") {
            if project_id == STANDALONE_CONVERSATIONS_PROJECT_ID {
                predicates.push(standalone_catalog_membership("catalog.thread_id"));
            } else if project_id == UNASSIGNED_PROJECT_ID {
                predicates.push("0 = 1".to_owned());
            } else {
                predicates.push(format!(
                    "catalog.project_id = ? AND NOT ({})",
                    standalone_catalog_membership("catalog.thread_id")
                ));
                values.push(SqlValue::Text(project_id.to_owned()));
            }
        }
        let where_sql = if predicates.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", predicates.join(" AND "))
        };
        self.connection
            .query_row(
                &format!(
                    "SELECT
                         SUM(CASE WHEN catalog.present_in_codex = 1 AND catalog.parent_thread_id IS NULL AND COALESCE(catalog.depth, 0) = 0 THEN 1 ELSE 0 END),
                         SUM(CASE WHEN catalog.present_in_codex = 1 AND (catalog.parent_thread_id IS NOT NULL OR COALESCE(catalog.depth, 0) > 0) THEN 1 ELSE 0 END),
                         SUM(CASE WHEN catalog.present_in_codex = 1 AND catalog.parent_thread_id IS NULL AND COALESCE(catalog.depth, 0) > 0 THEN 1 ELSE 0 END),
                         SUM(CASE WHEN catalog.present_in_codex = 0 AND catalog.parent_thread_id IS NULL AND COALESCE(catalog.depth, 0) = 0 THEN 1 ELSE 0 END),
                         SUM(CASE WHEN catalog.present_in_codex = 0 AND (catalog.parent_thread_id IS NOT NULL OR COALESCE(catalog.depth, 0) > 0) THEN 1 ELSE 0 END)
                     FROM thread_catalog catalog {where_sql}"
                ),
                params_from_iter(values),
                |row| dashboard_catalog_counts_from_row(row, 0),
            )
            .map_err(StoreError::from)
    }

    pub fn standalone_conversation_stats(&self) -> StoreResult<StandaloneConversationStats> {
        let (current, historical): (i64, i64) = self.connection.query_row(
            "SELECT
                 SUM(CASE WHEN present_in_codex = 1 THEN 1 ELSE 0 END),
                 SUM(CASE WHEN present_in_codex = 0 THEN 1 ELSE 0 END)
             FROM thread_catalog
             WHERE parent_thread_id IS NULL AND COALESCE(depth, 0) = 0
               AND project_id IS NULL AND source_kind = 'state_5'",
            [],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or_default(),
                    row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
                ))
            },
        )?;
        let with_local_evidence: i64 = self.connection.query_row(
            "SELECT COUNT(DISTINCT standalone.root_thread_id)
             FROM standalone_thread_membership standalone
             JOIN effective_daily_usage_rollups usage ON usage.thread_key = standalone.thread_id
             WHERE usage.quality = 'confirmed' AND usage.total_tokens > 0",
            [],
            |row| row.get(0),
        )?;
        Ok(StandaloneConversationStats {
            current: current.max(0) as u64,
            historical: historical.max(0) as u64,
            with_local_evidence: with_local_evidence.max(0) as u64,
        })
    }

    pub fn residual_usage_rows(
        &self,
        start_day: Option<&str>,
        end_day: Option<&str>,
        accounts: &[String],
    ) -> StoreResult<Vec<ResidualUsageRow>> {
        if accounts.is_empty() {
            return Ok(Vec::new());
        }
        let mut predicates = vec!["quality = 'confirmed'".to_owned()];
        let mut parameters = Vec::<SqlValue>::new();
        if let Some(start) = start_day {
            predicates.push("local_day >= ?".to_owned());
            parameters.push(SqlValue::Text(start.to_owned()));
        }
        if let Some(end) = end_day {
            predicates.push("local_day < ?".to_owned());
            parameters.push(SqlValue::Text(end.to_owned()));
        }
        predicates.push(format!(
            "account_key IN ({})",
            std::iter::repeat_n("?", accounts.len())
                .collect::<Vec<_>>()
                .join(",")
        ));
        parameters.extend(accounts.iter().cloned().map(SqlValue::Text));
        let sql = format!(
            "WITH RECURSIVE standalone_threads(thread_id) AS (
                 SELECT thread_id FROM thread_catalog
                 WHERE parent_thread_id IS NULL AND COALESCE(depth, 0) = 0
                   AND project_id IS NULL AND source_kind = 'state_5'
                 UNION
                 SELECT child.thread_id FROM thread_catalog child
                 JOIN standalone_threads parent ON child.parent_thread_id = parent.thread_id
             )
             SELECT local_day, account_key,
                    CASE WHEN standalone.thread_id IS NOT NULL THEN '{STANDALONE_CONVERSATIONS_PROJECT_ID}'
                         WHEN project_key = '' THEN '{UNASSIGNED_PROJECT_ID}'
                         ELSE project_key END,
                    model_key,
                    COALESCE(SUM(event_count), 0), COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0), COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0), COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0), COALESCE(SUM(total_tokens), 0)
             FROM daily_usage_rollups usage
             LEFT JOIN standalone_threads standalone ON standalone.thread_id = usage.thread_key
             WHERE {}
             GROUP BY local_day, account_key, 3, model_key
             ORDER BY local_day, account_key, 3, model_key",
            predicates.join(" AND ")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(parameters), |row| {
            Ok(ResidualUsageRow {
                day: row.get(0)?,
                account: row.get(1)?,
                project: row.get(2)?,
                model: row.get(3)?,
                source_events: u64_from_sql(row.get(4)?, 4)?,
                usage: TokenUsage {
                    input_tokens: u64_from_sql(row.get(5)?, 5)?,
                    cached_input_tokens: u64_from_sql(row.get(6)?, 6)?,
                    cache_write_input_tokens: u64_from_sql(row.get(7)?, 7)?,
                    cache_write_observed_input_tokens: u64_from_sql(row.get(8)?, 8)?,
                    output_tokens: u64_from_sql(row.get(9)?, 9)?,
                    reasoning_output_tokens: u64_from_sql(row.get(10)?, 10)?,
                    total_tokens: u64_from_sql(row.get(11)?, 11)?,
                },
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }
}
