//! Read a stable staged selection. No source import or active-policy mutation.
use super::*;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceUnionGrain {
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl std::str::FromStr for SourceUnionGrain {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "hour" => Ok(Self::Hour),
            "day" => Ok(Self::Day),
            "week" => Ok(Self::Week),
            "month" => Ok(Self::Month),
            "year" => Ok(Self::Year),
            _ => Err("grain must be hour, day, week, month or year".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceUnionQuery {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub timezone: String,
    pub grain: SourceUnionGrain,
    pub account: Option<String>,
    pub project: Option<String>,
    pub model: Option<String>,
    pub thread: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceUnionBucket {
    pub key: Option<String>,
    pub records: u64,
    pub usage: TokenUsage,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceUnionData {
    pub records: u64,
    pub usage: Option<TokenUsage>,
    pub by_time: Vec<SourceUnionBucket>,
    pub by_account: Vec<SourceUnionBucket>,
    pub by_project: Vec<SourceUnionBucket>,
    pub by_model: Vec<SourceUnionBucket>,
    pub by_thread: Vec<SourceUnionBucket>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceUnionSnapshot {
    pub version: u32,
    pub scope: &'static str,
    pub status: &'static str,
    pub query: SourceUnionQuery,
    pub projection: union_projection::UnionProjectionProgress,
    pub data: Option<SourceUnionData>,
    pub history_complete: bool,
    pub production_policy_changed: bool,
}

impl LedgerStore {
    pub fn read_source_union_projection(
        &self,
        query: &SourceUnionQuery,
    ) -> StoreResult<SourceUnionSnapshot> {
        if query.start >= query.end {
            return Err(StoreError::InvalidRequestQuery(
                "union start must precede end",
            ));
        }
        let timezone = query
            .timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| StoreError::InvalidTimezone(query.timezone.clone()))?;
        let transaction = self.connection.unchecked_transaction()?;
        let projection = union_projection::progress(&transaction, 0, 0)?;
        let status = if !projection.projection_ready || projection.policy_version != 2 {
            "pending"
        } else if projection.unresolved_groups > 0 {
            "unresolved"
        } else {
            "available"
        };
        let data = if status == "available" {
            Some(read_selected(&transaction, query, timezone)?)
        } else {
            None
        };
        let status = if data.as_ref().is_some_and(|data| data.records == 0) {
            "no_records"
        } else {
            status
        };
        transaction.commit()?;
        Ok(SourceUnionSnapshot {
            version: 1,
            scope: "staged_local_measurements_not_inference_usage",
            status,
            query: query.clone(),
            projection,
            data,
            history_complete: false,
            production_policy_changed: false,
        })
    }
}

fn query_sql(query: &SourceUnionQuery) -> (String, Vec<SqlValue>) {
    let mut sql = "SELECT effective_at,account_fingerprint,project_id,model,thread_id,input_tokens,cached_input_tokens,
        cache_write_input_tokens,cache_write_observed_input_tokens,output_tokens,reasoning_output_tokens,total_tokens
        FROM measurement_union_selected WHERE effective_at>=? AND effective_at<?".to_owned();
    let mut values = vec![
        SqlValue::Text(timestamp(query.start)),
        SqlValue::Text(timestamp(query.end)),
    ];
    for (column, value) in [
        ("account_fingerprint", &query.account),
        ("project_id", &query.project),
        ("model", &query.model),
        ("thread_id", &query.thread),
    ] {
        if let Some(value) = value {
            sql.push_str(&format!(" AND {column}=?"));
            values.push(SqlValue::Text(value.clone()));
        }
    }
    (sql, values)
}

fn read_selected(
    connection: &Connection,
    query: &SourceUnionQuery,
    timezone: chrono_tz::Tz,
) -> StoreResult<SourceUnionData> {
    let (sql, parameters) = query_sql(query);
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(params_from_iter(parameters))?;
    let mut result = SourceUnionData::default();
    let mut dimensions: [BTreeMap<Option<String>, SourceUnionBucket>; 5] =
        std::array::from_fn(|_| BTreeMap::new());
    while let Some(row) = rows.next()? {
        let at = parse_timestamp_column(row.get(0)?, 0)?.with_timezone(&timezone);
        let day = at.date_naive();
        let time = match query.grain {
            SourceUnionGrain::Hour => at.format("%Y-%m-%dT%H:00:00%:z").to_string(),
            SourceUnionGrain::Day => day.to_string(),
            SourceUnionGrain::Week => day
                .checked_sub_signed(ChronoDuration::days(
                    day.weekday().num_days_from_monday() as i64
                ))
                .ok_or(StoreError::InvalidRequestQuery(
                    "union week outside supported dates",
                ))?
                .to_string(),
            SourceUnionGrain::Month => format!("{:04}-{:02}", day.year(), day.month()),
            SourceUnionGrain::Year => day.year().to_string(),
        };
        let usage = TokenUsage {
            input_tokens: u64_from_sql(row.get(5)?, 5)?,
            cached_input_tokens: u64_from_sql(row.get(6)?, 6)?,
            cache_write_input_tokens: u64_from_sql(row.get(7)?, 7)?,
            cache_write_observed_input_tokens: u64_from_sql(row.get(8)?, 8)?,
            output_tokens: u64_from_sql(row.get(9)?, 9)?,
            reasoning_output_tokens: u64_from_sql(row.get(10)?, 10)?,
            total_tokens: u64_from_sql(row.get(11)?, 11)?,
        };
        if usage.validate().is_err() {
            return Err(StoreError::InvalidRequestQuery(
                "invalid selected union amounts",
            ));
        }
        result.records = result
            .records
            .checked_add(1)
            .ok_or(StoreError::AggregateOverflow)?;
        checked_add_usage(result.usage.get_or_insert_default(), usage)?;
        let keys = [
            Some(time),
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
        ];
        for (dimension, key) in dimensions.iter_mut().zip(keys) {
            let bucket = dimension
                .entry(key.clone())
                .or_insert_with(|| SourceUnionBucket {
                    key,
                    ..Default::default()
                });
            bucket.records = bucket
                .records
                .checked_add(1)
                .ok_or(StoreError::AggregateOverflow)?;
            checked_add_usage(&mut bucket.usage, usage)?;
            if dimension.len() > 10000 {
                return Err(StoreError::InvalidRequestQuery(
                    "too many union buckets; narrow scope",
                ));
            }
        }
    }
    let [time, account, project, model, thread] = dimensions;
    result.by_time = time.into_values().collect();
    result.by_account = account.into_values().collect();
    result.by_project = project.into_values().collect();
    result.by_model = model.into_values().collect();
    result.by_thread = thread.into_values().collect();
    Ok(result)
}

#[cfg(test)]
#[path = "union_query_tests.rs"]
mod tests;
