//! Scope-local projection readiness and bounded same-snapshot resolution.
use super::*;
use crate::source_union::{self, EvidenceSide, Measurement};
use std::collections::BTreeSet;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeReadiness {
    pub groups: u64,
    pub pending_groups: u64,
    pub unresolved_groups: u64,
}

pub(super) struct ResolvedScope {
    pub selected: Vec<Measurement>,
    pub unresolved_groups: usize,
}

pub(super) fn thread_condition(column: &str, parameter: &str, descendants: bool) -> String {
    if !descendants {
        return format!("{column}={parameter}");
    }
    format!("{column} IN (WITH RECURSIVE scoped_threads(id) AS (SELECT {parameter}
        UNION SELECT c.thread_id FROM thread_catalog c JOIN scoped_threads t ON c.parent_thread_id=t.id LIMIT 10001)
        SELECT id FROM scoped_threads)")
}

pub(super) fn scope_threads(
    connection: &Connection,
    query: &SourceUnionQuery,
) -> StoreResult<BTreeSet<String>> {
    let root = query
        .thread
        .as_ref()
        .filter(|id| !id.trim().is_empty())
        .ok_or(StoreError::InvalidRequestQuery(
            "descendant scope requires a thread",
        ))?;
    let sql = format!(
        "SELECT thread_id,parent_thread_id FROM thread_catalog WHERE {}",
        thread_condition("thread_id", "?1", true)
    );
    let rows = connection
        .prepare(&sql)?
        .query_map([root], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut members = BTreeSet::from([root.clone()]);
    members.extend(rows.iter().map(|(id, _)| id.clone()));
    if members.len() > 10000 {
        return Err(StoreError::UnionLimit);
    }
    if rows
        .iter()
        .any(|(id, parent)| id == root && parent.as_ref().is_some_and(|p| members.contains(p)))
    {
        return Err(StoreError::InvalidRequestQuery("cyclic descendant catalog"));
    }
    Ok(members)
}

fn predicate(query: &SourceUnionQuery, time: &str, columns: [&str; 4]) -> (String, Vec<SqlValue>) {
    let mut sql = format!("{time}>=?1 AND {time}<?2");
    let mut values = vec![
        SqlValue::Text(timestamp(query.start)),
        SqlValue::Text(timestamp(query.end)),
    ];
    for (index, (column, value)) in columns
        .into_iter()
        .zip([&query.account, &query.project, &query.model, &query.thread])
        .enumerate()
    {
        if let Some(value) = value {
            values.push(SqlValue::Text(value.clone()));
            sql.push_str(&format!(
                " AND {}",
                if index == 3 {
                    thread_condition(
                        column,
                        &format!("?{}", values.len()),
                        query.include_descendants,
                    )
                } else {
                    format!("{column}=?{}", values.len())
                }
            ));
        }
    }
    (sql, values)
}

fn seeds(query: &SourceUnionQuery, side: EvidenceSide) -> (String, Vec<SqlValue>) {
    let (from, time, columns, kind) = match side {
        EvidenceSide::Sampling => (
            "retained_request_evidence k LEFT JOIN retained_request_assignments a USING(event_id)",
            "k.effective_at",
            [
                "a.account_fingerprint",
                "a.project_id",
                "k.model",
                "k.thread_id",
            ],
            "sampling",
        ),
        EvidenceSide::Reconstruction => (
            "reconstruction_usage_events k",
            "COALESCE(k.source_timestamp,k.observed_at)",
            [
                "k.account_fingerprint",
                "k.project_id",
                "k.model",
                "k.thread_id",
            ],
            "reconstruction",
        ),
    };
    let (where_sql, values) = predicate(query, time, columns);
    let eligibility = if side == EvidenceSide::Sampling {
        " AND (k.quality='confirmed' OR COALESCE(p.record_key,'')<>'')"
    } else {
        ""
    };
    (format!("SELECT '{kind}' AS side,k.event_id,CASE WHEN p.record_key IS NULL OR p.record_key='' THEN '{kind}' ELSE 'key' END AS kind,
        COALESCE(NULLIF(p.record_key,''),k.event_id) AS identity FROM {from}
        LEFT JOIN source_record_evidence p ON p.evidence_source='{kind}' AND p.event_id=k.event_id WHERE {where_sql}{eligibility}"),values)
}

pub(super) fn unconfirmed_observations(
    connection: &Connection,
    query: &SourceUnionQuery,
) -> StoreResult<u64> {
    let (scope, values) = predicate(
        query,
        "k.effective_at",
        [
            "a.account_fingerprint",
            "a.project_id",
            "k.model",
            "k.thread_id",
        ],
    );
    Ok(connection.query_row(&format!("SELECT COUNT(*) FROM retained_request_evidence k LEFT JOIN retained_request_assignments a USING(event_id) WHERE {scope} AND k.quality<>'confirmed'"),params_from_iter(values),|row|u64_from_sql(row.get(0)?,0))?)
}

pub(super) fn readiness(
    connection: &Connection,
    query: &SourceUnionQuery,
) -> StoreResult<ScopeReadiness> {
    let (sampling, values) = seeds(query, EvidenceSide::Sampling);
    let (reconstruction, _) = seeds(query, EvidenceSide::Reconstruction);
    let (selected, _) = predicate(
        query,
        "effective_at",
        ["account_fingerprint", "project_id", "model", "thread_id"],
    );
    // Include stale selected rows: deleted/moved raw records must not disappear
    // from readiness checks while their old projection still matches the scope.
    let sql = format!(
        "WITH relevant AS (
        SELECT kind,identity FROM ({sampling}) UNION SELECT kind,identity FROM ({reconstruction})
        UNION SELECT kind,identity FROM measurement_union_selected WHERE {selected})
        SELECT COUNT(*),COALESCE(SUM(g.kind IS NULL OR d.kind IS NOT NULL),0),
        COALESCE(SUM(g.unresolved_reason IS NOT NULL),0) FROM relevant r
        LEFT JOIN measurement_union_groups g USING(kind,identity)
        LEFT JOIN measurement_union_dirty d USING(kind,identity)"
    );
    Ok(connection.query_row(&sql, params_from_iter(values), |row| {
        Ok(ScopeReadiness {
            groups: u64_from_sql(row.get(0)?, 0)?,
            pending_groups: u64_from_sql(row.get(1)?, 1)?,
            unresolved_groups: u64_from_sql(row.get(2)?, 2)?,
        })
    })?)
}

pub(super) fn resolve(
    connection: &Connection,
    query: &SourceUnionQuery,
    limit: usize,
) -> StoreResult<ResolvedScope> {
    let mut identities = Vec::new();
    for side in [EvidenceSide::Sampling, EvidenceSide::Reconstruction] {
        let (sql, values) = seeds(query, side);
        let ids = connection
            .prepare(&format!(
                "SELECT event_id FROM ({sql}) LIMIT {}",
                limit - identities.len() + 1
            ))?
            .query_map(params_from_iter(values), |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        identities.extend(ids.into_iter().map(|id| (side, id)));
        if identities.len() > limit {
            return Err(StoreError::UnionLimit);
        }
    }
    let mut rows = Vec::<Measurement>::new();
    let mut visited = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for identity in identities {
        if !visited.insert(identity.clone()) {
            continue;
        }
        let row = union_repository::load_measurement(connection, identity.0, &identity.1)?;
        let key = row.record_key.clone();
        rows.push(row);
        if rows.len() > limit {
            return Err(StoreError::UnionLimit);
        }
        if let Some(key) = key.filter(|key| keys.insert(key.clone())) {
            let peers = connection
                .prepare_cached(union_repository::COUNTERPARTS)?
                .query_map(params![key, (limit + 1) as i64], |row| {
                    Ok((
                        if row.get::<_, String>(0)? == "sampling" {
                            EvidenceSide::Sampling
                        } else {
                            EvidenceSide::Reconstruction
                        },
                        row.get::<_, String>(1)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            if peers.len() > limit {
                return Err(StoreError::UnionLimit);
            }
            for peer in peers {
                if visited.insert(peer.clone()) {
                    if rows.len() == limit {
                        return Err(StoreError::UnionLimit);
                    }
                    rows.push(union_repository::load_measurement(
                        connection, peer.0, &peer.1,
                    )?);
                }
            }
        }
    }
    let mut result = source_union::plan(rows, query.start, query.end)?;
    let descendants = if query.include_descendants {
        Some(scope_threads(connection, query)?)
    } else {
        None
    };
    // Resolve complete key groups before applying metadata filters. This keeps
    // conflicting accounts/models outside the requested filter visible.
    result.selected.retain(|row| {
        query
            .account
            .as_ref()
            .is_none_or(|v| row.account.as_ref() == Some(v))
            && query
                .project
                .as_ref()
                .is_none_or(|v| row.project.as_ref() == Some(v))
            && query
                .model
                .as_ref()
                .is_none_or(|v| row.model.as_ref() == Some(v))
            && descendants.as_ref().map_or_else(
                || query.thread.as_ref().is_none_or(|v| &row.thread == v),
                |members| members.contains(&row.thread),
            )
    });
    Ok(ResolvedScope {
        selected: result.selected,
        unresolved_groups: result.unresolved.len(),
    })
}
