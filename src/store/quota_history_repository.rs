//! Versioned quota boundaries keep historical pagination stable during late appends.
use super::*;
use crate::quota::cycles::{Boundary, WindowSample, boundary_between};
use hmac::{Hmac, Mac};

const POINT_COLUMNS: &str = "observation_id,account_fingerprint,stream_key,observed_at,snapshot_id,window_ordinal,used_percent,window_seconds,resets_at_unix,pool_key,limit_id,limit_name,role";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuotaHistoryKey {
    pub at: String,
    pub snapshot_id: String,
    pub ordinal: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuotaHistoryView {
    pub instance: String,
    pub revision: i64,
    pub account: String,
    pub as_of: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuotaHistoryCursor {
    pub view: QuotaHistoryView,
    pub before: QuotaHistoryKey,
    pub signature: String,
}

fn cursor_mac(
    key: &[u8],
    view: &QuotaHistoryView,
    before: &QuotaHistoryKey,
) -> StoreResult<Hmac<Sha256>> {
    if key.len() != 32 {
        return Err(StoreError::InvalidRequestQuery(
            "quota history cursor key unavailable",
        ));
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .map_err(|_| StoreError::InvalidRequestQuery("quota history cursor key unavailable"))?;
    mac.update(b"quota-history-view-v1");
    mac.update(&serde_json::to_vec(&(view, before))?);
    Ok(mac)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaHistoryInterval {
    pub id: String,
    pub account_id: String,
    pub key: QuotaHistoryKey,
    pub stream_key: String,
    pub pool_key: String,
    pub limit_id: String,
    pub limit_name: Option<String>,
    pub role: String,
    pub window_seconds: Option<String>,
    pub boundary_kind: String,
    pub boundary_after: Option<String>,
    pub first_observed_at: String,
    pub last_observed_at: String,
    pub sample_count: u64,
    pub first_used_percent: Option<f64>,
    pub last_used_percent: Option<f64>,
    pub nominal_start: Option<DateTime<Utc>>,
    pub reported_reset: Option<DateTime<Utc>>,
    pub token_sample_start: Option<DateTime<Utc>>,
    pub token_sample_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaHistoryPage {
    pub index_ready: bool,
    pub source_history_complete: bool,
    pub view: QuotaHistoryView,
    pub intervals: Vec<QuotaHistoryInterval>,
    pub next: Option<QuotaHistoryCursor>,
    pub selections: Vec<QuotaHistoryCursor>,
}

struct ViewState {
    key: Vec<u8>,
    instance: String,
    revision: i64,
    ready: Option<i64>,
}
fn view_state(connection: &Connection) -> StoreResult<ViewState> {
    Ok(connection.query_row("SELECT cursor_key,instance_id,revision,ready_revision FROM quota_boundary_state WHERE id=1",[],|row|Ok(ViewState {key:row.get(0)?,instance:row.get(1)?,revision:row.get(2)?,ready:row.get(3)?}))?)
}
fn verify_cursor(cursor: &QuotaHistoryCursor, state: &ViewState, account: &str) -> StoreResult<()> {
    let signature = hex::decode(&cursor.signature)
        .map_err(|_| StoreError::InvalidRequestQuery("invalid quota history cursor"))?;
    cursor_mac(&state.key, &cursor.view, &cursor.before)?
        .verify_slice(&signature)
        .map_err(|_| StoreError::InvalidRequestQuery("invalid quota history cursor"))?;
    if cursor.view.instance != state.instance
        || cursor.view.account != account
        || cursor.view.revision > state.revision
        || cursor.view.revision < state.ready.unwrap_or(0)
        || cursor.view.as_of > Utc::now()
        || cursor.before.ordinal < 0
        || cursor.before.snapshot_id.len() > 256
        || parse_timestamp_column(cursor.before.at.clone(), 0).is_err()
    {
        return Err(StoreError::InvalidRequestQuery(
            "quota history cursor does not match this view",
        ));
    }
    Ok(())
}

struct Point {
    id: String,
    account: String,
    stream: String,
    key: QuotaHistoryKey,
    used: Option<f64>,
    seconds: Option<u64>,
    reset: Option<i64>,
    pool_key: String,
    limit_id: String,
    limit_name: Option<String>,
    role: String,
}

fn point_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Point> {
    let seconds: Option<String> = row.get(7)?;
    Ok(Point {
        id: row.get(0)?,
        account: row.get(1)?,
        stream: row.get(2)?,
        key: QuotaHistoryKey {
            at: row.get(3)?,
            snapshot_id: row.get(4)?,
            ordinal: row.get(5)?,
        },
        used: row.get(6)?,
        seconds: seconds
            .map(|value| {
                value.parse::<u64>().map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        7,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            })
            .transpose()?,
        reset: row.get(8)?,
        pool_key: row.get(9)?,
        limit_id: row.get(10)?,
        limit_name: row.get(11)?,
        role: row.get(12)?,
    })
}

impl Point {
    fn sample(&self) -> StoreResult<WindowSample> {
        Ok(WindowSample {
            at: parse_timestamp_column(self.key.at.clone(), 3)?.timestamp_millis(),
            reset: self
                .reset
                .and_then(|at| DateTime::<Utc>::from_timestamp(at, 0))
                .map(|at| at.timestamp_millis()),
            seconds: self.seconds,
            used: self.used,
        })
    }
}

fn point(connection: &Connection, id: &str) -> StoreResult<Point> {
    Ok(connection.query_row(
        &format!("SELECT {POINT_COLUMNS} FROM quota_window_observations WHERE observation_id=?1"),
        [id],
        point_from_row,
    )?)
}

fn neighbor(
    connection: &Connection,
    current: &Point,
    previous: bool,
) -> StoreResult<Option<Point>> {
    let (operator, order) = if previous {
        ("<", "DESC")
    } else {
        (">", "ASC")
    };
    Ok(connection.query_row(&format!("SELECT {POINT_COLUMNS} FROM quota_window_observations
        WHERE account_fingerprint=?1 AND stream_key=?2 AND (observed_at,snapshot_id,window_ordinal){operator}(?3,?4,?5)
        ORDER BY observed_at {order},snapshot_id {order},window_ordinal {order} LIMIT 1"),
        params![current.account,current.stream,current.key.at,current.key.snapshot_id,current.key.ordinal],point_from_row).optional()?)
}

fn revise_boundary(connection: &Connection, current: &Point, revision: i64) -> StoreResult<()> {
    let previous = neighbor(connection, current, true)?;
    let boundary = match &previous {
        Some(previous) => boundary_between(previous.sample()?, current.sample()?),
        None => Some(Boundary::FirstObservation),
    };
    let next = boundary.map(|kind| {
        (
            kind.code().to_owned(),
            previous.as_ref().map(|p| p.key.at.clone()),
        )
    });
    let old:Option<(String,Option<String>)>=connection.query_row(
        "SELECT boundary_kind,boundary_after FROM quota_boundary_versions WHERE observation_id=?1 AND valid_to IS NULL",[&current.id],
        |row|Ok((row.get(0)?,row.get(1)?))).optional()?;
    if old == next {
        return Ok(());
    }
    connection.execute("UPDATE quota_boundary_versions SET valid_to=?2 WHERE observation_id=?1 AND valid_to IS NULL",params![current.id,revision])?;
    if let Some((kind, after)) = next {
        connection.execute("INSERT INTO quota_boundary_versions(observation_id,valid_from,account_fingerprint,stream_key,observed_at,snapshot_id,window_ordinal,boundary_kind,boundary_after)
            VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![current.id,revision,current.account,current.stream,current.key.at,current.key.snapshot_id,current.key.ordinal,kind,after])?;
    }
    Ok(())
}

fn next_revision(connection: &Connection) -> StoreResult<i64> {
    Ok(connection.query_row(
        "UPDATE quota_boundary_state SET revision=revision+1 WHERE id=1 RETURNING revision",
        [],
        |row| row.get(0),
    )?)
}

pub(super) fn observe_window_in(connection: &Connection, id: &str) -> StoreResult<()> {
    let revision = next_revision(connection)?;
    connection.execute(
        "UPDATE quota_window_observations SET created_revision=?2 WHERE observation_id=?1",
        params![id, revision],
    )?;
    let current = point(connection, id)?;
    revise_boundary(connection, &current, revision)?;
    if let Some(next) = neighbor(connection, &current, false)? {
        revise_boundary(connection, &next, revision)?;
    }
    Ok(())
}

impl LedgerStore {
    /// Completes versioned boundary projection in bounded, restartable batches.
    pub fn backfill_quota_history_chunk(&mut self, limit: usize) -> StoreResult<bool> {
        let ready: bool = self.connection.query_row(
            "SELECT ready_revision IS NOT NULL FROM quota_boundary_state WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        if ready && self.quota_window_index_complete()? {
            return Ok(true);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (last, target, complete): (i64, i64, bool) = transaction.query_row(
            "SELECT last_rowid,target_rowid,complete FROM quota_boundary_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if !complete {
            let batch = {
                let mut statement=transaction.prepare("SELECT rowid,observation_id FROM quota_window_observations WHERE rowid>?1 AND rowid<=?2 ORDER BY rowid LIMIT ?3")?;
                statement
                    .query_map(params![last, target, limit.clamp(1, 1000) as i64], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let revision = next_revision(&transaction)?;
            for (_, id) in &batch {
                revise_boundary(&transaction, &point(&transaction, id)?, revision)?;
            }
            let next = batch.last().map_or(last, |(id, _)| *id);
            transaction.execute("UPDATE quota_boundary_state SET last_rowid=?1,complete=NOT EXISTS(SELECT 1 FROM quota_window_observations WHERE rowid>?1 AND rowid<=target_rowid) WHERE id=1",[next])?;
        }
        transaction.execute("UPDATE quota_boundary_state SET ready_revision=revision WHERE id=1 AND ready_revision IS NULL AND complete=1 AND (SELECT complete FROM quota_window_index_state WHERE id=1)=1",[])?;
        let ready = transaction.query_row(
            "SELECT ready_revision IS NOT NULL FROM quota_boundary_state WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(ready)
    }

    pub fn quota_history_page(
        &self,
        account: &str,
        cursor: Option<&QuotaHistoryCursor>,
        limit: usize,
    ) -> StoreResult<QuotaHistoryPage> {
        if account.is_empty() || account.len() > 256 || !(1..=100).contains(&limit) {
            return Err(StoreError::InvalidRequestQuery(
                "invalid quota history scope or page size",
            ));
        }
        let read = |store: &LedgerStore| {
            let state = view_state(&store.connection)?;
            if let Some(cursor) = cursor {
                verify_cursor(cursor, &state, account)?;
            }
            let ViewState {
                key,
                instance,
                revision,
                ready,
            } = state;
            let view = cursor
                .map(|cursor| cursor.view.clone())
                .unwrap_or(QuotaHistoryView {
                    instance,
                    revision,
                    account: account.to_owned(),
                    as_of: Utc::now(),
                });
            if ready.is_none() || !store.quota_window_index_complete()? {
                return Ok(QuotaHistoryPage {
                    index_ready: false,
                    source_history_complete: false,
                    view,
                    intervals: vec![],
                    next: None,
                    selections: vec![],
                });
            }
            let before = cursor.map(|cursor| &cursor.before);
            let account_clause = if account == "all" {
                "1=1"
            } else {
                "account_fingerprint=?1"
            };
            let version_clause = if view.revision == revision {
                "valid_to IS NULL"
            } else {
                "(valid_to IS NULL OR valid_to>?2)"
            };
            let condition = if before.is_some() {
                "AND (observed_at,snapshot_id,window_ordinal)<(?4,?5,?6)"
            } else {
                ""
            };
            let mut values: Vec<SqlValue> = vec![
                account.to_owned().into(),
                view.revision.into(),
                timestamp(view.as_of).into(),
            ];
            if let Some(before) = before {
                values.extend([
                    before.at.clone().into(),
                    before.snapshot_id.clone().into(),
                    before.ordinal.into(),
                ]);
            }
            let limit_parameter = values.len() + 1;
            values.push(((limit + 1) as i64).into());
            let mut statement=store.connection.prepare(&format!("SELECT observation_id,boundary_kind,boundary_after FROM quota_boundary_versions
                WHERE {account_clause} AND valid_from<=?2 AND {version_clause} AND observed_at<=?3 {condition}
                ORDER BY observed_at DESC,snapshot_id DESC,window_ordinal DESC LIMIT ?{limit_parameter}"))?;
            let mut boundaries = statement
                .query_map(params_from_iter(values), |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let more = boundaries.len() > limit;
            boundaries.truncate(limit);
            let mut intervals = Vec::new();
            for (id, kind, after) in boundaries {
                intervals.push(history_interval(
                    &store.connection,
                    &view,
                    &id,
                    kind,
                    after,
                )?);
            }
            let selections = intervals
                .iter()
                .map(|row| {
                    Ok::<_, StoreError>(QuotaHistoryCursor {
                        view: view.clone(),
                        before: row.key.clone(),
                        signature: hex::encode(
                            cursor_mac(&key, &view, &row.key)?.finalize().into_bytes(),
                        ),
                    })
                })
                .collect::<StoreResult<Vec<_>>>()?;
            let next = if more {
                selections.last().cloned()
            } else {
                None
            };
            Ok(QuotaHistoryPage {
                index_ready: true,
                source_history_complete: false,
                view,
                intervals,
                next,
                selections,
            })
        };
        if self.connection.is_autocommit() {
            self.with_source_audit_snapshot(read)
        } else {
            read(self)
        }
    }

    pub(crate) fn selected_quota_history_interval(
        &self,
        selection: &QuotaHistoryCursor,
    ) -> StoreResult<QuotaHistoryInterval> {
        let read = |store: &LedgerStore| {
            let state = view_state(&store.connection)?;
            verify_cursor(selection, &state, &selection.view.account)?;
            if state.ready.is_none() || !store.quota_window_index_complete()? {
                return Err(StoreError::SnapshotUnavailable);
            }
            let (id,kind,after):(String,String,Option<String>)=store.connection.query_row(
                "SELECT observation_id,boundary_kind,boundary_after FROM quota_boundary_versions
                 WHERE (account_fingerprint=?1 OR ?1='all') AND observed_at=?2 AND snapshot_id=?3 AND window_ordinal=?4
                 AND valid_from<=?5 AND (valid_to IS NULL OR valid_to>?5)",
                params![selection.view.account,selection.before.at,selection.before.snapshot_id,selection.before.ordinal,selection.view.revision],
                |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
            history_interval(&store.connection, &selection.view, &id, kind, after)
        };
        if self.connection.is_autocommit() {
            self.with_source_audit_snapshot(read)
        } else {
            read(self)
        }
    }
}

fn history_interval(
    connection: &Connection,
    view: &QuotaHistoryView,
    id: &str,
    kind: String,
    after: Option<String>,
) -> StoreResult<QuotaHistoryInterval> {
    let first = point(connection, id)?;
    let next_id:Option<String>=connection.query_row("SELECT observation_id FROM quota_boundary_versions
        WHERE account_fingerprint=?1 AND stream_key=?2 AND valid_from<=?3 AND (valid_to IS NULL OR valid_to>?3)
        AND (observed_at,snapshot_id,window_ordinal)>(?4,?5,?6) AND observed_at<=?7
        ORDER BY observed_at,snapshot_id,window_ordinal LIMIT 1",
        params![first.account,first.stream,view.revision,first.key.at,first.key.snapshot_id,first.key.ordinal,timestamp(view.as_of)],|row|row.get(0)).optional()?;
    let next = next_id.map(|id| point(connection, &id)).transpose()?;
    let mut values: Vec<SqlValue> = vec![
        first.account.clone().into(),
        first.stream.clone().into(),
        view.revision.into(),
        first.key.at.clone().into(),
        first.key.snapshot_id.clone().into(),
        first.key.ordinal.into(),
        timestamp(view.as_of).into(),
    ];
    let ending = if let Some(next) = &next {
        values.extend([
            next.key.at.clone().into(),
            next.key.snapshot_id.clone().into(),
            next.key.ordinal.into(),
        ]);
        "AND (observed_at,snapshot_id,window_ordinal)<(?8,?9,?10)"
    } else {
        ""
    };
    let scope=format!("FROM quota_window_observations WHERE account_fingerprint=?1 AND stream_key=?2 AND created_revision<=?3
        AND (observed_at,snapshot_id,window_ordinal)>=(?4,?5,?6) AND observed_at<=?7 {ending}");
    let last=connection.query_row(&format!("SELECT {POINT_COLUMNS} {scope} ORDER BY observed_at DESC,snapshot_id DESC,window_ordinal DESC LIMIT 1"),params_from_iter(values.iter()),point_from_row)?;
    let count: i64 = connection.query_row(
        &format!("SELECT COUNT(*) {scope}"),
        params_from_iter(values.iter()),
        |row| row.get(0),
    )?;
    let reset = last
        .reset
        .and_then(|at| DateTime::<Utc>::from_timestamp(at, 0));
    let nominal_start = reset.zip(last.seconds).and_then(|(at, seconds)| {
        i64::try_from(seconds)
            .ok()
            .and_then(ChronoDuration::try_seconds)
            .and_then(|duration| at.checked_sub_signed(duration))
    });
    let first_at = parse_timestamp_column(first.key.at.clone(), 0)?;
    let last_at = parse_timestamp_column(last.key.at.clone(), 0)?;
    let start = nominal_start.map_or(first_at, |at| at.max(first_at));
    let mut end = reset.map_or(view.as_of, |at| at.min(view.as_of));
    if let Some(next) = next {
        let next_at = parse_timestamp_column(next.key.at, 0)?;
        end = end.min(
            reset
                .filter(|at| *at >= last_at && *at <= next_at)
                .unwrap_or(last_at),
        );
    }
    Ok(QuotaHistoryInterval {
        id: first.id,
        account_id: first.account,
        key: first.key.clone(),
        stream_key: first.stream,
        pool_key: first.pool_key,
        limit_id: first.limit_id,
        limit_name: first.limit_name,
        role: first.role,
        window_seconds: last.seconds.map(|value| value.to_string()),
        boundary_kind: kind,
        boundary_after: after,
        first_observed_at: first.key.at,
        last_observed_at: last.key.at,
        sample_count: count as u64,
        first_used_percent: first.used,
        last_used_percent: last.used,
        nominal_start,
        reported_reset: reset,
        token_sample_start: (start < end).then_some(start),
        token_sample_end: (start < end).then_some(end),
    })
}

#[cfg(test)]
mod tests;
