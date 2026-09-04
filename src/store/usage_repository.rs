use super::*;

impl LedgerStore {
    fn refresh_effective_source_selection(&self) -> StoreResult<()> {
        let dirty: bool = self.connection.query_row(
            "SELECT dirty FROM effective_source_selection_state WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        if !dirty {
            return Ok(());
        }
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute_batch(
            "DELETE FROM effective_thread_day_source;
             WITH sampling AS (
                 SELECT local_day, thread_key, SUM(total_tokens) AS total_tokens
                 FROM daily_usage_rollups WHERE quality = 'confirmed'
                 GROUP BY local_day, thread_key
             ), reconstructed AS (
                 SELECT local_day, thread_key, SUM(total_tokens) AS total_tokens
                 FROM reconstruction_daily_rollups
                 GROUP BY local_day, thread_key
             ), keys AS (
                 SELECT local_day, thread_key FROM sampling
                 UNION
                 SELECT local_day, thread_key FROM reconstructed
             )
             INSERT INTO effective_thread_day_source(
                 local_day, thread_key, evidence_source,
                 sampling_tokens, reconstruction_tokens
             )
             SELECT keys.local_day, keys.thread_key,
                    CASE WHEN COALESCE(reconstructed.total_tokens, 0) >
                                   COALESCE(sampling.total_tokens, 0)
                         THEN 'reconstruction' ELSE 'sampling' END,
                    COALESCE(sampling.total_tokens, 0),
                    COALESCE(reconstructed.total_tokens, 0)
             FROM keys
             LEFT JOIN sampling USING(local_day, thread_key)
             LEFT JOIN reconstructed USING(local_day, thread_key);
             UPDATE effective_source_selection_state
             SET dirty = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = 1;",
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn aggregate_usage(&self, filter: &AggregateFilter) -> StoreResult<UsageAggregate> {
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let (where_sql, values) = build_filter(filter);
        let table = if uses_effective_source(filter) {
            "effective_usage_events"
        } else {
            "usage_events"
        };
        let sql = format!(
            "SELECT COUNT(*),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS usage_events {where_sql}"
        );
        self.connection
            .query_row(&sql, params_from_iter(values), aggregate_from_row)
            .map_err(StoreError::from)
    }

    pub fn aggregate_rollup_usage(&self, filter: &AggregateFilter) -> StoreResult<UsageAggregate> {
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let (where_sql, values) = build_rollup_filter(filter);
        let table = if uses_effective_source(filter) {
            "effective_daily_usage_rollups"
        } else {
            "daily_usage_rollups"
        };
        let sql = format!(
            "SELECT COALESCE(SUM(event_count), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS daily_usage_rollups {where_sql}"
        );
        self.connection
            .query_row(&sql, params_from_iter(values), aggregate_from_row)
            .map_err(StoreError::from)
    }

    pub fn aggregate_rollup_by(
        &self,
        dimension: AggregateDimension,
        filter: &AggregateFilter,
    ) -> StoreResult<Vec<UsageBucket>> {
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let table = if uses_effective_source(filter) {
            "effective_daily_usage_rollups"
        } else {
            "daily_usage_rollups"
        };
        let (expression, ordering) = match dimension {
            AggregateDimension::Model => ("model_key".to_owned(), "model_key".to_owned()),
            AggregateDimension::Account => ("account_key".to_owned(), "account_key".to_owned()),
            AggregateDimension::Project => (
                classified_project_expression("project_key", "daily_usage_rollups.thread_key"),
                "1".to_owned(),
            ),
            AggregateDimension::Thread => ("thread_key".to_owned(), "thread_key".to_owned()),
            AggregateDimension::Day => ("local_day".to_owned(), "local_day".to_owned()),
            AggregateDimension::Quality => ("quality".to_owned(), "quality".to_owned()),
        };
        let (where_sql, values) = build_rollup_filter(filter);
        let sql = format!(
            "SELECT {expression}, COALESCE(SUM(event_count), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS daily_usage_rollups {where_sql}
             GROUP BY {expression}
             ORDER BY {ordering}"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            let key: String = row.get(0)?;
            Ok(UsageBucket {
                key: (!key.is_empty()).then_some(key),
                event_count: u64_from_sql(row.get(1)?, 1)?,
                usage: TokenUsage {
                    input_tokens: u64_from_sql(row.get(2)?, 2)?,
                    cached_input_tokens: u64_from_sql(row.get(3)?, 3)?,
                    cache_write_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                    cache_write_observed_input_tokens: u64_from_sql(row.get(5)?, 5)?,
                    output_tokens: u64_from_sql(row.get(6)?, 6)?,
                    reasoning_output_tokens: u64_from_sql(row.get(7)?, 7)?,
                    total_tokens: u64_from_sql(row.get(8)?, 8)?,
                },
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn aggregate_time_series(
        &self,
        grain: TimeGrain,
        dimension: Option<AggregateDimension>,
        filter: &AggregateFilter,
    ) -> StoreResult<Vec<UsageSeriesBucket>> {
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let (table, table_alias, time_column, where_sql, values) = match grain {
            TimeGrain::Hour => {
                let (where_sql, values) = build_hourly_filter(filter);
                (
                    if uses_effective_source(filter) {
                        "effective_hourly_usage_rollups"
                    } else {
                        "hourly_usage_rollups"
                    },
                    "hourly_usage_rollups",
                    "local_hour",
                    where_sql,
                    values,
                )
            }
            TimeGrain::Day => {
                let (where_sql, values) = build_rollup_filter(filter);
                (
                    if uses_effective_source(filter) {
                        "effective_daily_usage_rollups"
                    } else {
                        "daily_usage_rollups"
                    },
                    "daily_usage_rollups",
                    "local_day",
                    where_sql,
                    values,
                )
            }
        };
        let dimension_expression = match dimension {
            Some(AggregateDimension::Model) => "model_key".to_owned(),
            Some(AggregateDimension::Account) => "account_key".to_owned(),
            Some(AggregateDimension::Project) => {
                classified_project_expression("project_key", &format!("{table_alias}.thread_key"))
            }
            Some(AggregateDimension::Thread) => "thread_key".to_owned(),
            Some(AggregateDimension::Quality) => "quality".to_owned(),
            Some(AggregateDimension::Day) | None => "''".to_owned(),
        };
        let sql = format!(
            "SELECT {time_column}, {dimension_expression},
                    COALESCE(SUM(event_count), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS {table_alias} {where_sql}
             GROUP BY {time_column}, {dimension_expression}
             ORDER BY {time_column}, {dimension_expression}"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            let dimension_key: String = row.get(1)?;
            Ok(UsageSeriesBucket {
                time_key: row.get(0)?,
                dimension_key: (!dimension_key.is_empty()).then_some(dimension_key),
                event_count: u64_from_sql(row.get(2)?, 2)?,
                usage: TokenUsage {
                    input_tokens: u64_from_sql(row.get(3)?, 3)?,
                    cached_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                    cache_write_input_tokens: u64_from_sql(row.get(5)?, 5)?,
                    cache_write_observed_input_tokens: u64_from_sql(row.get(6)?, 6)?,
                    output_tokens: u64_from_sql(row.get(7)?, 7)?,
                    reasoning_output_tokens: u64_from_sql(row.get(8)?, 8)?,
                    total_tokens: u64_from_sql(row.get(9)?, 9)?,
                },
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Groups retained raw events inside the exact timestamp filter. This is
    /// used for rolling windows whose first and last buckets are partial; day
    /// and hour rollups intentionally cannot represent those boundaries.
    pub fn aggregate_exact_time_series(
        &self,
        grain: TimeGrain,
        dimension: Option<AggregateDimension>,
        filter: &AggregateFilter,
        timezone: &str,
    ) -> StoreResult<Vec<UsageSeriesBucket>> {
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let timezone = timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| StoreError::InvalidTimezone(timezone.to_owned()))?;
        let dimension_expression = match dimension {
            Some(AggregateDimension::Model) => "model".to_owned(),
            Some(AggregateDimension::Account) => "account_fingerprint".to_owned(),
            Some(AggregateDimension::Project) => {
                classified_project_expression("project_id", "usage_events.thread_id")
            }
            Some(AggregateDimension::Thread) => "thread_id".to_owned(),
            Some(AggregateDimension::Quality) => "quality".to_owned(),
            Some(AggregateDimension::Day) | None => "NULL".to_owned(),
        };
        let (where_sql, values) = build_filter(filter);
        let table = if uses_effective_source(filter) {
            "effective_usage_events"
        } else {
            "usage_events"
        };
        let sql = format!(
            "SELECT COALESCE(source_timestamp, observed_at), {dimension_expression},
                    input_tokens, cached_input_tokens, cache_write_input_tokens,
                    cache_write_observed_input_tokens, output_tokens,
                    reasoning_output_tokens, total_tokens
             FROM {table} AS usage_events {where_sql}
             ORDER BY COALESCE(source_timestamp, observed_at), event_id"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let mut rows = statement.query(params_from_iter(values))?;
        let mut buckets = BTreeMap::<(String, Option<String>), UsageAggregate>::new();
        while let Some(row) = rows.next()? {
            let effective_at: String = row.get(0)?;
            let effective_at = parse_timestamp_column(effective_at, 0)?.with_timezone(&timezone);
            let time_key = match grain {
                TimeGrain::Hour => effective_at.format("%Y-%m-%dT%H:00").to_string(),
                TimeGrain::Day => effective_at.date_naive().to_string(),
            };
            let dimension_key: Option<String> = row.get(1)?;
            let usage = TokenUsage {
                input_tokens: u64_from_sql(row.get(2)?, 2)?,
                cached_input_tokens: u64_from_sql(row.get(3)?, 3)?,
                cache_write_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                cache_write_observed_input_tokens: u64_from_sql(row.get(5)?, 5)?,
                output_tokens: u64_from_sql(row.get(6)?, 6)?,
                reasoning_output_tokens: u64_from_sql(row.get(7)?, 7)?,
                total_tokens: u64_from_sql(row.get(8)?, 8)?,
            };
            let bucket = buckets
                .entry((time_key, dimension_key))
                .or_insert(UsageAggregate {
                    event_count: 0,
                    usage: TokenUsage::default(),
                });
            bucket.event_count = bucket
                .event_count
                .checked_add(1)
                .ok_or(StoreError::AggregateOverflow)?;
            checked_add_usage(&mut bucket.usage, usage)?;
        }
        Ok(buckets
            .into_iter()
            .map(|((time_key, dimension_key), aggregate)| UsageSeriesBucket {
                time_key,
                dimension_key,
                event_count: aggregate.event_count,
                usage: aggregate.usage,
            })
            .collect())
    }

    pub fn aggregate_time_series_for_threads(
        &self,
        grain: TimeGrain,
        thread_ids: &[String],
        filter: &AggregateFilter,
    ) -> StoreResult<Vec<UsageSeriesBucket>> {
        if thread_ids.is_empty() {
            return Ok(Vec::new());
        }
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let (table, table_alias, time_column, mut where_sql, mut values) = match grain {
            TimeGrain::Hour => {
                let (where_sql, values) = build_hourly_filter(filter);
                (
                    if uses_effective_source(filter) {
                        "effective_hourly_usage_rollups"
                    } else {
                        "hourly_usage_rollups"
                    },
                    "hourly_usage_rollups",
                    "local_hour",
                    where_sql,
                    values,
                )
            }
            TimeGrain::Day => {
                let (where_sql, values) = build_rollup_filter(filter);
                (
                    if uses_effective_source(filter) {
                        "effective_daily_usage_rollups"
                    } else {
                        "daily_usage_rollups"
                    },
                    "daily_usage_rollups",
                    "local_day",
                    where_sql,
                    values,
                )
            }
        };
        append_rollup_thread_filter(&mut where_sql, &mut values, thread_ids);
        let sql = format!(
            "SELECT {time_column}, COALESCE(SUM(event_count), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS {table_alias} {where_sql}
             GROUP BY {time_column} ORDER BY {time_column}"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            Ok(UsageSeriesBucket {
                time_key: row.get(0)?,
                dimension_key: None,
                event_count: u64_from_sql(row.get(1)?, 1)?,
                usage: TokenUsage {
                    input_tokens: u64_from_sql(row.get(2)?, 2)?,
                    cached_input_tokens: u64_from_sql(row.get(3)?, 3)?,
                    cache_write_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                    cache_write_observed_input_tokens: u64_from_sql(row.get(5)?, 5)?,
                    output_tokens: u64_from_sql(row.get(6)?, 6)?,
                    reasoning_output_tokens: u64_from_sql(row.get(7)?, 7)?,
                    total_tokens: u64_from_sql(row.get(8)?, 8)?,
                },
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn aggregate_hourly_usage(&self, filter: &AggregateFilter) -> StoreResult<UsageAggregate> {
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let (where_sql, values) = build_hourly_filter(filter);
        let table = if uses_effective_source(filter) {
            "effective_hourly_usage_rollups"
        } else {
            "hourly_usage_rollups"
        };
        let sql = format!(
            "SELECT COALESCE(SUM(event_count), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS hourly_usage_rollups {where_sql}"
        );
        self.connection
            .query_row(&sql, params_from_iter(values), aggregate_from_row)
            .map_err(StoreError::from)
    }

    pub fn aggregate_rollup_usage_for_threads(
        &self,
        thread_ids: &[String],
        filter: &AggregateFilter,
    ) -> StoreResult<UsageAggregate> {
        if thread_ids.is_empty() {
            return Ok(UsageAggregate {
                event_count: 0,
                usage: TokenUsage::default(),
            });
        }
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let (mut where_sql, mut values) = build_rollup_filter(filter);
        append_rollup_thread_filter(&mut where_sql, &mut values, thread_ids);
        let table = if uses_effective_source(filter) {
            "effective_daily_usage_rollups"
        } else {
            "daily_usage_rollups"
        };
        let sql = format!(
            "SELECT COALESCE(SUM(event_count), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS daily_usage_rollups {where_sql}"
        );
        self.connection
            .query_row(&sql, params_from_iter(values), aggregate_from_row)
            .map_err(StoreError::from)
    }

    pub fn aggregate_rollup_by_thread_ids(
        &self,
        thread_ids: &[String],
        filter: &AggregateFilter,
    ) -> StoreResult<Vec<UsageBucket>> {
        if thread_ids.is_empty() {
            return Ok(Vec::new());
        }
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let (mut where_sql, mut values) = build_rollup_filter(filter);
        append_rollup_thread_filter(&mut where_sql, &mut values, thread_ids);
        let table = if uses_effective_source(filter) {
            "effective_daily_usage_rollups"
        } else {
            "daily_usage_rollups"
        };
        let sql = format!(
            "SELECT thread_key, COALESCE(SUM(event_count), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS daily_usage_rollups {where_sql}
             GROUP BY thread_key"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            let key: String = row.get(0)?;
            Ok(UsageBucket {
                key: (!key.is_empty()).then_some(key),
                event_count: u64_from_sql(row.get(1)?, 1)?,
                usage: TokenUsage {
                    input_tokens: u64_from_sql(row.get(2)?, 2)?,
                    cached_input_tokens: u64_from_sql(row.get(3)?, 3)?,
                    cache_write_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                    cache_write_observed_input_tokens: u64_from_sql(row.get(5)?, 5)?,
                    output_tokens: u64_from_sql(row.get(6)?, 6)?,
                    reasoning_output_tokens: u64_from_sql(row.get(7)?, 7)?,
                    total_tokens: u64_from_sql(row.get(8)?, 8)?,
                },
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn aggregate_rollup_by_root_threads(
        &self,
        root_thread_ids: &[String],
        filter: &AggregateFilter,
    ) -> StoreResult<Vec<RootUsageBucket>> {
        if root_thread_ids.is_empty() {
            return Ok(Vec::new());
        }
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let table = if uses_effective_source(filter) {
            "effective_daily_usage_rollups"
        } else {
            "daily_usage_rollups"
        };
        let (mut where_sql, mut values) = build_rollup_filter(filter);
        let placeholders = std::iter::repeat_n("?", root_thread_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        if where_sql.is_empty() {
            where_sql = format!("WHERE membership.root_thread_id IN ({placeholders})");
        } else {
            where_sql.push_str(&format!(
                " AND membership.root_thread_id IN ({placeholders})"
            ));
        }
        values.extend(root_thread_ids.iter().cloned().map(SqlValue::Text));
        let sql = format!(
            "SELECT membership.root_thread_id,
                    (SELECT COUNT(*) FROM thread_root_membership nodes
                     WHERE nodes.root_thread_id = membership.root_thread_id),
                    COALESCE(SUM(CASE WHEN daily_usage_rollups.thread_key = membership.root_thread_id
                                      THEN event_count ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN daily_usage_rollups.thread_key = membership.root_thread_id
                                      THEN input_tokens ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN daily_usage_rollups.thread_key = membership.root_thread_id
                                      THEN cached_input_tokens ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN daily_usage_rollups.thread_key = membership.root_thread_id
                                      THEN cache_write_input_tokens ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN daily_usage_rollups.thread_key = membership.root_thread_id
                                      THEN cache_write_observed_input_tokens ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN daily_usage_rollups.thread_key = membership.root_thread_id
                                      THEN output_tokens ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN daily_usage_rollups.thread_key = membership.root_thread_id
                                      THEN reasoning_output_tokens ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN daily_usage_rollups.thread_key = membership.root_thread_id
                                      THEN total_tokens ELSE 0 END), 0),
                    COALESCE(SUM(event_count), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS daily_usage_rollups
             JOIN thread_root_membership membership
               ON membership.thread_id = daily_usage_rollups.thread_key
             {where_sql}
             GROUP BY membership.root_thread_id"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            Ok(RootUsageBucket {
                root_thread_id: row.get(0)?,
                node_count: u64_from_sql(row.get(1)?, 1)?,
                own: UsageAggregate {
                    event_count: u64_from_sql(row.get(2)?, 2)?,
                    usage: TokenUsage {
                        input_tokens: u64_from_sql(row.get(3)?, 3)?,
                        cached_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                        cache_write_input_tokens: u64_from_sql(row.get(5)?, 5)?,
                        cache_write_observed_input_tokens: u64_from_sql(row.get(6)?, 6)?,
                        output_tokens: u64_from_sql(row.get(7)?, 7)?,
                        reasoning_output_tokens: u64_from_sql(row.get(8)?, 8)?,
                        total_tokens: u64_from_sql(row.get(9)?, 9)?,
                    },
                },
                tree: UsageAggregate {
                    event_count: u64_from_sql(row.get(10)?, 10)?,
                    usage: TokenUsage {
                        input_tokens: u64_from_sql(row.get(11)?, 11)?,
                        cached_input_tokens: u64_from_sql(row.get(12)?, 12)?,
                        cache_write_input_tokens: u64_from_sql(row.get(13)?, 13)?,
                        cache_write_observed_input_tokens: u64_from_sql(row.get(14)?, 14)?,
                        output_tokens: u64_from_sql(row.get(15)?, 15)?,
                        reasoning_output_tokens: u64_from_sql(row.get(16)?, 16)?,
                        total_tokens: u64_from_sql(row.get(17)?, 17)?,
                    },
                },
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn aggregate_usage_for_threads(
        &self,
        thread_ids: &[String],
        filter: &AggregateFilter,
    ) -> StoreResult<UsageAggregate> {
        if thread_ids.is_empty() {
            return Ok(UsageAggregate {
                event_count: 0,
                usage: TokenUsage::default(),
            });
        }
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let (mut where_sql, mut values) = build_filter(filter);
        let table = if uses_effective_source(filter) {
            "effective_usage_events"
        } else {
            "usage_events"
        };
        let placeholders = std::iter::repeat_n("?", thread_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        if where_sql.is_empty() {
            where_sql = format!("WHERE thread_id IN ({placeholders})");
        } else {
            where_sql.push_str(&format!(" AND thread_id IN ({placeholders})"));
        }
        values.extend(thread_ids.iter().cloned().map(SqlValue::Text));
        let sql = format!(
            "SELECT COUNT(*),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS usage_events {where_sql}"
        );
        self.connection
            .query_row(&sql, params_from_iter(values), aggregate_from_row)
            .map_err(StoreError::from)
    }

    pub fn aggregate_by_thread_ids(
        &self,
        thread_ids: &[String],
        filter: &AggregateFilter,
    ) -> StoreResult<Vec<UsageBucket>> {
        if thread_ids.is_empty() {
            return Ok(Vec::new());
        }
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let (mut where_sql, mut values) = build_filter(filter);
        let table = if uses_effective_source(filter) {
            "effective_usage_events"
        } else {
            "usage_events"
        };
        let placeholders = std::iter::repeat_n("?", thread_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        if where_sql.is_empty() {
            where_sql = format!("WHERE thread_id IN ({placeholders})");
        } else {
            where_sql.push_str(&format!(" AND thread_id IN ({placeholders})"));
        }
        values.extend(thread_ids.iter().cloned().map(SqlValue::Text));
        let sql = format!(
            "SELECT thread_id, COUNT(*),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS usage_events {where_sql}
             GROUP BY thread_id"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            let event_count: i64 = row.get(1)?;
            Ok(UsageBucket {
                key: row.get(0)?,
                event_count: u64_from_sql(event_count, 1)?,
                usage: TokenUsage {
                    input_tokens: u64_from_sql(row.get(2)?, 2)?,
                    cached_input_tokens: u64_from_sql(row.get(3)?, 3)?,
                    cache_write_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                    cache_write_observed_input_tokens: u64_from_sql(row.get(5)?, 5)?,
                    output_tokens: u64_from_sql(row.get(6)?, 6)?,
                    reasoning_output_tokens: u64_from_sql(row.get(7)?, 7)?,
                    total_tokens: u64_from_sql(row.get(8)?, 8)?,
                },
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn aggregate_by(
        &self,
        dimension: AggregateDimension,
        filter: &AggregateFilter,
    ) -> StoreResult<Vec<UsageBucket>> {
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let table = if uses_effective_source(filter) {
            "effective_usage_events"
        } else {
            "usage_events"
        };
        let (expression, ordering) = match dimension {
            AggregateDimension::Model => ("model".to_owned(), "model IS NULL, model".to_owned()),
            AggregateDimension::Account => (
                "account_fingerprint".to_owned(),
                "account_fingerprint IS NULL, account_fingerprint".to_owned(),
            ),
            AggregateDimension::Project => (
                classified_project_expression("project_id", "usage_events.thread_id"),
                "1".to_owned(),
            ),
            AggregateDimension::Thread => (
                "thread_id".to_owned(),
                "thread_id IS NULL, thread_id".to_owned(),
            ),
            AggregateDimension::Day => (
                "substr(COALESCE(source_timestamp, observed_at), 1, 10)".to_owned(),
                "substr(COALESCE(source_timestamp, observed_at), 1, 10)".to_owned(),
            ),
            AggregateDimension::Quality => ("quality".to_owned(), "quality".to_owned()),
        };
        let (where_sql, values) = build_filter(filter);
        let sql = format!(
            "SELECT {expression}, COUNT(*),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(cached_input_tokens), 0),
                    COALESCE(SUM(cache_write_input_tokens), 0),
                    COALESCE(SUM(cache_write_observed_input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(reasoning_output_tokens), 0),
                    COALESCE(SUM(total_tokens), 0)
             FROM {table} AS usage_events {where_sql}
             GROUP BY {expression}
             ORDER BY {ordering}"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            let event_count: i64 = row.get(1)?;
            Ok(UsageBucket {
                key: row.get(0)?,
                event_count: u64_from_sql(event_count, 1)?,
                usage: TokenUsage {
                    input_tokens: u64_from_sql(row.get(2)?, 2)?,
                    cached_input_tokens: u64_from_sql(row.get(3)?, 3)?,
                    cache_write_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                    cache_write_observed_input_tokens: u64_from_sql(row.get(5)?, 5)?,
                    output_tokens: u64_from_sql(row.get(6)?, 6)?,
                    reasoning_output_tokens: u64_from_sql(row.get(7)?, 7)?,
                    total_tokens: u64_from_sql(row.get(8)?, 8)?,
                },
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Groups by source-time calendar day in an IANA timezone. Rust performs
    /// the conversion so daylight-saving transitions follow the timezone
    /// database rather than SQLite's fixed-offset date modifiers.
    pub fn aggregate_by_day(
        &self,
        filter: &AggregateFilter,
        timezone: &str,
    ) -> StoreResult<Vec<UsageBucket>> {
        if uses_effective_source(filter) {
            self.refresh_effective_source_selection()?;
        }
        let timezone = timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| StoreError::InvalidTimezone(timezone.to_owned()))?;
        let (where_sql, values) = build_filter(filter);
        let table = if uses_effective_source(filter) {
            "effective_usage_events"
        } else {
            "usage_events"
        };
        let sql = format!(
            "SELECT COALESCE(source_timestamp, observed_at),
                    input_tokens, cached_input_tokens, cache_write_input_tokens,
                    cache_write_observed_input_tokens, output_tokens,
                    reasoning_output_tokens, total_tokens
             FROM {table} AS usage_events {where_sql}
             ORDER BY COALESCE(source_timestamp, observed_at), event_id"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let mut rows = statement.query(params_from_iter(values))?;
        let mut buckets: BTreeMap<String, UsageAggregate> = BTreeMap::new();
        while let Some(row) = rows.next()? {
            let effective_at: String = row.get(0)?;
            let effective_at = parse_timestamp_column(effective_at, 0)?;
            let day = effective_at
                .with_timezone(&timezone)
                .date_naive()
                .to_string();
            let usage = TokenUsage {
                input_tokens: u64_from_sql(row.get(1)?, 1)?,
                cached_input_tokens: u64_from_sql(row.get(2)?, 2)?,
                cache_write_input_tokens: u64_from_sql(row.get(3)?, 3)?,
                cache_write_observed_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                output_tokens: u64_from_sql(row.get(5)?, 5)?,
                reasoning_output_tokens: u64_from_sql(row.get(6)?, 6)?,
                total_tokens: u64_from_sql(row.get(7)?, 7)?,
            };
            let bucket = buckets.entry(day).or_insert(UsageAggregate {
                event_count: 0,
                usage: TokenUsage::default(),
            });
            bucket.event_count = bucket
                .event_count
                .checked_add(1)
                .ok_or(StoreError::AggregateOverflow)?;
            checked_add_usage(&mut bucket.usage, usage)?;
        }
        Ok(buckets
            .into_iter()
            .map(|(key, aggregate)| UsageBucket {
                key: Some(key),
                event_count: aggregate.event_count,
                usage: aggregate.usage,
            })
            .collect())
    }
}
