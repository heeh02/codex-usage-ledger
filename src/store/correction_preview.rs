//! Separate review artifact. Never attached to production ledger/query views.
use super::*;
use anyhow::{Result, anyhow};

const APP_ID: i64 = 0x43554c50;

#[cfg(test)]
#[path = "correction_preview_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionPreviewGrain {
    #[default]
    Day,
    Week,
    Month,
    Year,
}
impl std::str::FromStr for CorrectionPreviewGrain {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "day" => Ok(Self::Day),
            "week" => Ok(Self::Week),
            "month" => Ok(Self::Month),
            "year" => Ok(Self::Year),
            _ => Err("grain must be day, week, month or year".into()),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionPreviewFilter {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub timezone: String,
    pub grain: CorrectionPreviewGrain,
    pub account: Option<String>,
    pub project: Option<String>,
    pub model: Option<String>,
    pub thread: Option<String>,
}
impl Default for CorrectionPreviewFilter {
    fn default() -> Self {
        Self {
            start: None,
            end: None,
            timezone: "Asia/Shanghai".into(),
            grain: CorrectionPreviewGrain::Day,
            account: None,
            project: None,
            model: None,
            thread: None,
        }
    }
}
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewBucket {
    key: Option<String>,
    events: u64,
    usage: Option<TokenUsage>,
}
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSide {
    events: u64,
    usage: Option<TokenUsage>,
    by_time: Vec<PreviewBucket>,
    by_account: Vec<PreviewBucket>,
    by_project: Vec<PreviewBucket>,
    by_model: Vec<PreviewBucket>,
    by_thread: Vec<PreviewBucket>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionPreview {
    scope: &'static str,
    manifest_body_sha256: String,
    filter: CorrectionPreviewFilter,
    old: PreviewSide,
    candidate: PreviewSide,
    production_policy_changed: bool,
    migration_authorized: bool,
}

pub fn create_correction_preview(
    manifest: &Path,
    db: &Path,
    output: &Path,
) -> Result<CorrectionPreview> {
    let source = LedgerStore::open_reconstruction_audit(db)?;
    let mut preview = new_preview(output)?;
    let transaction = preview.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let checked = source.with_source_audit_snapshot(|source| {
        crate::reconstruction::visit_correction_records(manifest, source, |row| {
            transaction
                .prepare_cached("INSERT INTO preview_changes VALUES(?1,?2,?3,?4)")?
                .execute(params![
                    sql_u64(row.byte_offset, "preview byte offset")?,
                    row.source_json_digest,
                    row.change,
                    row.suppression_rule
                ])?;
            for (side, fact) in [("old", &row.stored), ("candidate", &row.proposed)] {
                if let Some(fact) = fact {
                    insert_fact(&transaction, side, row.byte_offset, fact)?;
                }
            }
            Ok(())
        })
    })?;
    if !checked.full_source_scan {
        return Err(anyhow!(
            "correction preview requires a complete stable source scan"
        ));
    }
    let rows: i64 =
        transaction.query_row("SELECT COUNT(*) FROM preview_changes", [], |row| row.get(0))?;
    if rows as u64 != checked.records {
        return Err(anyhow!("preview record count does not match draft"));
    }
    transaction.execute(
        "UPDATE preview_meta SET ready=1,manifest_sha256=?1 WHERE id=1",
        [checked.body_sha256],
    )?;
    transaction.commit()?;
    drop(preview);
    read_correction_preview(output, &CorrectionPreviewFilter::default())
}

fn new_preview(output: &Path) -> Result<Connection> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // Reserve the new artifact. Existing files/symlinks can never be replaced.
    drop(options.open(output)?);
    let preview = Connection::open_with_flags(output, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    preview.pragma_update(None, "trusted_schema", "OFF")?;
    preview.execute_batch("PRAGMA page_size=4096; PRAGMA max_page_count=65536;")?;
    preview.pragma_update(None, "application_id", APP_ID)?;
    preview.pragma_update(None, "user_version", 1)?;
    preview.execute_batch("CREATE TABLE preview_meta(id INTEGER PRIMARY KEY CHECK(id=1),ready INTEGER NOT NULL,manifest_sha256 TEXT);
        INSERT INTO preview_meta VALUES(1,0,NULL);
        CREATE TABLE preview_changes(byte_offset INTEGER PRIMARY KEY,source_json_digest TEXT NOT NULL,action TEXT NOT NULL,reason TEXT);
        CREATE TABLE preview_facts(side TEXT NOT NULL CHECK(side IN ('old','candidate')),event_id TEXT NOT NULL,byte_offset INTEGER NOT NULL,
            at TEXT NOT NULL,thread_id TEXT,model TEXT,account_id TEXT,project_id TEXT,stored_hash TEXT,record_key TEXT,
            input_tokens INTEGER NOT NULL,cached_input_tokens INTEGER NOT NULL,cache_write_input_tokens INTEGER NOT NULL,
            cache_write_observed_input_tokens INTEGER NOT NULL,output_tokens INTEGER NOT NULL,reasoning_output_tokens INTEGER NOT NULL,total_tokens INTEGER NOT NULL,
            PRIMARY KEY(side,event_id));
        CREATE INDEX preview_time ON preview_facts(side,at);
        CREATE INDEX preview_account ON preview_facts(side,account_id,at);
        CREATE INDEX preview_project ON preview_facts(side,project_id,at);
        CREATE INDEX preview_model ON preview_facts(side,model,at);")?;
    Ok(preview)
}

fn insert_fact(
    connection: &Connection,
    side: &str,
    offset: u64,
    fact: &ReconstructionAuditFact,
) -> Result<()> {
    let u = fact.usage;
    connection.prepare_cached("INSERT INTO preview_facts VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)")?.execute(
        params![side,fact.event_id,sql_u64(offset,"preview byte offset")?,timestamp(fact.at),fact.thread,fact.model,fact.account,fact.project,fact.stored_hash,fact.record_key,
            sql_u64(u.input_tokens,"input")?,sql_u64(u.cached_input_tokens,"cache read")?,sql_u64(u.cache_write_input_tokens,"cache write")?,
            sql_u64(u.cache_write_observed_input_tokens,"write coverage")?,sql_u64(u.output_tokens,"output")?,sql_u64(u.reasoning_output_tokens,"reasoning")?,sql_u64(u.total_tokens,"total")?])?;
    Ok(())
}

pub fn read_correction_preview(
    path: &Path,
    filter: &CorrectionPreviewFilter,
) -> Result<CorrectionPreview> {
    let timezone: chrono_tz::Tz = filter
        .timezone
        .parse()
        .map_err(|_| anyhow!("invalid preview timezone"))?;
    if matches!((filter.start,filter.end),(Some(start),Some(end)) if start>=end) {
        return Err(anyhow!("preview start must precede end"));
    }
    let connection = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    connection.pragma_update(None, "query_only", "ON")?;
    connection.pragma_update(None, "trusted_schema", "OFF")?;
    let transaction = connection.unchecked_transaction()?;
    let app: i64 = transaction.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if app != APP_ID || version != 1 {
        return Err(anyhow!("not a supported correction preview"));
    }
    let hash: Option<String> = transaction
        .query_row(
            "SELECT manifest_sha256 FROM preview_meta WHERE id=1 AND ready=1",
            [],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let manifest_body_sha256 = hash.ok_or_else(|| anyhow!("correction preview is incomplete"))?;
    let old = read_side(&transaction, "old", filter, timezone)?;
    let candidate = read_side(&transaction, "candidate", filter, timezone)?;
    transaction.commit()?;
    Ok(CorrectionPreview {
        scope: "single_source_correction_preview",
        manifest_body_sha256,
        filter: filter.clone(),
        old,
        candidate,
        production_policy_changed: false,
        migration_authorized: false,
    })
}

fn read_side(
    connection: &Connection,
    side: &str,
    filter: &CorrectionPreviewFilter,
    timezone: chrono_tz::Tz,
) -> Result<PreviewSide> {
    let mut sql="SELECT at,thread_id,model,account_id,project_id,input_tokens,cached_input_tokens,cache_write_input_tokens,
        cache_write_observed_input_tokens,output_tokens,reasoning_output_tokens,total_tokens FROM preview_facts WHERE side=?".to_owned();
    let mut parameters = vec![SqlValue::Text(side.into())];
    for (column, op, value) in [
        ("at", ">=", filter.start.map(timestamp)),
        ("at", "<", filter.end.map(timestamp)),
        ("account_id", "=", filter.account.clone()),
        ("project_id", "=", filter.project.clone()),
        ("model", "=", filter.model.clone()),
        ("thread_id", "=", filter.thread.clone()),
    ] {
        if let Some(value) = value {
            sql.push_str(&format!(" AND {column}{op}?"));
            parameters.push(SqlValue::Text(value));
        }
    }
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(params_from_iter(parameters))?;
    let mut summary = PreviewSide::default();
    let mut dimensions: [BTreeMap<Option<String>, PreviewBucket>; 5] =
        std::array::from_fn(|_| BTreeMap::new());
    while let Some(row) = rows.next()? {
        let at = parse_timestamp_column(row.get(0)?, 0)?.with_timezone(&timezone);
        let day = at.date_naive();
        let time = match filter.grain {
            CorrectionPreviewGrain::Day => day.to_string(),
            CorrectionPreviewGrain::Week => day
                .checked_sub_signed(ChronoDuration::days(
                    day.weekday().num_days_from_monday() as i64
                ))
                .ok_or_else(|| anyhow!("week is outside supported date range"))?
                .to_string(),
            CorrectionPreviewGrain::Month => format!("{:04}-{:02}", day.year(), day.month()),
            CorrectionPreviewGrain::Year => day.year().to_string(),
        };
        let keys = [
            Some(time),
            row.get(3)?,
            row.get(4)?,
            row.get(2)?,
            row.get(1)?,
        ];
        let usage = TokenUsage {
            input_tokens: u64_from_sql(row.get(5)?, 5)?,
            cached_input_tokens: u64_from_sql(row.get(6)?, 6)?,
            cache_write_input_tokens: u64_from_sql(row.get(7)?, 7)?,
            cache_write_observed_input_tokens: u64_from_sql(row.get(8)?, 8)?,
            output_tokens: u64_from_sql(row.get(9)?, 9)?,
            reasoning_output_tokens: u64_from_sql(row.get(10)?, 10)?,
            total_tokens: u64_from_sql(row.get(11)?, 11)?,
        };
        usage.validate()?;
        summary.events += 1;
        crate::source_union::add(summary.usage.get_or_insert_default(), usage)?;
        for (dimension, key) in dimensions.iter_mut().zip(keys) {
            let bucket = dimension
                .entry(key.clone())
                .or_insert_with(|| PreviewBucket {
                    key,
                    ..Default::default()
                });
            bucket.events += 1;
            crate::source_union::add(bucket.usage.get_or_insert_default(), usage)?;
            if dimension.len() > 10000 {
                return Err(anyhow!("preview has too many buckets; narrow the filter"));
            }
        }
    }
    let [time, account, project, model, thread] = dimensions;
    summary.by_time = time.into_values().collect();
    summary.by_account = account.into_values().collect();
    summary.by_project = project.into_values().collect();
    summary.by_model = model.into_values().collect();
    summary.by_thread = thread.into_values().collect();
    Ok(summary)
}
