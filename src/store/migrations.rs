use chrono::Utc;
use rusqlite::{Connection, TransactionBehavior, params};

use super::{StoreError, StoreResult, rebuild_reconstruction_rollups_in, timestamp};

pub(super) const CURRENT_SCHEMA_VERSION: i64 = 24;

pub(super) fn migrate(connection: &mut Connection) -> StoreResult<()> {
    let mut version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::SchemaTooNew {
            found: version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    while version < CURRENT_SCHEMA_VERSION {
        let next = version + 1;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        match next {
            1 => transaction.execute_batch(MIGRATION_1)?,
            2 => transaction.execute_batch(MIGRATION_2)?,
            3 => transaction.execute_batch(MIGRATION_3)?,
            4 => transaction.execute_batch(MIGRATION_4)?,
            5 => transaction.execute_batch(MIGRATION_5)?,
            6 => transaction.execute_batch(MIGRATION_6)?,
            7 => transaction.execute_batch(MIGRATION_7)?,
            8 => transaction.execute_batch(MIGRATION_8)?,
            9 => transaction.execute_batch(MIGRATION_9)?,
            10 => transaction.execute_batch(MIGRATION_10)?,
            11 => transaction.execute_batch(MIGRATION_11)?,
            12 => transaction.execute_batch(MIGRATION_12)?,
            13 => transaction.execute_batch(MIGRATION_13)?,
            14 => transaction.execute_batch(MIGRATION_14)?,
            15 => transaction.execute_batch(MIGRATION_15)?,
            16 => transaction.execute_batch(MIGRATION_16)?,
            17 => transaction.execute_batch(MIGRATION_17)?,
            18 => transaction.execute_batch(MIGRATION_18)?,
            19 => transaction.execute_batch(MIGRATION_19)?,
            20 => transaction.execute_batch(MIGRATION_20)?,
            21 => transaction.execute_batch(MIGRATION_21)?,
            22 => transaction.execute_batch(MIGRATION_22)?,
            23 => transaction.execute_batch(MIGRATION_23)?,
            24 => {
                repair_legacy_reconstruction_coverage(&transaction)?;
                audit_persisted_usage_invariants(&transaction)?;
                transaction.execute_batch(MIGRATION_24)?;
            }
            _ => unreachable!("all migrations must be enumerated"),
        }
        transaction.pragma_update(None, "user_version", next)?;
        transaction.execute(
            "INSERT OR REPLACE INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
            params![next, timestamp(Utc::now())],
        )?;
        transaction.commit()?;
        version = next;
    }
    Ok(())
}

fn repair_legacy_reconstruction_coverage(
    transaction: &rusqlite::Transaction<'_>,
) -> StoreResult<()> {
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS migration_repairs (
             repair_id TEXT PRIMARY KEY,
             table_name TEXT NOT NULL,
             row_count INTEGER NOT NULL CHECK(row_count >= 0),
             fields TEXT NOT NULL,
             applied_at TEXT NOT NULL
         ) WITHOUT ROWID;",
    )?;
    let rows: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM reconstruction_usage_events
         WHERE cache_write_observed_input_tokens > input_tokens",
        [],
        |row| row.get(0),
    )?;
    transaction.execute(
        "INSERT OR REPLACE INTO migration_repairs(
             repair_id, table_name, row_count, fields, applied_at
         ) VALUES ('schema24-reconstruction-cache-write-coverage',
                   'reconstruction_usage_events', ?1,
                   'cache_write_observed_input_tokens', ?2)",
        params![rows, timestamp(Utc::now())],
    )?;
    if rows > 0 {
        transaction.execute(
            "UPDATE reconstruction_usage_events
             SET cache_write_observed_input_tokens = input_tokens
             WHERE cache_write_observed_input_tokens > input_tokens",
            [],
        )?;
        rebuild_reconstruction_rollups_in(transaction)?;
    }
    Ok(())
}

fn audit_persisted_usage_invariants(transaction: &rusqlite::Transaction<'_>) -> StoreResult<()> {
    for (table, predicate) in [
        (
            "usage_events",
            "quality = 'confirmed' AND (
                cached_input_tokens > input_tokens
                OR cache_write_input_tokens > input_tokens - cached_input_tokens
                OR cache_write_observed_input_tokens > input_tokens
                OR total_tokens - input_tokens != output_tokens
                OR reasoning_output_tokens > output_tokens
            )",
        ),
        (
            "reconstruction_usage_events",
            "cached_input_tokens > input_tokens
                OR cache_write_input_tokens > input_tokens - cached_input_tokens
                OR cache_write_observed_input_tokens > input_tokens
                OR total_tokens - input_tokens != output_tokens
                OR reasoning_output_tokens > output_tokens",
        ),
        (
            "daily_usage_rollups",
            "quality = 'confirmed' AND (
                cached_input_tokens > input_tokens
                OR cache_write_input_tokens > input_tokens - cached_input_tokens
                OR cache_write_observed_input_tokens > input_tokens
                OR total_tokens - input_tokens != output_tokens
                OR reasoning_output_tokens > output_tokens
            )",
        ),
        (
            "reconstruction_daily_rollups",
            "cached_input_tokens > input_tokens
                OR cache_write_input_tokens > input_tokens - cached_input_tokens
                OR cache_write_observed_input_tokens > input_tokens
                OR total_tokens - input_tokens != output_tokens
                OR reasoning_output_tokens > output_tokens",
        ),
        (
            "reconstruction_hourly_rollups",
            "cached_input_tokens > input_tokens
                OR cache_write_input_tokens > input_tokens - cached_input_tokens
                OR cache_write_observed_input_tokens > input_tokens
                OR total_tokens - input_tokens != output_tokens
                OR reasoning_output_tokens > output_tokens",
        ),
    ] {
        let rows: i64 = transaction.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE {predicate}"),
            [],
            |row| row.get(0),
        )?;
        if rows > 0 {
            return Err(StoreError::PersistedUsageInvariantViolation {
                table,
                rows: rows as u64,
            });
        }
    }
    Ok(())
}

pub(super) const MIGRATION_1: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS usage_events (
    event_id TEXT PRIMARY KEY,
    event_hash TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    source_timestamp TEXT,
    thread_id TEXT,
    parent_thread_id TEXT,
    model TEXT,
    cwd TEXT,
    account_fingerprint TEXT,
    account_confidence TEXT NOT NULL CHECK(account_confidence IN ('verified', 'inferred', 'unknown')),
    project_id TEXT,
    project_name TEXT,
    project_confidence TEXT NOT NULL CHECK(project_confidence IN ('verified', 'inferred', 'unknown')),
    project_method TEXT NOT NULL,
    input_tokens INTEGER NOT NULL CHECK(input_tokens >= 0),
    cached_input_tokens INTEGER NOT NULL CHECK(cached_input_tokens >= 0),
    output_tokens INTEGER NOT NULL CHECK(output_tokens >= 0),
    reasoning_output_tokens INTEGER NOT NULL CHECK(reasoning_output_tokens >= 0),
    total_tokens INTEGER NOT NULL CHECK(total_tokens >= 0),
    quality TEXT NOT NULL CHECK(quality IN ('confirmed', 'quarantined', 'unknown')),
    quality_reason TEXT,
    machine_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    rollout_id TEXT NOT NULL,
    file_identity TEXT NOT NULL,
    byte_offset INTEGER NOT NULL CHECK(byte_offset >= 0),
    line_number INTEGER NOT NULL CHECK(line_number >= 0),
    UNIQUE(machine_id, source_id, file_identity, byte_offset)
);

CREATE TABLE IF NOT EXISTS file_cursors (
    machine_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    file_identity TEXT NOT NULL,
    byte_offset INTEGER NOT NULL CHECK(byte_offset >= 0),
    line_number INTEGER NOT NULL CHECK(line_number >= 0),
    updated_at TEXT NOT NULL,
    PRIMARY KEY(machine_id, source_id)
);
"#;

pub(super) const MIGRATION_2: &str = r#"
CREATE INDEX IF NOT EXISTS usage_events_observed_at_idx ON usage_events(observed_at);
CREATE INDEX IF NOT EXISTS usage_events_account_time_idx ON usage_events(account_fingerprint, observed_at);
CREATE INDEX IF NOT EXISTS usage_events_project_time_idx ON usage_events(project_id, observed_at);
CREATE INDEX IF NOT EXISTS usage_events_model_time_idx ON usage_events(model, observed_at);
CREATE INDEX IF NOT EXISTS usage_events_quality_time_idx ON usage_events(quality, observed_at);
CREATE INDEX IF NOT EXISTS usage_events_rollout_idx ON usage_events(machine_id, rollout_id, byte_offset);
"#;

pub(super) const MIGRATION_3: &str = r#"
CREATE TABLE IF NOT EXISTS auth_epochs (
    epoch_id INTEGER PRIMARY KEY AUTOINCREMENT,
    machine_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    generation TEXT NOT NULL,
    observed_from TEXT NOT NULL,
    observed_to TEXT,
    account_fingerprint TEXT,
    workspace_fingerprint TEXT,
    confidence TEXT NOT NULL CHECK(confidence IN ('verified', 'inferred', 'unknown')),
    CHECK(observed_to IS NULL OR observed_to >= observed_from)
);
CREATE UNIQUE INDEX IF NOT EXISTS auth_epochs_one_open_idx
    ON auth_epochs(machine_id, source_id) WHERE observed_to IS NULL;
CREATE INDEX IF NOT EXISTS auth_epochs_time_idx
    ON auth_epochs(machine_id, source_id, observed_from, observed_to);

CREATE TABLE IF NOT EXISTS quota_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    account_fingerprint TEXT NOT NULL,
    auth_epoch TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('wham_usage', 'token_count_event')),
    normalized_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS quota_snapshots_account_time_idx
    ON quota_snapshots(account_fingerprint, observed_at DESC);
"#;

pub(super) const MIGRATION_4: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
    project_id TEXT PRIMARY KEY,
    project_name TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS project_roots (
    project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
    root TEXT NOT NULL,
    PRIMARY KEY(project_id, root)
);
CREATE INDEX IF NOT EXISTS project_roots_root_idx ON project_roots(root);
CREATE TABLE IF NOT EXISTS project_git_identities (
    project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
    git_identity TEXT NOT NULL,
    PRIMARY KEY(project_id, git_identity)
);
CREATE INDEX IF NOT EXISTS project_git_identity_idx
    ON project_git_identities(git_identity);
CREATE TABLE IF NOT EXISTS manual_assignments (
    assignment_key TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    project_name_override TEXT,
    updated_at TEXT NOT NULL
);
"#;

pub(super) const MIGRATION_5: &str = r#"
ALTER TABLE file_cursors ADD COLUMN parser_state_json TEXT;
"#;

pub(super) const MIGRATION_6: &str = r#"
CREATE INDEX IF NOT EXISTS usage_events_effective_time_idx
    ON usage_events(COALESCE(source_timestamp, observed_at));
CREATE INDEX IF NOT EXISTS usage_events_quality_effective_time_idx
    ON usage_events(quality, COALESCE(source_timestamp, observed_at));
"#;

pub(super) const MIGRATION_7: &str = r#"
CREATE INDEX IF NOT EXISTS file_cursors_identity_idx
    ON file_cursors(machine_id, file_identity, updated_at DESC);
"#;

pub(super) const MIGRATION_8: &str = r#"
CREATE TABLE IF NOT EXISTS thread_catalog (
    thread_id TEXT PRIMARY KEY,
    parent_thread_id TEXT,
    project_id TEXT,
    project_name TEXT,
    title TEXT,
    model TEXT,
    agent_nickname TEXT,
    agent_role TEXT,
    agent_path TEXT,
    depth INTEGER CHECK(depth IS NULL OR depth >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0, 1)),
    has_user_event INTEGER NOT NULL DEFAULT 0 CHECK(has_user_event IN (0, 1)),
    source_kind TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS thread_catalog_parent_idx
    ON thread_catalog(parent_thread_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS thread_catalog_project_idx
    ON thread_catalog(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS thread_catalog_updated_idx
    ON thread_catalog(updated_at DESC);
CREATE INDEX IF NOT EXISTS usage_events_thread_time_idx
    ON usage_events(thread_id, quality, COALESCE(source_timestamp, observed_at));

INSERT OR IGNORE INTO thread_catalog(
    thread_id, parent_thread_id, project_id, project_name, title, model,
    agent_nickname, agent_role, agent_path, depth, created_at, updated_at,
    archived, has_user_event, source_kind
)
SELECT thread_id, MAX(parent_thread_id), MAX(project_id), MAX(project_name), NULL,
       MAX(model), NULL, NULL, NULL,
       CASE WHEN MAX(parent_thread_id) IS NULL THEN 0 ELSE 1 END,
       MIN(COALESCE(source_timestamp, observed_at)),
       MAX(COALESCE(source_timestamp, observed_at)), 0, 0, 'usage_events'
FROM usage_events
WHERE thread_id IS NOT NULL
GROUP BY thread_id;
"#;

pub(super) const MIGRATION_9: &str = r#"
CREATE TABLE IF NOT EXISTS daily_usage_rollups (
    local_day TEXT NOT NULL,
    thread_key TEXT NOT NULL,
    account_key TEXT NOT NULL,
    project_key TEXT NOT NULL,
    model_key TEXT NOT NULL,
    quality TEXT NOT NULL CHECK(quality IN ('confirmed', 'quarantined', 'unknown')),
    event_count INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    reasoning_output_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    PRIMARY KEY(local_day, thread_key, account_key, project_key, model_key, quality)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS daily_rollups_project_day_idx
    ON daily_usage_rollups(project_key, local_day, quality);
CREATE INDEX IF NOT EXISTS daily_rollups_thread_day_idx
    ON daily_usage_rollups(thread_key, local_day, quality);
CREATE INDEX IF NOT EXISTS daily_rollups_account_day_idx
    ON daily_usage_rollups(account_key, local_day, quality);
CREATE INDEX IF NOT EXISTS daily_rollups_model_day_idx
    ON daily_usage_rollups(model_key, local_day, quality);

CREATE TABLE IF NOT EXISTS rollup_state (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    last_backfilled_rowid INTEGER NOT NULL DEFAULT 0,
    target_rowid INTEGER NOT NULL DEFAULT 0,
    complete INTEGER NOT NULL DEFAULT 0 CHECK(complete IN (0, 1)),
    verified_at TEXT,
    updated_at TEXT NOT NULL
);
INSERT OR IGNORE INTO rollup_state(
    id, last_backfilled_rowid, target_rowid, complete, verified_at, updated_at
)
SELECT 1, 0, COALESCE(MAX(rowid), 0),
       CASE WHEN COUNT(*) = 0 THEN 1 ELSE 0 END,
       NULL,
       strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM usage_events;

CREATE TABLE IF NOT EXISTS collector_status (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    mode TEXT NOT NULL,
    phase TEXT NOT NULL,
    items_total INTEGER NOT NULL DEFAULT 0,
    items_completed INTEGER NOT NULL DEFAULT 0,
    bytes_read INTEGER NOT NULL DEFAULT 0,
    events_inserted INTEGER NOT NULL DEFAULT 0,
    message TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rollup_control (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    suppress_delete INTEGER NOT NULL DEFAULT 0 CHECK(suppress_delete IN (0, 1))
);
INSERT OR IGNORE INTO rollup_control(id, suppress_delete) VALUES (1, 0);

-- Old raw facts are removed after verified daily aggregation. Keeping this
-- compact immutable key prevents a cursor reset or archived-file replay from
-- counting the same sampling event twice without retaining the full payload.
CREATE TABLE IF NOT EXISTS compacted_event_keys (
    event_id TEXT PRIMARY KEY,
    event_hash TEXT NOT NULL,
    compacted_at TEXT NOT NULL
) WITHOUT ROWID;

INSERT OR IGNORE INTO collector_status(
    id, mode, phase, items_total, items_completed, bytes_read,
    events_inserted, message, updated_at
) VALUES (
    1, 'serve', 'idle', 0, 0, 0, 0, NULL,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
);

CREATE TRIGGER IF NOT EXISTS usage_events_rollup_insert
AFTER INSERT ON usage_events
BEGIN
    INSERT INTO daily_usage_rollups(
        local_day, thread_key, account_key, project_key, model_key, quality,
        event_count, input_tokens, cached_input_tokens, output_tokens,
        reasoning_output_tokens, total_tokens
    ) VALUES (
        date(COALESCE(NEW.source_timestamp, NEW.observed_at), '+8 hours'),
        COALESCE(NEW.thread_id, ''), COALESCE(NEW.account_fingerprint, ''),
        COALESCE(NEW.project_id, ''), COALESCE(NEW.model, ''), NEW.quality,
        1, NEW.input_tokens, NEW.cached_input_tokens, NEW.output_tokens,
        NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_day, thread_key, account_key, project_key, model_key, quality)
    DO UPDATE SET
        event_count = event_count + excluded.event_count,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;

CREATE TRIGGER IF NOT EXISTS usage_events_rollup_delete
AFTER DELETE ON usage_events
WHEN (SELECT suppress_delete FROM rollup_control WHERE id = 1) = 0
BEGIN
    UPDATE daily_usage_rollups SET
        event_count = event_count - 1,
        input_tokens = input_tokens - OLD.input_tokens,
        cached_input_tokens = cached_input_tokens - OLD.cached_input_tokens,
        output_tokens = output_tokens - OLD.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens - OLD.reasoning_output_tokens,
        total_tokens = total_tokens - OLD.total_tokens
    WHERE local_day = date(COALESCE(OLD.source_timestamp, OLD.observed_at), '+8 hours')
      AND thread_key = COALESCE(OLD.thread_id, '')
      AND account_key = COALESCE(OLD.account_fingerprint, '')
      AND project_key = COALESCE(OLD.project_id, '')
      AND model_key = COALESCE(OLD.model, '')
      AND quality = OLD.quality;
    DELETE FROM daily_usage_rollups WHERE event_count <= 0;
END;

CREATE TRIGGER IF NOT EXISTS usage_events_rollup_update_remove
AFTER UPDATE ON usage_events
BEGIN
    UPDATE daily_usage_rollups SET
        event_count = event_count - 1,
        input_tokens = input_tokens - OLD.input_tokens,
        cached_input_tokens = cached_input_tokens - OLD.cached_input_tokens,
        output_tokens = output_tokens - OLD.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens - OLD.reasoning_output_tokens,
        total_tokens = total_tokens - OLD.total_tokens
    WHERE local_day = date(COALESCE(OLD.source_timestamp, OLD.observed_at), '+8 hours')
      AND thread_key = COALESCE(OLD.thread_id, '')
      AND account_key = COALESCE(OLD.account_fingerprint, '')
      AND project_key = COALESCE(OLD.project_id, '')
      AND model_key = COALESCE(OLD.model, '')
      AND quality = OLD.quality;
    DELETE FROM daily_usage_rollups WHERE event_count <= 0;
    INSERT INTO daily_usage_rollups(
        local_day, thread_key, account_key, project_key, model_key, quality,
        event_count, input_tokens, cached_input_tokens, output_tokens,
        reasoning_output_tokens, total_tokens
    ) VALUES (
        date(COALESCE(NEW.source_timestamp, NEW.observed_at), '+8 hours'),
        COALESCE(NEW.thread_id, ''), COALESCE(NEW.account_fingerprint, ''),
        COALESCE(NEW.project_id, ''), COALESCE(NEW.model, ''), NEW.quality,
        1, NEW.input_tokens, NEW.cached_input_tokens, NEW.output_tokens,
        NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_day, thread_key, account_key, project_key, model_key, quality)
    DO UPDATE SET
        event_count = event_count + excluded.event_count,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;
"#;

pub(super) const MIGRATION_10: &str = r#"
CREATE TABLE IF NOT EXISTS hourly_usage_rollups (
    local_hour TEXT NOT NULL,
    thread_key TEXT NOT NULL,
    account_key TEXT NOT NULL,
    project_key TEXT NOT NULL,
    model_key TEXT NOT NULL,
    quality TEXT NOT NULL CHECK(quality IN ('confirmed', 'quarantined', 'unknown')),
    event_count INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    reasoning_output_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    PRIMARY KEY(local_hour, thread_key, account_key, project_key, model_key, quality)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS hourly_rollups_project_hour_idx
    ON hourly_usage_rollups(project_key, local_hour, quality);
CREATE INDEX IF NOT EXISTS hourly_rollups_thread_hour_idx
    ON hourly_usage_rollups(thread_key, local_hour, quality);
CREATE INDEX IF NOT EXISTS hourly_rollups_account_hour_idx
    ON hourly_usage_rollups(account_key, local_hour, quality);
CREATE INDEX IF NOT EXISTS hourly_rollups_model_hour_idx
    ON hourly_usage_rollups(model_key, local_hour, quality);

INSERT INTO hourly_usage_rollups(
    local_hour, thread_key, account_key, project_key, model_key, quality,
    event_count, input_tokens, cached_input_tokens, output_tokens,
    reasoning_output_tokens, total_tokens
)
SELECT strftime('%Y-%m-%dT%H:00', COALESCE(source_timestamp, observed_at), '+8 hours'),
       COALESCE(thread_id, ''), COALESCE(account_fingerprint, ''),
       COALESCE(project_id, ''), COALESCE(model, ''), quality,
       COUNT(*), SUM(input_tokens), SUM(cached_input_tokens), SUM(output_tokens),
       SUM(reasoning_output_tokens), SUM(total_tokens)
FROM usage_events
GROUP BY 1, 2, 3, 4, 5, 6
ON CONFLICT(local_hour, thread_key, account_key, project_key, model_key, quality)
DO UPDATE SET
    event_count = excluded.event_count,
    input_tokens = excluded.input_tokens,
    cached_input_tokens = excluded.cached_input_tokens,
    output_tokens = excluded.output_tokens,
    reasoning_output_tokens = excluded.reasoning_output_tokens,
    total_tokens = excluded.total_tokens;

CREATE TRIGGER IF NOT EXISTS usage_events_hourly_insert
AFTER INSERT ON usage_events
BEGIN
    INSERT INTO hourly_usage_rollups(
        local_hour, thread_key, account_key, project_key, model_key, quality,
        event_count, input_tokens, cached_input_tokens, output_tokens,
        reasoning_output_tokens, total_tokens
    ) VALUES (
        strftime('%Y-%m-%dT%H:00', COALESCE(NEW.source_timestamp, NEW.observed_at), '+8 hours'),
        COALESCE(NEW.thread_id, ''), COALESCE(NEW.account_fingerprint, ''),
        COALESCE(NEW.project_id, ''), COALESCE(NEW.model, ''), NEW.quality,
        1, NEW.input_tokens, NEW.cached_input_tokens, NEW.output_tokens,
        NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_hour, thread_key, account_key, project_key, model_key, quality)
    DO UPDATE SET
        event_count = event_count + excluded.event_count,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;

CREATE TRIGGER IF NOT EXISTS usage_events_hourly_delete
AFTER DELETE ON usage_events
WHEN (SELECT suppress_delete FROM rollup_control WHERE id = 1) = 0
BEGIN
    UPDATE hourly_usage_rollups SET
        event_count = event_count - 1,
        input_tokens = input_tokens - OLD.input_tokens,
        cached_input_tokens = cached_input_tokens - OLD.cached_input_tokens,
        output_tokens = output_tokens - OLD.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens - OLD.reasoning_output_tokens,
        total_tokens = total_tokens - OLD.total_tokens
    WHERE local_hour = strftime('%Y-%m-%dT%H:00', COALESCE(OLD.source_timestamp, OLD.observed_at), '+8 hours')
      AND thread_key = COALESCE(OLD.thread_id, '')
      AND account_key = COALESCE(OLD.account_fingerprint, '')
      AND project_key = COALESCE(OLD.project_id, '')
      AND model_key = COALESCE(OLD.model, '')
      AND quality = OLD.quality;
    DELETE FROM hourly_usage_rollups WHERE event_count <= 0;
END;

CREATE TRIGGER IF NOT EXISTS usage_events_hourly_update
AFTER UPDATE ON usage_events
BEGIN
    UPDATE hourly_usage_rollups SET
        event_count = event_count - 1,
        input_tokens = input_tokens - OLD.input_tokens,
        cached_input_tokens = cached_input_tokens - OLD.cached_input_tokens,
        output_tokens = output_tokens - OLD.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens - OLD.reasoning_output_tokens,
        total_tokens = total_tokens - OLD.total_tokens
    WHERE local_hour = strftime('%Y-%m-%dT%H:00', COALESCE(OLD.source_timestamp, OLD.observed_at), '+8 hours')
      AND thread_key = COALESCE(OLD.thread_id, '')
      AND account_key = COALESCE(OLD.account_fingerprint, '')
      AND project_key = COALESCE(OLD.project_id, '')
      AND model_key = COALESCE(OLD.model, '')
      AND quality = OLD.quality;
    DELETE FROM hourly_usage_rollups WHERE event_count <= 0;
    INSERT INTO hourly_usage_rollups(
        local_hour, thread_key, account_key, project_key, model_key, quality,
        event_count, input_tokens, cached_input_tokens, output_tokens,
        reasoning_output_tokens, total_tokens
    ) VALUES (
        strftime('%Y-%m-%dT%H:00', COALESCE(NEW.source_timestamp, NEW.observed_at), '+8 hours'),
        COALESCE(NEW.thread_id, ''), COALESCE(NEW.account_fingerprint, ''),
        COALESCE(NEW.project_id, ''), COALESCE(NEW.model, ''), NEW.quality,
        1, NEW.input_tokens, NEW.cached_input_tokens, NEW.output_tokens,
        NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_hour, thread_key, account_key, project_key, model_key, quality)
    DO UPDATE SET
        event_count = event_count + excluded.event_count,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;
"#;

pub(super) const MIGRATION_11: &str = r#"
CREATE TABLE IF NOT EXISTS official_account_usage_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    account_fingerprint TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    normalized_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS official_usage_snapshots_account_time_idx
    ON official_account_usage_snapshots(account_fingerprint, observed_at DESC);

CREATE TABLE IF NOT EXISTS official_daily_usage (
    account_fingerprint TEXT NOT NULL,
    local_day TEXT NOT NULL,
    total_tokens INTEGER NOT NULL CHECK(total_tokens >= 0),
    observed_at TEXT NOT NULL,
    PRIMARY KEY(account_fingerprint, local_day)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS official_daily_usage_day_idx
    ON official_daily_usage(local_day, account_fingerprint);

CREATE TABLE IF NOT EXISTS official_usage_sync_state (
    account_fingerprint TEXT PRIMARY KEY,
    last_attempt_at TEXT NOT NULL,
    last_success_at TEXT,
    last_error TEXT
);
"#;

pub(super) const MIGRATION_12: &str = r#"
CREATE TABLE IF NOT EXISTS official_thread_usage (
    account_fingerprint TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    normalized_json TEXT NOT NULL,
    PRIMARY KEY(account_fingerprint, thread_id)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS official_thread_usage_time_idx
    ON official_thread_usage(observed_at DESC);
"#;

pub(super) const MIGRATION_13: &str = r#"
CREATE TABLE IF NOT EXISTS auth_log_markers (
    machine_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    log_id INTEGER NOT NULL CHECK(log_id >= 0),
    observed_at TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('account_seen', 'logout', 'login_success')),
    workspace_fingerprint TEXT,
    PRIMARY KEY(machine_id, source_id, log_id)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS auth_log_markers_time_idx
    ON auth_log_markers(machine_id, observed_at, log_id);

CREATE TABLE IF NOT EXISTS account_workspace_aliases (
    workspace_fingerprint TEXT PRIMARY KEY,
    account_fingerprint TEXT NOT NULL,
    canonical INTEGER NOT NULL DEFAULT 0 CHECK(canonical IN (0, 1)),
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS account_workspace_aliases_account_idx
    ON account_workspace_aliases(account_fingerprint, canonical);
"#;

pub(super) const MIGRATION_14: &str = r#"
ALTER TABLE thread_catalog
    ADD COLUMN present_in_codex INTEGER NOT NULL DEFAULT 0
    CHECK(present_in_codex IN (0, 1));
UPDATE thread_catalog
SET present_in_codex = CASE WHEN source_kind = 'state_5' THEN 1 ELSE 0 END;
CREATE INDEX IF NOT EXISTS thread_catalog_presence_idx
    ON thread_catalog(present_in_codex, updated_at DESC);
"#;

pub(super) const MIGRATION_15: &str = r#"
CREATE TABLE IF NOT EXISTS account_registry_settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    user_confirmed_total INTEGER NOT NULL
        CHECK(user_confirmed_total BETWEEN 1 AND 64),
    updated_at TEXT NOT NULL
);
"#;

const MIGRATION_16: &str = r#"
ALTER TABLE usage_events
    ADD COLUMN cache_write_input_tokens INTEGER NOT NULL DEFAULT 0
    CHECK(cache_write_input_tokens >= 0);
ALTER TABLE usage_events
    ADD COLUMN cache_write_observed_input_tokens INTEGER NOT NULL DEFAULT 0
    CHECK(cache_write_observed_input_tokens >= 0);
ALTER TABLE daily_usage_rollups
    ADD COLUMN cache_write_input_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE daily_usage_rollups
    ADD COLUMN cache_write_observed_input_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE hourly_usage_rollups
    ADD COLUMN cache_write_input_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE hourly_usage_rollups
    ADD COLUMN cache_write_observed_input_tokens INTEGER NOT NULL DEFAULT 0;

DROP TRIGGER IF EXISTS usage_events_rollup_insert;
DROP TRIGGER IF EXISTS usage_events_rollup_delete;
DROP TRIGGER IF EXISTS usage_events_rollup_update_remove;
DROP TRIGGER IF EXISTS usage_events_hourly_insert;
DROP TRIGGER IF EXISTS usage_events_hourly_delete;
DROP TRIGGER IF EXISTS usage_events_hourly_update;

CREATE TRIGGER usage_events_rollup_insert
AFTER INSERT ON usage_events
BEGIN
    INSERT INTO daily_usage_rollups(
        local_day, thread_key, account_key, project_key, model_key, quality,
        event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
        cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens, total_tokens
    ) VALUES (
        date(COALESCE(NEW.source_timestamp, NEW.observed_at), '+8 hours'),
        COALESCE(NEW.thread_id, ''), COALESCE(NEW.account_fingerprint, ''),
        COALESCE(NEW.project_id, ''), COALESCE(NEW.model, ''), NEW.quality,
        1, NEW.input_tokens, NEW.cached_input_tokens, NEW.cache_write_input_tokens,
        NEW.cache_write_observed_input_tokens, NEW.output_tokens,
        NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_day, thread_key, account_key, project_key, model_key, quality)
    DO UPDATE SET
        event_count = event_count + excluded.event_count,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;

CREATE TRIGGER usage_events_rollup_delete
AFTER DELETE ON usage_events
WHEN (SELECT suppress_delete FROM rollup_control WHERE id = 1) = 0
BEGIN
    UPDATE daily_usage_rollups SET
        event_count = event_count - 1,
        input_tokens = input_tokens - OLD.input_tokens,
        cached_input_tokens = cached_input_tokens - OLD.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens - OLD.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens - OLD.cache_write_observed_input_tokens,
        output_tokens = output_tokens - OLD.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens - OLD.reasoning_output_tokens,
        total_tokens = total_tokens - OLD.total_tokens
    WHERE local_day = date(COALESCE(OLD.source_timestamp, OLD.observed_at), '+8 hours')
      AND thread_key = COALESCE(OLD.thread_id, '')
      AND account_key = COALESCE(OLD.account_fingerprint, '')
      AND project_key = COALESCE(OLD.project_id, '')
      AND model_key = COALESCE(OLD.model, '')
      AND quality = OLD.quality;
    DELETE FROM daily_usage_rollups WHERE event_count <= 0;
END;

CREATE TRIGGER usage_events_rollup_update_remove
AFTER UPDATE ON usage_events
BEGIN
    UPDATE daily_usage_rollups SET
        event_count = event_count - 1,
        input_tokens = input_tokens - OLD.input_tokens,
        cached_input_tokens = cached_input_tokens - OLD.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens - OLD.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens - OLD.cache_write_observed_input_tokens,
        output_tokens = output_tokens - OLD.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens - OLD.reasoning_output_tokens,
        total_tokens = total_tokens - OLD.total_tokens
    WHERE local_day = date(COALESCE(OLD.source_timestamp, OLD.observed_at), '+8 hours')
      AND thread_key = COALESCE(OLD.thread_id, '')
      AND account_key = COALESCE(OLD.account_fingerprint, '')
      AND project_key = COALESCE(OLD.project_id, '')
      AND model_key = COALESCE(OLD.model, '')
      AND quality = OLD.quality;
    DELETE FROM daily_usage_rollups WHERE event_count <= 0;
    INSERT INTO daily_usage_rollups(
        local_day, thread_key, account_key, project_key, model_key, quality,
        event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
        cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens, total_tokens
    ) VALUES (
        date(COALESCE(NEW.source_timestamp, NEW.observed_at), '+8 hours'),
        COALESCE(NEW.thread_id, ''), COALESCE(NEW.account_fingerprint, ''),
        COALESCE(NEW.project_id, ''), COALESCE(NEW.model, ''), NEW.quality,
        1, NEW.input_tokens, NEW.cached_input_tokens, NEW.cache_write_input_tokens,
        NEW.cache_write_observed_input_tokens, NEW.output_tokens,
        NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_day, thread_key, account_key, project_key, model_key, quality)
    DO UPDATE SET
        event_count = event_count + excluded.event_count,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;

CREATE TRIGGER usage_events_hourly_insert
AFTER INSERT ON usage_events
BEGIN
    INSERT INTO hourly_usage_rollups(
        local_hour, thread_key, account_key, project_key, model_key, quality,
        event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
        cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens, total_tokens
    ) VALUES (
        strftime('%Y-%m-%dT%H:00', COALESCE(NEW.source_timestamp, NEW.observed_at), '+8 hours'),
        COALESCE(NEW.thread_id, ''), COALESCE(NEW.account_fingerprint, ''),
        COALESCE(NEW.project_id, ''), COALESCE(NEW.model, ''), NEW.quality,
        1, NEW.input_tokens, NEW.cached_input_tokens, NEW.cache_write_input_tokens,
        NEW.cache_write_observed_input_tokens, NEW.output_tokens,
        NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_hour, thread_key, account_key, project_key, model_key, quality)
    DO UPDATE SET
        event_count = event_count + excluded.event_count,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;

CREATE TRIGGER usage_events_hourly_delete
AFTER DELETE ON usage_events
WHEN (SELECT suppress_delete FROM rollup_control WHERE id = 1) = 0
BEGIN
    UPDATE hourly_usage_rollups SET
        event_count = event_count - 1,
        input_tokens = input_tokens - OLD.input_tokens,
        cached_input_tokens = cached_input_tokens - OLD.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens - OLD.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens - OLD.cache_write_observed_input_tokens,
        output_tokens = output_tokens - OLD.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens - OLD.reasoning_output_tokens,
        total_tokens = total_tokens - OLD.total_tokens
    WHERE local_hour = strftime('%Y-%m-%dT%H:00', COALESCE(OLD.source_timestamp, OLD.observed_at), '+8 hours')
      AND thread_key = COALESCE(OLD.thread_id, '')
      AND account_key = COALESCE(OLD.account_fingerprint, '')
      AND project_key = COALESCE(OLD.project_id, '')
      AND model_key = COALESCE(OLD.model, '')
      AND quality = OLD.quality;
    DELETE FROM hourly_usage_rollups WHERE event_count <= 0;
END;

CREATE TRIGGER usage_events_hourly_update
AFTER UPDATE ON usage_events
BEGIN
    UPDATE hourly_usage_rollups SET
        event_count = event_count - 1,
        input_tokens = input_tokens - OLD.input_tokens,
        cached_input_tokens = cached_input_tokens - OLD.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens - OLD.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens - OLD.cache_write_observed_input_tokens,
        output_tokens = output_tokens - OLD.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens - OLD.reasoning_output_tokens,
        total_tokens = total_tokens - OLD.total_tokens
    WHERE local_hour = strftime('%Y-%m-%dT%H:00', COALESCE(OLD.source_timestamp, OLD.observed_at), '+8 hours')
      AND thread_key = COALESCE(OLD.thread_id, '')
      AND account_key = COALESCE(OLD.account_fingerprint, '')
      AND project_key = COALESCE(OLD.project_id, '')
      AND model_key = COALESCE(OLD.model, '')
      AND quality = OLD.quality;
    DELETE FROM hourly_usage_rollups WHERE event_count <= 0;
    INSERT INTO hourly_usage_rollups(
        local_hour, thread_key, account_key, project_key, model_key, quality,
        event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
        cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens, total_tokens
    ) VALUES (
        strftime('%Y-%m-%dT%H:00', COALESCE(NEW.source_timestamp, NEW.observed_at), '+8 hours'),
        COALESCE(NEW.thread_id, ''), COALESCE(NEW.account_fingerprint, ''),
        COALESCE(NEW.project_id, ''), COALESCE(NEW.model, ''), NEW.quality,
        1, NEW.input_tokens, NEW.cached_input_tokens, NEW.cache_write_input_tokens,
        NEW.cache_write_observed_input_tokens, NEW.output_tokens,
        NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_hour, thread_key, account_key, project_key, model_key, quality)
    DO UPDATE SET
        event_count = event_count + excluded.event_count,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;
"#;

const MIGRATION_17: &str = r#"
CREATE TABLE IF NOT EXISTS reconstruction_usage_events (
    event_id TEXT PRIMARY KEY,
    event_hash TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    source_timestamp TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    parent_thread_id TEXT,
    model TEXT,
    cwd TEXT,
    account_fingerprint TEXT,
    account_confidence TEXT NOT NULL CHECK(account_confidence IN ('verified', 'inferred', 'unknown')),
    project_id TEXT,
    project_name TEXT,
    project_confidence TEXT NOT NULL CHECK(project_confidence IN ('verified', 'inferred', 'unknown')),
    project_method TEXT NOT NULL,
    input_tokens INTEGER NOT NULL CHECK(input_tokens >= 0),
    cached_input_tokens INTEGER NOT NULL CHECK(cached_input_tokens >= 0),
    cache_write_input_tokens INTEGER NOT NULL CHECK(cache_write_input_tokens >= 0),
    cache_write_observed_input_tokens INTEGER NOT NULL CHECK(cache_write_observed_input_tokens >= 0),
    output_tokens INTEGER NOT NULL CHECK(output_tokens >= 0),
    reasoning_output_tokens INTEGER NOT NULL CHECK(reasoning_output_tokens >= 0),
    total_tokens INTEGER NOT NULL CHECK(total_tokens >= 0),
    machine_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    file_identity TEXT NOT NULL,
    byte_offset INTEGER NOT NULL CHECK(byte_offset >= 0),
    line_number INTEGER NOT NULL CHECK(line_number >= 0),
    counter_epoch INTEGER NOT NULL CHECK(counter_epoch >= 0),
    UNIQUE(machine_id, source_id, file_identity, byte_offset)
);
CREATE INDEX IF NOT EXISTS reconstruction_events_time_idx
    ON reconstruction_usage_events(source_timestamp);
CREATE INDEX IF NOT EXISTS reconstruction_events_thread_time_idx
    ON reconstruction_usage_events(thread_id, source_timestamp);
CREATE INDEX IF NOT EXISTS reconstruction_events_project_time_idx
    ON reconstruction_usage_events(project_id, source_timestamp);

CREATE TABLE IF NOT EXISTS reconstruction_daily_rollups (
    local_day TEXT NOT NULL,
    thread_key TEXT NOT NULL,
    account_key TEXT NOT NULL,
    project_key TEXT NOT NULL,
    model_key TEXT NOT NULL,
    event_count INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_input_tokens INTEGER NOT NULL,
    cache_write_input_tokens INTEGER NOT NULL,
    cache_write_observed_input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    reasoning_output_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    PRIMARY KEY(local_day, thread_key, account_key, project_key, model_key)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS reconstruction_daily_project_idx
    ON reconstruction_daily_rollups(project_key, local_day);
CREATE INDEX IF NOT EXISTS reconstruction_daily_thread_idx
    ON reconstruction_daily_rollups(thread_key, local_day);

CREATE TABLE IF NOT EXISTS reconstruction_hourly_rollups (
    local_hour TEXT NOT NULL,
    thread_key TEXT NOT NULL,
    account_key TEXT NOT NULL,
    project_key TEXT NOT NULL,
    model_key TEXT NOT NULL,
    event_count INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_input_tokens INTEGER NOT NULL,
    cache_write_input_tokens INTEGER NOT NULL,
    cache_write_observed_input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    reasoning_output_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    PRIMARY KEY(local_hour, thread_key, account_key, project_key, model_key)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS reconstruction_hourly_project_idx
    ON reconstruction_hourly_rollups(project_key, local_hour);
CREATE INDEX IF NOT EXISTS reconstruction_hourly_thread_idx
    ON reconstruction_hourly_rollups(thread_key, local_hour);

CREATE TABLE IF NOT EXISTS reconstruction_sources (
    machine_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    file_identity TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'reconstructing', 'reconstructed', 'unrecoverable')),
    bytes_total INTEGER NOT NULL CHECK(bytes_total >= 0),
    bytes_processed INTEGER NOT NULL CHECK(bytes_processed >= 0),
    prefix_events INTEGER NOT NULL DEFAULT 0 CHECK(prefix_events >= 0),
    unchanged_events INTEGER NOT NULL DEFAULT 0 CHECK(unchanged_events >= 0),
    counter_resets INTEGER NOT NULL DEFAULT 0 CHECK(counter_resets >= 0),
    last_error TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(machine_id, source_id)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS reconstruction_sources_status_idx
    ON reconstruction_sources(status, updated_at);
CREATE INDEX IF NOT EXISTS reconstruction_sources_thread_idx
    ON reconstruction_sources(thread_id);

CREATE TRIGGER IF NOT EXISTS reconstruction_events_daily_insert
AFTER INSERT ON reconstruction_usage_events
BEGIN
    INSERT INTO reconstruction_daily_rollups(
        local_day, thread_key, account_key, project_key, model_key,
        event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
        cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens, total_tokens
    ) VALUES (
        date(NEW.source_timestamp, '+8 hours'), NEW.thread_id,
        COALESCE(NEW.account_fingerprint, ''), COALESCE(NEW.project_id, ''),
        COALESCE(NEW.model, ''), 1, NEW.input_tokens, NEW.cached_input_tokens,
        NEW.cache_write_input_tokens, NEW.cache_write_observed_input_tokens,
        NEW.output_tokens, NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_day, thread_key, account_key, project_key, model_key)
    DO UPDATE SET
        event_count = event_count + 1,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;

CREATE TRIGGER IF NOT EXISTS reconstruction_events_hourly_insert
AFTER INSERT ON reconstruction_usage_events
BEGIN
    INSERT INTO reconstruction_hourly_rollups(
        local_hour, thread_key, account_key, project_key, model_key,
        event_count, input_tokens, cached_input_tokens, cache_write_input_tokens,
        cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens, total_tokens
    ) VALUES (
        strftime('%Y-%m-%dT%H:00', NEW.source_timestamp, '+8 hours'), NEW.thread_id,
        COALESCE(NEW.account_fingerprint, ''), COALESCE(NEW.project_id, ''),
        COALESCE(NEW.model, ''), 1, NEW.input_tokens, NEW.cached_input_tokens,
        NEW.cache_write_input_tokens, NEW.cache_write_observed_input_tokens,
        NEW.output_tokens, NEW.reasoning_output_tokens, NEW.total_tokens
    )
    ON CONFLICT(local_hour, thread_key, account_key, project_key, model_key)
    DO UPDATE SET
        event_count = event_count + 1,
        input_tokens = input_tokens + excluded.input_tokens,
        cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
        cache_write_input_tokens = cache_write_input_tokens + excluded.cache_write_input_tokens,
        cache_write_observed_input_tokens = cache_write_observed_input_tokens + excluded.cache_write_observed_input_tokens,
        output_tokens = output_tokens + excluded.output_tokens,
        reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
        total_tokens = total_tokens + excluded.total_tokens;
END;

CREATE VIEW effective_thread_day_source AS
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
SELECT keys.local_day, keys.thread_key,
       CASE WHEN COALESCE(reconstructed.total_tokens, 0) > COALESCE(sampling.total_tokens, 0)
            THEN 'reconstruction' ELSE 'sampling' END AS evidence_source,
       COALESCE(sampling.total_tokens, 0) AS sampling_tokens,
       COALESCE(reconstructed.total_tokens, 0) AS reconstruction_tokens
FROM keys
LEFT JOIN sampling USING(local_day, thread_key)
LEFT JOIN reconstructed USING(local_day, thread_key);

CREATE VIEW effective_daily_usage_rollups AS
SELECT sampling.local_day, sampling.thread_key, sampling.account_key,
       sampling.project_key, sampling.model_key, sampling.quality,
       sampling.event_count, sampling.input_tokens, sampling.cached_input_tokens,
       sampling.cache_write_input_tokens, sampling.cache_write_observed_input_tokens,
       sampling.output_tokens, sampling.reasoning_output_tokens, sampling.total_tokens,
       'sampling' AS evidence_source
FROM daily_usage_rollups sampling
JOIN effective_thread_day_source choice
  ON choice.local_day = sampling.local_day AND choice.thread_key = sampling.thread_key
WHERE sampling.quality = 'confirmed' AND choice.evidence_source = 'sampling'
UNION ALL
SELECT reconstructed.local_day, reconstructed.thread_key, reconstructed.account_key,
       reconstructed.project_key, reconstructed.model_key, 'confirmed' AS quality,
       reconstructed.event_count, reconstructed.input_tokens, reconstructed.cached_input_tokens,
       reconstructed.cache_write_input_tokens, reconstructed.cache_write_observed_input_tokens,
       reconstructed.output_tokens, reconstructed.reasoning_output_tokens,
       reconstructed.total_tokens, 'reconstruction' AS evidence_source
FROM reconstruction_daily_rollups reconstructed
JOIN effective_thread_day_source choice
  ON choice.local_day = reconstructed.local_day AND choice.thread_key = reconstructed.thread_key
WHERE choice.evidence_source = 'reconstruction';

CREATE VIEW effective_hourly_usage_rollups AS
SELECT sampling.local_hour, sampling.thread_key, sampling.account_key,
       sampling.project_key, sampling.model_key, sampling.quality,
       sampling.event_count, sampling.input_tokens, sampling.cached_input_tokens,
       sampling.cache_write_input_tokens, sampling.cache_write_observed_input_tokens,
       sampling.output_tokens, sampling.reasoning_output_tokens, sampling.total_tokens,
       'sampling' AS evidence_source
FROM hourly_usage_rollups sampling
JOIN effective_thread_day_source choice
  ON choice.local_day = substr(sampling.local_hour, 1, 10)
 AND choice.thread_key = sampling.thread_key
WHERE sampling.quality = 'confirmed' AND choice.evidence_source = 'sampling'
UNION ALL
SELECT reconstructed.local_hour, reconstructed.thread_key, reconstructed.account_key,
       reconstructed.project_key, reconstructed.model_key, 'confirmed' AS quality,
       reconstructed.event_count, reconstructed.input_tokens, reconstructed.cached_input_tokens,
       reconstructed.cache_write_input_tokens, reconstructed.cache_write_observed_input_tokens,
       reconstructed.output_tokens, reconstructed.reasoning_output_tokens,
       reconstructed.total_tokens, 'reconstruction' AS evidence_source
FROM reconstruction_hourly_rollups reconstructed
JOIN effective_thread_day_source choice
  ON choice.local_day = substr(reconstructed.local_hour, 1, 10)
 AND choice.thread_key = reconstructed.thread_key
WHERE choice.evidence_source = 'reconstruction';

CREATE VIEW effective_usage_events AS
SELECT sampling.event_id, sampling.observed_at, sampling.source_timestamp,
       sampling.thread_id, sampling.parent_thread_id, sampling.model, sampling.cwd,
       sampling.account_fingerprint, sampling.project_id, sampling.input_tokens,
       sampling.cached_input_tokens, sampling.cache_write_input_tokens,
       sampling.cache_write_observed_input_tokens, sampling.output_tokens,
       sampling.reasoning_output_tokens, sampling.total_tokens, sampling.quality
FROM usage_events sampling
JOIN effective_thread_day_source choice
  ON choice.local_day = date(COALESCE(sampling.source_timestamp, sampling.observed_at), '+8 hours')
 AND choice.thread_key = COALESCE(sampling.thread_id, '')
WHERE sampling.quality = 'confirmed' AND choice.evidence_source = 'sampling'
UNION ALL
SELECT reconstructed.event_id, reconstructed.observed_at, reconstructed.source_timestamp,
       reconstructed.thread_id, reconstructed.parent_thread_id, reconstructed.model,
       reconstructed.cwd, reconstructed.account_fingerprint, reconstructed.project_id,
       reconstructed.input_tokens, reconstructed.cached_input_tokens,
       reconstructed.cache_write_input_tokens,
       reconstructed.cache_write_observed_input_tokens, reconstructed.output_tokens,
       reconstructed.reasoning_output_tokens, reconstructed.total_tokens,
       'confirmed' AS quality
FROM reconstruction_usage_events reconstructed
JOIN effective_thread_day_source choice
  ON choice.local_day = date(reconstructed.source_timestamp, '+8 hours')
 AND choice.thread_key = reconstructed.thread_id
WHERE choice.evidence_source = 'reconstruction';
"#;

const MIGRATION_18: &str = r#"
DROP VIEW IF EXISTS effective_usage_events;
DROP VIEW IF EXISTS effective_hourly_usage_rollups;
DROP VIEW IF EXISTS effective_daily_usage_rollups;
DROP VIEW IF EXISTS effective_thread_day_source;

CREATE TABLE effective_thread_day_source (
    local_day TEXT NOT NULL,
    thread_key TEXT NOT NULL,
    evidence_source TEXT NOT NULL CHECK(evidence_source IN ('sampling', 'reconstruction')),
    sampling_tokens INTEGER NOT NULL CHECK(sampling_tokens >= 0),
    reconstruction_tokens INTEGER NOT NULL CHECK(reconstruction_tokens >= 0),
    PRIMARY KEY(local_day, thread_key)
) WITHOUT ROWID;
CREATE INDEX effective_thread_day_source_kind_idx
    ON effective_thread_day_source(evidence_source, local_day);

CREATE TABLE effective_source_selection_state (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    dirty INTEGER NOT NULL CHECK(dirty IN (0, 1)),
    updated_at TEXT NOT NULL
);
INSERT INTO effective_source_selection_state(id, dirty, updated_at)
VALUES (1, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

CREATE TRIGGER effective_sampling_daily_insert
AFTER INSERT ON daily_usage_rollups WHEN NEW.quality = 'confirmed'
BEGIN
    UPDATE effective_source_selection_state SET dirty = 1 WHERE id = 1;
END;
CREATE TRIGGER effective_sampling_daily_update
AFTER UPDATE ON daily_usage_rollups
WHEN OLD.quality = 'confirmed' OR NEW.quality = 'confirmed'
BEGIN
    UPDATE effective_source_selection_state SET dirty = 1 WHERE id = 1;
END;
CREATE TRIGGER effective_sampling_daily_delete
AFTER DELETE ON daily_usage_rollups WHEN OLD.quality = 'confirmed'
BEGIN
    UPDATE effective_source_selection_state SET dirty = 1 WHERE id = 1;
END;
CREATE TRIGGER effective_reconstruction_daily_insert
AFTER INSERT ON reconstruction_daily_rollups
BEGIN
    UPDATE effective_source_selection_state SET dirty = 1 WHERE id = 1;
END;
CREATE TRIGGER effective_reconstruction_daily_update
AFTER UPDATE ON reconstruction_daily_rollups
BEGIN
    UPDATE effective_source_selection_state SET dirty = 1 WHERE id = 1;
END;
CREATE TRIGGER effective_reconstruction_daily_delete
AFTER DELETE ON reconstruction_daily_rollups
BEGIN
    UPDATE effective_source_selection_state SET dirty = 1 WHERE id = 1;
END;

CREATE VIEW effective_daily_usage_rollups AS
SELECT sampling.local_day, sampling.thread_key, sampling.account_key,
       sampling.project_key, sampling.model_key, sampling.quality,
       sampling.event_count, sampling.input_tokens, sampling.cached_input_tokens,
       sampling.cache_write_input_tokens, sampling.cache_write_observed_input_tokens,
       sampling.output_tokens, sampling.reasoning_output_tokens, sampling.total_tokens,
       'sampling' AS evidence_source
FROM daily_usage_rollups sampling
JOIN effective_thread_day_source choice
  ON choice.local_day = sampling.local_day AND choice.thread_key = sampling.thread_key
WHERE sampling.quality = 'confirmed' AND choice.evidence_source = 'sampling'
UNION ALL
SELECT reconstructed.local_day, reconstructed.thread_key, reconstructed.account_key,
       reconstructed.project_key, reconstructed.model_key, 'confirmed' AS quality,
       reconstructed.event_count, reconstructed.input_tokens, reconstructed.cached_input_tokens,
       reconstructed.cache_write_input_tokens, reconstructed.cache_write_observed_input_tokens,
       reconstructed.output_tokens, reconstructed.reasoning_output_tokens,
       reconstructed.total_tokens, 'reconstruction' AS evidence_source
FROM reconstruction_daily_rollups reconstructed
JOIN effective_thread_day_source choice
  ON choice.local_day = reconstructed.local_day AND choice.thread_key = reconstructed.thread_key
WHERE choice.evidence_source = 'reconstruction';

CREATE VIEW effective_hourly_usage_rollups AS
SELECT sampling.local_hour, sampling.thread_key, sampling.account_key,
       sampling.project_key, sampling.model_key, sampling.quality,
       sampling.event_count, sampling.input_tokens, sampling.cached_input_tokens,
       sampling.cache_write_input_tokens, sampling.cache_write_observed_input_tokens,
       sampling.output_tokens, sampling.reasoning_output_tokens, sampling.total_tokens,
       'sampling' AS evidence_source
FROM hourly_usage_rollups sampling
JOIN effective_thread_day_source choice
  ON choice.local_day = substr(sampling.local_hour, 1, 10)
 AND choice.thread_key = sampling.thread_key
WHERE sampling.quality = 'confirmed' AND choice.evidence_source = 'sampling'
UNION ALL
SELECT reconstructed.local_hour, reconstructed.thread_key, reconstructed.account_key,
       reconstructed.project_key, reconstructed.model_key, 'confirmed' AS quality,
       reconstructed.event_count, reconstructed.input_tokens, reconstructed.cached_input_tokens,
       reconstructed.cache_write_input_tokens, reconstructed.cache_write_observed_input_tokens,
       reconstructed.output_tokens, reconstructed.reasoning_output_tokens,
       reconstructed.total_tokens, 'reconstruction' AS evidence_source
FROM reconstruction_hourly_rollups reconstructed
JOIN effective_thread_day_source choice
  ON choice.local_day = substr(reconstructed.local_hour, 1, 10)
 AND choice.thread_key = reconstructed.thread_key
WHERE choice.evidence_source = 'reconstruction';

CREATE VIEW effective_usage_events AS
SELECT sampling.event_id, sampling.observed_at, sampling.source_timestamp,
       sampling.thread_id, sampling.parent_thread_id, sampling.model, sampling.cwd,
       sampling.account_fingerprint, sampling.project_id, sampling.input_tokens,
       sampling.cached_input_tokens, sampling.cache_write_input_tokens,
       sampling.cache_write_observed_input_tokens, sampling.output_tokens,
       sampling.reasoning_output_tokens, sampling.total_tokens, sampling.quality
FROM usage_events sampling
JOIN effective_thread_day_source choice
  ON choice.local_day = date(COALESCE(sampling.source_timestamp, sampling.observed_at), '+8 hours')
 AND choice.thread_key = COALESCE(sampling.thread_id, '')
WHERE sampling.quality = 'confirmed' AND choice.evidence_source = 'sampling'
UNION ALL
SELECT reconstructed.event_id, reconstructed.observed_at, reconstructed.source_timestamp,
       reconstructed.thread_id, reconstructed.parent_thread_id, reconstructed.model,
       reconstructed.cwd, reconstructed.account_fingerprint, reconstructed.project_id,
       reconstructed.input_tokens, reconstructed.cached_input_tokens,
       reconstructed.cache_write_input_tokens,
       reconstructed.cache_write_observed_input_tokens, reconstructed.output_tokens,
       reconstructed.reasoning_output_tokens, reconstructed.total_tokens,
       'confirmed' AS quality
FROM reconstruction_usage_events reconstructed
JOIN effective_thread_day_source choice
  ON choice.local_day = date(reconstructed.source_timestamp, '+8 hours')
 AND choice.thread_key = reconstructed.thread_id
WHERE choice.evidence_source = 'reconstruction';
"#;

const MIGRATION_19: &str = r#"
CREATE TABLE standalone_thread_membership (
    thread_id TEXT PRIMARY KEY,
    root_thread_id TEXT NOT NULL
) WITHOUT ROWID;
CREATE INDEX standalone_thread_membership_root_idx
    ON standalone_thread_membership(root_thread_id);

INSERT INTO standalone_thread_membership(thread_id, root_thread_id)
WITH RECURSIVE tree(root_thread_id, thread_id) AS (
    SELECT thread_id, thread_id FROM thread_catalog
    WHERE parent_thread_id IS NULL AND COALESCE(depth, 0) = 0
      AND project_id IS NULL AND source_kind = 'state_5'
    UNION
    SELECT tree.root_thread_id, child.thread_id
    FROM tree JOIN thread_catalog child ON child.parent_thread_id = tree.thread_id
)
SELECT thread_id, MIN(root_thread_id) FROM tree GROUP BY thread_id;
"#;

const MIGRATION_20: &str = r#"
CREATE TABLE thread_root_membership (
    thread_id TEXT PRIMARY KEY,
    root_thread_id TEXT NOT NULL,
    relative_depth INTEGER NOT NULL CHECK(relative_depth >= 0)
) WITHOUT ROWID;
CREATE INDEX thread_root_membership_root_idx
    ON thread_root_membership(root_thread_id, relative_depth);

INSERT INTO thread_root_membership(thread_id, root_thread_id, relative_depth)
WITH RECURSIVE tree(root_thread_id, thread_id, relative_depth) AS (
    SELECT thread_id, thread_id, 0 FROM thread_catalog
    WHERE parent_thread_id IS NULL
    UNION
    SELECT tree.root_thread_id, child.thread_id, tree.relative_depth + 1
    FROM tree JOIN thread_catalog child ON child.parent_thread_id = tree.thread_id
    WHERE tree.relative_depth < 32
)
SELECT thread_id, MIN(root_thread_id), MIN(relative_depth)
FROM tree GROUP BY thread_id;
"#;

const MIGRATION_21: &str = r#"
CREATE INDEX reconstruction_events_effective_time_idx
    ON reconstruction_usage_events(COALESCE(source_timestamp, observed_at));
CREATE INDEX reconstruction_events_account_effective_time_idx
    ON reconstruction_usage_events(
        account_fingerprint, COALESCE(source_timestamp, observed_at)
    );
"#;

const MIGRATION_22: &str = r#"
CREATE INDEX reconstruction_events_thread_effective_time_idx
    ON reconstruction_usage_events(
        thread_id, COALESCE(source_timestamp, observed_at)
    );
"#;

const MIGRATION_23: &str = r#"
CREATE TRIGGER usage_events_confirmed_usage_insert_guard
BEFORE INSERT ON usage_events
WHEN NEW.quality = 'confirmed' AND (
    NEW.cached_input_tokens > NEW.input_tokens
    OR NEW.cache_write_input_tokens > NEW.input_tokens - NEW.cached_input_tokens
    OR NEW.cache_write_observed_input_tokens > NEW.input_tokens
    OR NEW.total_tokens - NEW.input_tokens != NEW.output_tokens
    OR NEW.reasoning_output_tokens > NEW.output_tokens
)
BEGIN
    SELECT RAISE(ABORT, 'confirmed token usage violates accounting invariants');
END;

CREATE TRIGGER usage_events_confirmed_usage_update_guard
BEFORE UPDATE OF input_tokens, cached_input_tokens, cache_write_input_tokens,
    cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
    total_tokens, quality ON usage_events
WHEN NEW.quality = 'confirmed' AND (
    NEW.cached_input_tokens > NEW.input_tokens
    OR NEW.cache_write_input_tokens > NEW.input_tokens - NEW.cached_input_tokens
    OR NEW.cache_write_observed_input_tokens > NEW.input_tokens
    OR NEW.total_tokens - NEW.input_tokens != NEW.output_tokens
    OR NEW.reasoning_output_tokens > NEW.output_tokens
)
BEGIN
    SELECT RAISE(ABORT, 'confirmed token usage violates accounting invariants');
END;

CREATE TRIGGER reconstruction_usage_insert_guard
BEFORE INSERT ON reconstruction_usage_events
WHEN NEW.cached_input_tokens > NEW.input_tokens
    OR NEW.cache_write_input_tokens > NEW.input_tokens - NEW.cached_input_tokens
    OR NEW.cache_write_observed_input_tokens > NEW.input_tokens
    OR NEW.total_tokens - NEW.input_tokens != NEW.output_tokens
    OR NEW.reasoning_output_tokens > NEW.output_tokens
BEGIN
    SELECT RAISE(ABORT, 'reconstruction token usage violates accounting invariants');
END;

CREATE TRIGGER reconstruction_usage_update_guard
BEFORE UPDATE OF input_tokens, cached_input_tokens, cache_write_input_tokens,
    cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
    total_tokens ON reconstruction_usage_events
WHEN NEW.cached_input_tokens > NEW.input_tokens
    OR NEW.cache_write_input_tokens > NEW.input_tokens - NEW.cached_input_tokens
    OR NEW.cache_write_observed_input_tokens > NEW.input_tokens
    OR NEW.total_tokens - NEW.input_tokens != NEW.output_tokens
    OR NEW.reasoning_output_tokens > NEW.output_tokens
BEGIN
    SELECT RAISE(ABORT, 'reconstruction token usage violates accounting invariants');
END;
"#;

const MIGRATION_24: &str = r#"
CREATE TRIGGER daily_usage_rollups_confirmed_usage_insert_guard
BEFORE INSERT ON daily_usage_rollups
WHEN NEW.quality = 'confirmed' AND (
    NEW.cached_input_tokens > NEW.input_tokens
    OR NEW.cache_write_input_tokens > NEW.input_tokens - NEW.cached_input_tokens
    OR NEW.cache_write_observed_input_tokens > NEW.input_tokens
    OR NEW.total_tokens - NEW.input_tokens != NEW.output_tokens
    OR NEW.reasoning_output_tokens > NEW.output_tokens
)
BEGIN
    SELECT RAISE(ABORT, 'confirmed rollup violates accounting invariants');
END;

CREATE TRIGGER daily_usage_rollups_confirmed_usage_update_guard
BEFORE UPDATE OF input_tokens, cached_input_tokens, cache_write_input_tokens,
    cache_write_observed_input_tokens, output_tokens, reasoning_output_tokens,
    total_tokens, quality ON daily_usage_rollups
WHEN NEW.quality = 'confirmed' AND (
    NEW.cached_input_tokens > NEW.input_tokens
    OR NEW.cache_write_input_tokens > NEW.input_tokens - NEW.cached_input_tokens
    OR NEW.cache_write_observed_input_tokens > NEW.input_tokens
    OR NEW.total_tokens - NEW.input_tokens != NEW.output_tokens
    OR NEW.reasoning_output_tokens > NEW.output_tokens
)
BEGIN
    SELECT RAISE(ABORT, 'confirmed rollup violates accounting invariants');
END;
"#;
