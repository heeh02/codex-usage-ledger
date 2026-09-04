use super::*;

fn explorer_event(id: &str, thread_id: &str, parent: Option<&str>) -> crate::UsageEvent {
    crate::UsageEvent {
        event_id: id.to_owned(),
        observed_at: Utc::now(),
        source_timestamp: Some(Utc::now()),
        thread_id: Some(thread_id.to_owned()),
        parent_thread_id: parent.map(str::to_owned),
        model: Some("gpt-5.6-sol".to_owned()),
        cwd: None,
        account_fingerprint: None,
        account_confidence: crate::AttributionConfidence::Unknown,
        project: crate::ProjectAttribution {
            project_id: Some("project".to_owned()),
            project_name: Some("Project".to_owned()),
            confidence: crate::AttributionConfidence::Verified,
            method: "test".to_owned(),
        },
        usage: TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 80,
            cache_write_input_tokens: 0,
            cache_write_observed_input_tokens: 100,
            output_tokens: 20,
            reasoning_output_tokens: 5,
            total_tokens: 120,
        },
        quality: DataQuality::Confirmed,
        quality_reason: None,
        provenance: crate::EventProvenance {
            machine_id: "machine".to_owned(),
            source_id: format!("source-{id}"),
            rollout_id: thread_id.to_owned(),
            file_identity: format!("file-{id}"),
            byte_offset: 1,
            line_number: 1,
        },
    }
}

fn native_catalog_thread(
    thread_id: &str,
    parent: Option<&str>,
    project: Option<&str>,
    depth: u32,
    title: &str,
) -> crate::store::ThreadCatalogRecord {
    let at = Utc.with_ymd_and_hms(2026, 8, 31, 12, 0, 0).unwrap();
    crate::store::ThreadCatalogRecord {
        thread_id: thread_id.to_owned(),
        parent_thread_id: parent.map(str::to_owned),
        project_id: project.map(str::to_owned),
        project_name: project.map(str::to_owned),
        title: Some(title.to_owned()),
        model: Some("gpt-5.6-sol".to_owned()),
        agent_nickname: None,
        agent_role: None,
        agent_path: None,
        depth: Some(depth),
        created_at: at,
        updated_at: at,
        archived: false,
        has_user_event: depth == 0,
        source_kind: "state_5".to_owned(),
    }
}

#[test]
fn cached_input_is_not_added_twice() {
    let totals = UsageTotals::from(TokenUsage {
        input_tokens: 100,
        cached_input_tokens: 80,
        cache_write_input_tokens: 5,
        cache_write_observed_input_tokens: 100,
        output_tokens: 15,
        reasoning_output_tokens: 5,
        total_tokens: 115,
    });
    assert_eq!(totals.uncached_input_tokens, 15);
    assert_eq!(totals.cache_write_input_tokens, 5);
    assert_eq!(totals.total_tokens, 115);
}

#[test]
fn calendar_and_rolling_periods_have_distinct_shanghai_boundaries() {
    let now = Utc.with_ymd_and_hms(2026, 8, 31, 8, 0, 0).unwrap();
    let resolve = |period: &str| {
        resolve_period_at(
            &UsageQuery {
                period: Some(period.to_owned()),
                timezone: Some("Asia/Shanghai".to_owned()),
                ..UsageQuery::default()
            },
            now,
        )
        .2
    };
    let today = resolve("today");
    let week = resolve("week");
    let rolling = resolve("rolling7");
    let month = resolve("month");
    let weeks = resolve("weeks12");
    let months = resolve("months12");

    assert_eq!(
        today.start,
        Some(Utc.with_ymd_and_hms(2026, 8, 30, 16, 0, 0).unwrap())
    );
    assert_eq!(week.start, today.start);
    assert_eq!(
        rolling.start,
        Some(Utc.with_ymd_and_hms(2026, 8, 24, 8, 0, 0).unwrap())
    );
    assert_eq!(
        month.start,
        Some(Utc.with_ymd_and_hms(2026, 7, 31, 16, 0, 0).unwrap())
    );
    assert_eq!(
        month.comparison_start,
        Some(Utc.with_ymd_and_hms(2026, 6, 30, 16, 0, 0).unwrap())
    );
    assert_eq!(
        month.comparison_end,
        Some(Utc.with_ymd_and_hms(2026, 7, 31, 8, 0, 0).unwrap())
    );
    assert_eq!(
        weeks.start,
        Some(Utc.with_ymd_and_hms(2026, 6, 14, 16, 0, 0).unwrap())
    );
    assert_eq!(weeks.default_grain, "week");
    assert_eq!(
        months.start,
        Some(Utc.with_ymd_and_hms(2025, 8, 31, 16, 0, 0).unwrap())
    );
    assert_eq!(months.default_grain, "month");
}

#[test]
fn september_first_exposes_a_cross_month_week_without_changing_month_semantics() {
    let now = Utc.with_ymd_and_hms(2026, 8, 31, 17, 30, 0).unwrap();
    let resolve = |period: &str| {
        resolve_period_at(
            &UsageQuery {
                period: Some(period.to_owned()),
                timezone: Some("Asia/Shanghai".to_owned()),
                ..UsageQuery::default()
            },
            now,
        )
        .2
    };
    let week = resolve("week");
    let month = resolve("month");
    let timezone = chrono_tz::Asia::Shanghai;

    assert_eq!(
        week.start,
        Some(Utc.with_ymd_and_hms(2026, 8, 30, 16, 0, 0).unwrap())
    );
    assert_eq!(
        month.start,
        Some(Utc.with_ymd_and_hms(2026, 8, 31, 16, 0, 0).unwrap())
    );
    assert!(week.start < month.start);
    assert!(window_crosses_month(week.start, now, timezone));
    assert!(!window_crosses_month(month.start, now, timezone));
    assert!(!window_crosses_year(week.start, now, timezone));
}

#[test]
fn rejects_non_local_origin() {
    let mut headers = HeaderMap::new();
    headers.insert("origin", "https://example.com".parse().unwrap());
    assert!(!accepts_local_origin(&headers));
}

#[test]
fn explorer_keeps_session_own_usage_separate_from_subtree_usage() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    store
        .upsert_event(&explorer_event("root-event", "root", None))
        .unwrap();
    store
        .upsert_event(&explorer_event("child-event", "child", Some("root")))
        .unwrap();
    let query = UsageQuery {
        period: Some("lifetime".to_owned()),
        project: Some("project".to_owned()),
        session: Some("root".to_owned()),
        ..UsageQuery::default()
    };
    let value = http_explorer(&store, &query).unwrap();
    assert_eq!(
        value
            .pointer("/stats/sessionCount")
            .and_then(|v| v.as_u64()),
        Some(0)
    );
    assert_eq!(
        value
            .pointer("/stats/subagentCount")
            .and_then(|v| v.as_u64()),
        Some(0)
    );
    assert_eq!(
        value
            .pointer("/stats/historicalSessionCount")
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/stats/historicalSubagentCount")
            .and_then(|v| v.as_u64()),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/selectedSession/ownUsage/total")
            .and_then(|v| v.as_u64()),
        Some(120)
    );
    assert_eq!(
        value
            .pointer("/selectedSession/treeUsage/total")
            .and_then(|v| v.as_u64()),
        Some(240)
    );
    assert_eq!(
        value
            .pointer("/selectedSession/nodes")
            .and_then(|v| v.as_array())
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn explorer_project_lifetime_buckets_reconcile_to_lifetime_total() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    store
        .upsert_event(&explorer_event("project-event", "project-thread", None))
        .unwrap();
    let mut unassigned = explorer_event("unassigned-event", "unassigned-thread", None);
    unassigned.project.project_id = None;
    unassigned.project.project_name = None;
    store.upsert_event(&unassigned).unwrap();

    let value = http_explorer(
        &store,
        &UsageQuery {
            period: Some("week".to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();
    let lifetime = value
        .pointer("/stats/lifetime/total")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    let project_sum = value
        .get("projects")
        .and_then(serde_json::Value::as_array)
        .unwrap()
        .iter()
        .map(|project| {
            project
                .pointer("/lifetimeUsage/total")
                .and_then(serde_json::Value::as_u64)
                .unwrap()
        })
        .sum::<u64>();
    assert_eq!(lifetime, 240);
    assert_eq!(project_sum, lifetime);
    assert!(
        value
            .get("projects")
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .iter()
            .any(|project| project.get("id").and_then(|id| id.as_str()) == Some("unassigned"))
    );
}

#[test]
fn standalone_conversations_are_native_root_trees_not_all_null_project_events() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut root = explorer_event("standalone-root-event", "standalone-root", None);
    root.project.project_id = None;
    root.project.project_name = None;
    let mut child = explorer_event(
        "standalone-child-event",
        "standalone-child",
        Some("standalone-root"),
    );
    child.project.project_id = Some("cwd-project".to_owned());
    child.project.project_name = Some("Cwd Project".to_owned());
    let mut unmatched = explorer_event("unmatched-event", "unmatched-thread", None);
    unmatched.project.project_id = None;
    unmatched.project.project_name = None;
    store.upsert_event(&root).unwrap();
    store.upsert_event(&child).unwrap();
    store.upsert_event(&unmatched).unwrap();

    store
        .upsert_thread_catalog(&native_catalog_thread(
            "historical-standalone",
            None,
            None,
            0,
            "Historical standalone",
        ))
        .unwrap();
    store
        .sync_native_thread_catalog_batch(&[
            native_catalog_thread("standalone-root", None, None, 0, "Standalone root"),
            native_catalog_thread(
                "standalone-child",
                Some("standalone-root"),
                Some("cwd-project"),
                1,
                "Standalone child",
            ),
        ])
        .unwrap();

    let buckets = store
        .aggregate_rollup_by(AggregateDimension::Project, &AggregateFilter::default())
        .unwrap()
        .into_iter()
        .map(|bucket| (bucket.key.unwrap(), bucket.usage.total_tokens))
        .collect::<HashMap<_, _>>();
    assert_eq!(buckets.get(STANDALONE_PROJECT_ID), Some(&240));
    assert_eq!(buckets.get(UNASSIGNED_PROJECT_ID), Some(&120));
    assert_eq!(buckets.get("cwd-project"), None);

    let overview = http_explorer(
        &store,
        &UsageQuery {
            period: Some("lifetime".to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();
    assert_eq!(overview["stats"]["standaloneConversations"]["current"], 1);
    assert_eq!(
        overview["stats"]["standaloneConversations"]["historical"],
        1
    );
    assert_eq!(overview["stats"]["standaloneConversations"]["indexed"], 2);
    assert_eq!(
        overview["stats"]["standaloneConversations"]["withLocalEvidence"],
        1
    );
    let projects = overview["projects"].as_array().unwrap();
    assert!(projects.iter().any(|project| {
        project["id"] == STANDALONE_PROJECT_ID
            && project["kind"] == "standalone_conversations"
            && project["lifetimeUsage"]["total"] == 240
    }));
    assert!(projects.iter().any(|project| {
        project["id"] == UNASSIGNED_PROJECT_ID
            && project["kind"] == "unmatched_records"
            && project["lifetimeUsage"]["total"] == 120
    }));

    let standalone = http_explorer(
        &store,
        &UsageQuery {
            period: Some("lifetime".to_owned()),
            project: Some(STANDALONE_PROJECT_ID.to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();
    assert_eq!(standalone["stats"]["lifetime"]["total"], 240);
    assert_eq!(standalone["stats"]["sessionCount"], 1);
    assert_eq!(standalone["stats"]["subagentCount"], 1);
    assert_eq!(standalone["stats"]["historicalSessionCount"], 1);
    assert_eq!(standalone["sessions"].as_array().unwrap().len(), 2);

    let unmatched = http_explorer(
        &store,
        &UsageQuery {
            period: Some("lifetime".to_owned()),
            project: Some(UNASSIGNED_PROJECT_ID.to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();
    assert_eq!(unmatched["stats"]["lifetime"]["total"], 120);
    assert_eq!(unmatched["stats"]["sessionCount"], 0);
    assert_eq!(unmatched["sessions"].as_array().unwrap().len(), 0);
}

#[test]
fn bundle_local_dimensions_and_series_reconcile_under_one_filter() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut first = explorer_event("first", "thread-a", None);
    first.model = Some("model-a".to_owned());
    first.project.project_id = Some("project-a".to_owned());
    first.project.project_name = Some("Project A".to_owned());
    let mut second = explorer_event("second", "thread-b", None);
    second.model = Some("model-b".to_owned());
    second.project.project_id = Some("project-b".to_owned());
    second.project.project_name = Some("Project B".to_owned());
    store.upsert_event(&first).unwrap();
    store.upsert_event(&second).unwrap();

    let bundle = http_bundle(
        &store,
        &UsageQuery {
            period: Some("lifetime".to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();
    serde_json::from_value::<crate::api::wire::DashboardBundle>(bundle.clone())
        .expect("bundle response must satisfy the Rust wire DTO contract");
    let total = bundle
        .pointer("/summary/usage/confirmed/total")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    let series_sum = bundle["timeseries"]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| point["confirmed"]["total"].as_u64().unwrap())
        .sum::<u64>();
    for dimension in ["project", "model", "account"] {
        let sum = bundle["breakdowns"][dimension]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["usage"]["confirmed"]["total"].as_u64().unwrap())
            .sum::<u64>();
        assert_eq!(sum, total, "{dimension} must reconcile");
    }
    assert_eq!(series_sum, total);
}

#[test]
fn rolling_seven_series_uses_the_same_exact_boundary_as_the_kpi() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let now = Utc::now();
    let mut outside = explorer_event("outside", "thread-outside", None);
    outside.observed_at = now - ChronoDuration::days(7) - ChronoDuration::minutes(10);
    outside.source_timestamp = Some(outside.observed_at);
    let mut inside = explorer_event("inside", "thread-inside", None);
    inside.observed_at = now - ChronoDuration::days(7) + ChronoDuration::minutes(10);
    inside.source_timestamp = Some(inside.observed_at);
    store.upsert_event(&outside).unwrap();
    store.upsert_event(&inside).unwrap();

    let bundle = http_bundle(
        &store,
        &UsageQuery {
            period: Some("rolling7".to_owned()),
            timezone: Some("Asia/Shanghai".to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();
    let summary_total = bundle
        .pointer("/summary/usage/confirmed/total")
        .and_then(serde_json::Value::as_u64)
        .unwrap();
    let series_total = bundle["timeseries"]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| point["confirmed"]["total"].as_u64().unwrap())
        .sum::<u64>();
    let project_series_total = bundle["timeseries"]["projectSeries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|series| series["totalTokens"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(summary_total, 120);
    assert_eq!(series_total, summary_total);
    assert_eq!(project_series_total, summary_total);
}

#[test]
fn official_daily_coverage_does_not_turn_a_missing_current_day_into_zero() {
    use crate::official_usage::{
        OfficialAccountUsage, OfficialDailyUsageBucket, OfficialUsageSummary,
    };
    let mut store = LedgerStore::open_in_memory().unwrap();
    store
        .upsert_official_account_usage(
            "account",
            Utc.with_ymd_and_hms(2026, 8, 31, 8, 0, 0).unwrap(),
            &OfficialAccountUsage {
                summary: OfficialUsageSummary {
                    lifetime_tokens: Some(100),
                    peak_daily_tokens: Some(40),
                    ..OfficialUsageSummary::default()
                },
                daily_usage_buckets: vec![OfficialDailyUsageBucket {
                    start_date: "2026-08-30".to_owned(),
                    tokens: 40,
                }],
                thread_usage: None,
            },
        )
        .unwrap();
    let period = PeriodDescriptor {
        label: "today".to_owned(),
        start: Some(Utc.with_ymd_and_hms(2026, 8, 30, 16, 0, 0).unwrap()),
        end: Some(Utc.with_ymd_and_hms(2026, 8, 31, 16, 0, 0).unwrap()),
        timezone: "Asia/Shanghai".to_owned(),
        default_grain: "hour".to_owned(),
        partial: true,
        ..PeriodDescriptor::default()
    };
    let value = official_usage_view(
        &store,
        &UsageQuery {
            account: Some("account".to_owned()),
            period: Some("today".to_owned()),
            ..UsageQuery::default()
        },
        &period,
    )
    .unwrap();
    assert!(value.get("totalTokens").unwrap().is_null());
    assert_eq!(
        value
            .get("coverageThrough")
            .and_then(serde_json::Value::as_str),
        Some("2026-08-30")
    );
}

#[test]
fn all_accounts_is_a_lower_bound_until_every_account_has_official_usage() {
    use crate::official_usage::{
        OfficialAccountUsage, OfficialDailyUsageBucket, OfficialUsageSummary,
    };
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut secondary = explorer_event("secondary", "secondary-thread", None);
    secondary.account_fingerprint = Some("secondary-account".to_owned());
    secondary.account_confidence = crate::AttributionConfidence::Inferred;
    store.upsert_event(&secondary).unwrap();
    store
        .upsert_official_account_usage(
            "primary-account",
            Utc::now(),
            &OfficialAccountUsage {
                summary: OfficialUsageSummary {
                    lifetime_tokens: Some(610),
                    peak_daily_tokens: Some(100),
                    ..OfficialUsageSummary::default()
                },
                daily_usage_buckets: vec![OfficialDailyUsageBucket {
                    start_date: Utc::now().date_naive().pred_opt().unwrap().to_string(),
                    tokens: 100,
                }],
                thread_usage: None,
            },
        )
        .unwrap();
    let query = UsageQuery {
        account: Some("all".to_owned()),
        period: Some("lifetime".to_owned()),
        metric: Some("total".to_owned()),
        ..UsageQuery::default()
    };
    let (_, period) = filter_and_period(&query, DataQuality::Confirmed);
    let all = official_usage_view(&store, &query, &period).unwrap();
    assert_eq!(all["knownAccountCount"], 2);
    assert_eq!(all["missingOfficialAccountCount"], 1);
    assert_eq!(all["totalIsLowerBound"], true);
    assert_eq!(all["authoritativeForAccountTotal"], false);
    assert_eq!(all["localTailTokens"], 0);
    assert_eq!(all["missingAccountLocalTokens"], 120);
    assert_eq!(all["localComplementTokens"], 120);
    assert_eq!(all["displayTotalTokens"], 730);
    assert_eq!(
        all["displayTotalKind"],
        "official_plus_local_tail_lower_bound"
    );
    assert_eq!(all["displayIsLowerBound"], true);

    let primary_query = UsageQuery {
        account: Some("primary-account".to_owned()),
        ..query
    };
    let (_, primary_period) = filter_and_period(&primary_query, DataQuality::Confirmed);
    let primary = official_usage_view(&store, &primary_query, &primary_period).unwrap();
    assert_eq!(primary["knownAccountCount"], 1);
    assert_eq!(primary["authoritativeForAccountTotal"], true);
    assert_eq!(primary["lifetimeTokens"], 610);
    assert_eq!(primary["displayTotalTokens"], 610);
    assert_eq!(primary["displayTotalKind"], "official");
    assert!(
        all["displayTotalTokens"].as_u64().unwrap()
            >= primary["displayTotalTokens"].as_u64().unwrap()
    );
}

#[test]
fn account_total_metric_keeps_its_definition_under_local_dimension_filters() {
    use crate::official_usage::{
        OfficialAccountUsage, OfficialDailyUsageBucket, OfficialUsageSummary,
    };
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut local_event = explorer_event("local", "thread", None);
    local_event.account_fingerprint = Some("account".to_owned());
    local_event.account_confidence = crate::AttributionConfidence::Verified;
    store.upsert_event(&local_event).unwrap();
    let mut other_event = explorer_event("other", "other-thread", None);
    other_event.account_fingerprint = Some("account".to_owned());
    other_event.account_confidence = crate::AttributionConfidence::Verified;
    other_event.model = Some("gpt-5.6-luna".to_owned());
    other_event.project.project_id = Some("other-project".to_owned());
    other_event.project.project_name = Some("Other project".to_owned());
    store.upsert_event(&other_event).unwrap();
    store
        .upsert_official_account_usage(
            "account",
            Utc::now(),
            &OfficialAccountUsage {
                summary: OfficialUsageSummary {
                    lifetime_tokens: Some(610),
                    peak_daily_tokens: Some(120),
                    ..OfficialUsageSummary::default()
                },
                daily_usage_buckets: vec![OfficialDailyUsageBucket {
                    start_date: Utc::now().date_naive().to_string(),
                    tokens: 120,
                }],
                thread_usage: None,
            },
        )
        .unwrap();
    let base = http_summary(
        &store,
        &UsageQuery {
            account: Some("account".to_owned()),
            period: Some("lifetime".to_owned()),
            metric: Some("total".to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();
    let filtered = http_summary(
        &store,
        &UsageQuery {
            account: Some("account".to_owned()),
            project: Some("project".to_owned()),
            model: Some("gpt-5.6-sol".to_owned()),
            period: Some("lifetime".to_owned()),
            metric: Some("output".to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();

    for field in [
        "value",
        "source",
        "status",
        "timezone",
        "accountScope",
        "machineScope",
        "coverage",
        "definitionId",
    ] {
        assert_eq!(
            filtered["metrics"]["accountTotal"][field], base["metrics"]["accountTotal"][field],
            "account metric field {field} changed under local filters"
        );
    }
    assert_eq!(
        filtered["metrics"]["accountTotal"]["definitionId"],
        "account_total_v1"
    );
    assert_eq!(
        filtered["metrics"]["accountTotal"]["machineScope"],
        "all_devices"
    );
    assert_eq!(base["metrics"]["localAttributedTotal"]["value"], 240);
    assert_eq!(filtered["metrics"]["localAttributedTotal"]["value"], 120);
    assert_eq!(
        filtered["metrics"]["localAttributedTotal"]["status"],
        "local_sample"
    );
    assert_eq!(filtered["official"]["primaryScope"], true);
}

#[test]
fn unknown_quality_events_do_not_claim_zero_tokens() {
    let mut store = LedgerStore::open_in_memory().unwrap();
    let mut event = explorer_event("unknown", "thread", None);
    event.quality = DataQuality::Unknown;
    store.upsert_event(&event).unwrap();

    let value = http_quality(
        &store,
        &UsageQuery {
            period: Some("lifetime".to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();

    let state = value["states"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["state"] == "unknown")
        .unwrap();
    assert!(state["tokenCount"].is_null());
    let issue = value["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "unknown-events")
        .unwrap();
    assert!(issue["tokenCount"].is_null());
}

#[test]
fn all_account_exactness_uses_common_not_latest_daily_coverage() {
    use crate::official_usage::{
        OfficialAccountUsage, OfficialDailyUsageBucket, OfficialUsageSummary,
    };
    let mut store = LedgerStore::open_in_memory().unwrap();
    let observed_at = Utc.with_ymd_and_hms(2026, 9, 1, 4, 0, 0).unwrap();
    let usage = |lifetime_tokens: u64, days: &[(&str, u64)]| OfficialAccountUsage {
        summary: OfficialUsageSummary {
            lifetime_tokens: Some(lifetime_tokens),
            peak_daily_tokens: days.iter().map(|(_, tokens)| *tokens).max(),
            ..OfficialUsageSummary::default()
        },
        daily_usage_buckets: days
            .iter()
            .map(|(date, tokens)| OfficialDailyUsageBucket {
                start_date: (*date).to_owned(),
                tokens: *tokens,
            })
            .collect(),
        thread_usage: None,
    };
    store
        .upsert_official_account_usage(
            "account-a",
            observed_at,
            &usage(100, &[("2026-08-31", 10), ("2026-09-01", 20)]),
        )
        .unwrap();
    store
        .upsert_official_account_usage("account-b", observed_at, &usage(200, &[("2026-08-31", 30)]))
        .unwrap();
    let mut tail = explorer_event("account-b-tail", "tail-thread", None);
    tail.observed_at = Utc.with_ymd_and_hms(2026, 9, 1, 3, 0, 0).unwrap();
    tail.source_timestamp = Some(tail.observed_at);
    tail.account_fingerprint = Some("account-b".to_owned());
    tail.account_confidence = crate::AttributionConfidence::Verified;
    store.upsert_event(&tail).unwrap();
    let period = PeriodDescriptor {
        label: "today".to_owned(),
        start: Some(Utc.with_ymd_and_hms(2026, 8, 31, 16, 0, 0).unwrap()),
        end: Some(observed_at),
        timezone: "Asia/Shanghai".to_owned(),
        comparison_start: Some(Utc.with_ymd_and_hms(2026, 8, 30, 16, 0, 0).unwrap()),
        comparison_end: Some(Utc.with_ymd_and_hms(2026, 8, 31, 4, 0, 0).unwrap()),
        default_grain: "hour".to_owned(),
        partial: true,
    };
    let value = official_usage_view(
        &store,
        &UsageQuery {
            account: Some("all".to_owned()),
            period: Some("today".to_owned()),
            metric: Some("total".to_owned()),
            ..UsageQuery::default()
        },
        &period,
    )
    .unwrap();

    assert_eq!(value["accountCoverageComplete"], true);
    assert_eq!(value["coverageComplete"], false);
    assert_eq!(value["authoritativeForAccountTotal"], false);
    assert_eq!(value["displayIsLowerBound"], true);
    assert_eq!(value["commonCoverageStart"], "2026-08-31");
    assert_eq!(value["commonCoverageThrough"], "2026-08-31");
    assert_eq!(value["latestCoverageThrough"], "2026-09-01");
    assert_eq!(value["accountCoverage"].as_array().unwrap().len(), 2);
    assert_eq!(value["reconciledPoints"].as_array().unwrap().len(), 1);
    assert_eq!(value["reconciledPoints"][0]["date"], "2026-09-01");
    assert_eq!(value["reconciledPoints"][0]["status"], "local_tail");
    assert_eq!(value["reconciledPoints"][0]["officialTokens"], 20);
    assert_eq!(value["reconciledPoints"][0]["localTailTokens"], 120);
    assert_eq!(value["reconciledPoints"][0]["value"], 140);
    assert_eq!(
        value["reconciledComparisonPoints"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(value["displayPreviousTotalTokens"], 40);
    assert_eq!(value["previousDisplayIsLowerBound"], false);
    assert!(value["displayDeltaTokens"].is_null());
    assert!(value["displayDeltaPercent"].is_null());
}

#[test]
fn quota_history_separates_scheduled_and_early_official_resets() {
    use crate::quota::normalize_rate_limit_event;
    let mut store = LedgerStore::open_in_memory().unwrap();
    let scheduled_reset = Utc.with_ymd_and_hms(2026, 9, 2, 0, 0, 0).unwrap();
    let early_old_reset = Utc.with_ymd_and_hms(2026, 9, 7, 0, 0, 0).unwrap();
    let early_next_reset = Utc.with_ymd_and_hms(2026, 9, 8, 0, 0, 0).unwrap();
    let append = |store: &mut LedgerStore,
                  account: &str,
                  observed_at: DateTime<Utc>,
                  used: f64,
                  reset: DateTime<Utc>| {
        let snapshot = normalize_rate_limit_event(&serde_json::json!({
            "limit_id": "weekly-pool",
            "limit_name": "Weekly pool",
            "primary": {
                "used_percent": used,
                "window_minutes": 10_080,
                "resets_at": reset.timestamp(),
            }
        }))
        .unwrap();
        store
            .append_quota_snapshot(account, "epoch", observed_at, &snapshot)
            .unwrap();
    };

    append(
        &mut store,
        "scheduled-account",
        scheduled_reset - ChronoDuration::minutes(30),
        98.0,
        scheduled_reset,
    );
    append(
        &mut store,
        "scheduled-account",
        scheduled_reset + ChronoDuration::minutes(1),
        2.0,
        early_next_reset,
    );
    append(
        &mut store,
        "scheduled-account",
        scheduled_reset + ChronoDuration::minutes(2),
        3.0,
        early_next_reset,
    );
    append(
        &mut store,
        "early-account",
        Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap(),
        91.0,
        early_old_reset,
    );
    append(
        &mut store,
        "early-account",
        Utc.with_ymd_and_hms(2026, 9, 1, 1, 0, 0).unwrap(),
        4.0,
        early_next_reset,
    );
    append(
        &mut store,
        "early-account",
        Utc.with_ymd_and_hms(2026, 9, 1, 1, 5, 0).unwrap(),
        5.0,
        early_next_reset,
    );
    append(
        &mut store,
        "stale-account",
        Utc.with_ymd_and_hms(2026, 9, 1, 2, 0, 0).unwrap(),
        45.0,
        early_next_reset,
    );
    append(
        &mut store,
        "stale-account",
        Utc.with_ymd_and_hms(2026, 9, 1, 2, 1, 0).unwrap(),
        44.0,
        early_next_reset,
    );
    append(
        &mut store,
        "stale-account",
        Utc.with_ymd_and_hms(2026, 9, 1, 2, 2, 0).unwrap(),
        45.0,
        early_next_reset,
    );

    let events = quota_reset_events(&store, &UsageQuery::default()).unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().any(|event| {
        event["accountId"] == "scheduled-account" && event["resetClass"] == "scheduled_rollover"
    }));
    assert!(events.iter().any(|event| {
        event["accountId"] == "early-account"
            && event["resetClass"] == "observed_official_reset"
            && event["title"] == "观察到官方提前重置"
    }));
}

#[test]
fn quota_cycle_reports_account_local_tokens_as_sample_not_conversion() {
    use crate::quota::normalize_rate_limit_event;
    let mut store = LedgerStore::open_in_memory().unwrap();
    let observed_at = Utc::now() - ChronoDuration::minutes(30);
    let reset = Utc::now() + ChronoDuration::days(4);
    let snapshot = |used: f64| {
        normalize_rate_limit_event(&serde_json::json!({
            "limit_id": "weekly-pool",
            "limit_name": "Weekly pool",
            "primary": {
                "used_percent": used,
                "window_minutes": 10_080,
                "resets_at": reset.timestamp(),
            }
        }))
        .unwrap()
    };
    store
        .append_quota_snapshot("account", "epoch", observed_at, &snapshot(20.0))
        .unwrap();
    store
        .append_quota_snapshot(
            "account",
            "epoch",
            observed_at + ChronoDuration::minutes(20),
            &snapshot(25.0),
        )
        .unwrap();
    let mut local = explorer_event("quota-local", "thread", None);
    local.observed_at = observed_at + ChronoDuration::minutes(10);
    local.source_timestamp = Some(local.observed_at);
    local.account_fingerprint = Some("account".to_owned());
    local.account_confidence = crate::AttributionConfidence::Verified;
    store.upsert_event(&local).unwrap();

    let cycles = quota_cycle_views(
        &store,
        &UsageQuery {
            account: Some("account".to_owned()),
            ..UsageQuery::default()
        },
    )
    .unwrap();
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0]["windowKind"], "weekly");
    assert_eq!(cycles[0]["sampleCount"], 2);
    assert_eq!(cycles[0]["localUsage"]["total"], 120);
    assert_eq!(cycles[0]["usedDeltaPercent"], 5.0);
    assert_eq!(cycles[0]["empiricalRatioIsConversion"], false);
}

#[test]
fn quota_labels_hide_internal_dynamic_pool_keys() {
    assert_eq!(
        quota_display_label(None, Some("codex"), "dynamic:id:codex"),
        "Codex 主额度"
    );
    assert_eq!(
        quota_display_label(
            Some("GPT-5.3-Codex-Spark"),
            Some("spark"),
            "dynamic:id:spark"
        ),
        "GPT-5.3-Codex-Spark"
    );
    assert_eq!(quota_duration_label(10_080), "7 天");
    assert_eq!(quota_duration_label(300), "5 小时");
}

#[test]
fn quota_cards_keep_each_accounts_latest_distinct_pool() {
    use crate::quota::normalize_rate_limit_event;
    let mut store = LedgerStore::open_in_memory().unwrap();
    let at = Utc::now();
    let snapshot = |limit_id: &str, limit_name: Option<&str>, used: f64| {
        normalize_rate_limit_event(&serde_json::json!({
            "limit_id": limit_id,
            "limit_name": limit_name,
            "primary": {
                "used_percent": used,
                "window_minutes": 10_080,
                "resets_at": (at + ChronoDuration::days(7)).timestamp(),
            }
        }))
        .unwrap()
    };
    store
        .append_quota_snapshot("account", "epoch", at, &snapshot("codex", None, 46.0))
        .unwrap();
    store
        .append_quota_snapshot(
            "account",
            "epoch",
            at + ChronoDuration::minutes(1),
            &snapshot("spark", Some("GPT-5.3-Codex-Spark"), 0.0),
        )
        .unwrap();

    let cards = quota_views(&store, &UsageQuery::default()).unwrap();
    assert_eq!(cards.len(), 2);
    assert!(cards.iter().any(|card| card["label"] == "Codex 主额度"));
    assert!(
        cards
            .iter()
            .any(|card| card["label"] == "GPT-5.3-Codex-Spark")
    );
}

#[test]
fn account_registry_keeps_provisional_history_as_observed_lower_bound() {
    use crate::official_usage::{OfficialAccountUsage, OfficialUsageSummary};
    let mut store = LedgerStore::open_in_memory().unwrap();
    let at = Utc::now();
    store
        .upsert_workspace_account_alias("workspace-a", "account-a", true, at)
        .unwrap();
    store
        .upsert_workspace_account_alias("workspace-p", "provisional-p", false, at)
        .unwrap();
    let mut canonical = explorer_event("canonical", "thread-a", None);
    canonical.account_fingerprint = Some("account-a".to_owned());
    canonical.account_confidence = crate::AttributionConfidence::Verified;
    store.upsert_event(&canonical).unwrap();
    let mut provisional = explorer_event("provisional", "thread-p", None);
    provisional.account_fingerprint = Some("provisional-p".to_owned());
    provisional.account_confidence = crate::AttributionConfidence::Inferred;
    store.upsert_event(&provisional).unwrap();
    store
        .upsert_official_account_usage(
            "account-a",
            at,
            &OfficialAccountUsage {
                summary: OfficialUsageSummary {
                    lifetime_tokens: Some(500),
                    ..OfficialUsageSummary::default()
                },
                daily_usage_buckets: Vec::new(),
                thread_usage: None,
            },
        )
        .unwrap();
    store.set_user_confirmed_account_count(Some(4)).unwrap();

    let registry = account_registry(&store).unwrap();
    assert_eq!(registry.canonical, BTreeSet::from(["account-a".to_owned()]));
    assert_eq!(
        registry.provisional,
        BTreeSet::from(["provisional-p".to_owned()])
    );
    let catalog = filter_catalog(&store).unwrap();
    let account_ids = catalog["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["id"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(account_ids, vec!["all", "account-a", "provisional-p"]);

    let query = UsageQuery {
        account: Some("all".to_owned()),
        period: Some("lifetime".to_owned()),
        metric: Some("total".to_owned()),
        ..UsageQuery::default()
    };
    let (_, period) = filter_and_period(&query, DataQuality::Confirmed);
    let view = official_usage_view(&store, &query, &period).unwrap();
    assert_eq!(view["knownAccountCount"], 4);
    assert_eq!(view["observedAccountCount"], 2);
    assert_eq!(view["userConfirmedAccountCount"], 4);
    assert_eq!(view["unobservedAccountCount"], 2);
    assert_eq!(view["verifiedAccountCount"], 1);
    assert_eq!(view["missingOfficialAccountCount"], 3);
    assert_eq!(view["provisionalIdentityCount"], 1);
    assert_eq!(view["provisionalLocalTokens"], 120);
    assert_eq!(view["identityScopeComplete"], false);
    assert_eq!(view["authoritativeForAccountTotal"], false);
    assert!(
        view["reconciledPoints"]
            .as_array()
            .unwrap()
            .iter()
            .all(|point| point["knownAccounts"] == 4 && point["status"] != "exact_official")
    );
}

#[test]
fn missing_account_residual_is_conservative_and_dimensionally_conserved() {
    use crate::official_usage::{
        OfficialAccountUsage, OfficialDailyUsageBucket, OfficialUsageSummary,
    };
    let mut store = LedgerStore::open_in_memory().unwrap();
    let observed_at = Utc.with_ymd_and_hms(2026, 9, 2, 0, 0, 0).unwrap();
    let official = |tokens: u64| OfficialAccountUsage {
        summary: OfficialUsageSummary {
            lifetime_tokens: Some(tokens),
            peak_daily_tokens: Some(tokens),
            ..OfficialUsageSummary::default()
        },
        daily_usage_buckets: vec![OfficialDailyUsageBucket {
            start_date: "2026-08-31".to_owned(),
            tokens,
        }],
        thread_usage: None,
    };
    store
        .upsert_official_account_usage("account-a", observed_at, &official(100))
        .unwrap();
    store
        .upsert_official_account_usage("account-b", observed_at, &official(200))
        .unwrap();
    store.set_user_confirmed_account_count(Some(4)).unwrap();

    let mut append = |id: &str, account: &str, day: u32, project: &str, usage: TokenUsage| {
        let mut event = explorer_event(id, id, None);
        let at = Utc.with_ymd_and_hms(2026, 8, day, 8, 0, 0).unwrap();
        event.observed_at = at;
        event.source_timestamp = Some(at);
        event.account_fingerprint = Some(account.to_owned());
        event.account_confidence = crate::AttributionConfidence::Verified;
        event.project.project_id = Some(project.to_owned());
        event.project.project_name = Some(project.to_owned());
        event.usage = usage;
        store.upsert_event(&event).unwrap();
    };
    append(
        "a-project-1",
        "account-a",
        31,
        "project-1",
        TokenUsage {
            input_tokens: 90,
            cached_input_tokens: 50,
            cache_write_input_tokens: 10,
            cache_write_observed_input_tokens: 90,
            output_tokens: 30,
            reasoning_output_tokens: 10,
            total_tokens: 120,
        },
    );
    append(
        "a-project-2",
        "account-a",
        31,
        "project-2",
        TokenUsage {
            input_tokens: 60,
            cached_input_tokens: 30,
            cache_write_input_tokens: 0,
            cache_write_observed_input_tokens: 60,
            output_tokens: 20,
            reasoning_output_tokens: 5,
            total_tokens: 80,
        },
    );
    append(
        "b-project-1",
        "account-b",
        31,
        "project-1",
        TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 80,
            cache_write_input_tokens: 5,
            cache_write_observed_input_tokens: 100,
            output_tokens: 20,
            reasoning_output_tokens: 5,
            total_tokens: 120,
        },
    );
    // This day has no official bucket and must never be treated as official zero.
    append(
        "a-uncovered",
        "account-a",
        30,
        "project-1",
        TokenUsage {
            input_tokens: 40,
            cached_input_tokens: 20,
            cache_write_input_tokens: 0,
            cache_write_observed_input_tokens: 0,
            output_tokens: 10,
            reasoning_output_tokens: 3,
            total_tokens: 50,
        },
    );

    let period = PeriodDescriptor {
        label: "lifetime".to_owned(),
        start: Some(Utc.with_ymd_and_hms(2026, 8, 29, 16, 0, 0).unwrap()),
        end: Some(observed_at),
        timezone: "Asia/Shanghai".to_owned(),
        ..PeriodDescriptor::default()
    };
    let all = missing_account_estimate(&store, &UsageQuery::default(), &period).unwrap();
    assert_eq!(all["status"], "conservative_floor");
    assert_eq!(all["combinedUnobservedAccountCount"], 2);
    assert_eq!(all["alignedAccountDays"], 2);
    assert_eq!(all["excessAccountDays"], 1);
    assert_eq!(all["excludedAccountDays"], 1);
    assert_eq!(all["rawResidualTokens"], 100);
    assert_eq!(all["totalUsage"]["total"], 100);
    assert_eq!(all["totalUsage"]["input"], 75);
    assert_eq!(all["totalUsage"]["cached"], 40);
    assert_eq!(all["totalUsage"]["cacheWrite"], 5);
    assert_eq!(all["totalUsage"]["uncached"], 30);
    assert_eq!(all["totalUsage"]["output"], 25);
    assert_eq!(all["totalUsage"]["cacheWriteCoverage"], 1.0);
    assert_eq!(all["allocationRoundingDelta"], 0);
    assert_eq!(all["componentInvariantMismatchTokens"], 0);
    assert_eq!(
        all["byProject"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["usage"]["total"].as_u64().unwrap())
            .sum::<u64>(),
        100
    );

    let project = missing_account_estimate(
        &store,
        &UsageQuery {
            project: Some("project-1".to_owned()),
            ..UsageQuery::default()
        },
        &period,
    )
    .unwrap();
    assert_eq!(project["selectedUsage"]["total"], 60);
    assert_eq!(project["totalUsage"]["total"], 100);

    let single = missing_account_estimate(
        &store,
        &UsageQuery {
            account: Some("account-a".to_owned()),
            ..UsageQuery::default()
        },
        &period,
    )
    .unwrap();
    assert_eq!(single["applicable"], false);
    assert_eq!(single["status"], "not_applicable_to_single_account");
}

#[test]
fn project_attribution_coverage_explains_the_gap_without_allocating_it() {
    use crate::official_usage::{
        OfficialAccountUsage, OfficialDailyUsageBucket, OfficialUsageSummary,
    };
    let mut store = LedgerStore::open_in_memory().unwrap();
    let end = Utc.with_ymd_and_hms(2026, 6, 4, 0, 0, 0).unwrap();
    store
        .upsert_official_account_usage(
            "account-a",
            end,
            &OfficialAccountUsage {
                summary: OfficialUsageSummary {
                    lifetime_tokens: Some(1_000),
                    peak_daily_tokens: Some(600),
                    ..OfficialUsageSummary::default()
                },
                daily_usage_buckets: vec![
                    OfficialDailyUsageBucket {
                        start_date: "2026-06-01".to_owned(),
                        tokens: 100,
                    },
                    OfficialDailyUsageBucket {
                        start_date: "2026-06-02".to_owned(),
                        tokens: 300,
                    },
                    OfficialDailyUsageBucket {
                        start_date: "2026-06-03".to_owned(),
                        tokens: 600,
                    },
                ],
                thread_usage: None,
            },
        )
        .unwrap();
    let mut named = explorer_event("named", "named-thread", None);
    named.observed_at = Utc.with_ymd_and_hms(2026, 6, 2, 2, 0, 0).unwrap();
    named.source_timestamp = Some(named.observed_at);
    named.account_fingerprint = Some("account-a".to_owned());
    named.account_confidence = crate::AttributionConfidence::Verified;
    named.project.project_id = Some("project-a".to_owned());
    named.project.project_name = Some("Project A".to_owned());
    named.usage.input_tokens = 70;
    named.usage.cached_input_tokens = 50;
    named.usage.cache_write_observed_input_tokens = 70;
    named.usage.output_tokens = 10;
    named.usage.total_tokens = 80;
    store.upsert_event(&named).unwrap();

    let mut unassigned = explorer_event("unassigned", "unassigned-thread", None);
    unassigned.observed_at = Utc.with_ymd_and_hms(2026, 6, 2, 3, 0, 0).unwrap();
    unassigned.source_timestamp = Some(unassigned.observed_at);
    unassigned.account_fingerprint = Some("account-a".to_owned());
    unassigned.account_confidence = crate::AttributionConfidence::Verified;
    unassigned.project.project_id = None;
    unassigned.project.project_name = None;
    unassigned.usage.input_tokens = 15;
    unassigned.usage.cached_input_tokens = 10;
    unassigned.usage.cache_write_observed_input_tokens = 15;
    unassigned.usage.output_tokens = 5;
    unassigned.usage.total_tokens = 20;
    store.upsert_event(&unassigned).unwrap();

    let query = UsageQuery {
        period: Some("lifetime".to_owned()),
        account: Some("all".to_owned()),
        project: Some("all".to_owned()),
        model: Some("all".to_owned()),
        ..UsageQuery::default()
    };
    let period = PeriodDescriptor {
        label: "lifetime".to_owned(),
        end: Some(end),
        timezone: "Asia/Shanghai".to_owned(),
        default_grain: "month".to_owned(),
        ..PeriodDescriptor::default()
    };
    let official = official_usage_view(&store, &query, &period).unwrap();
    let coverage = project_attribution_coverage(
        &store,
        &query,
        &period,
        &official,
        TokenUsage {
            input_tokens: 85,
            cached_input_tokens: 60,
            cache_write_input_tokens: 0,
            cache_write_observed_input_tokens: 85,
            output_tokens: 15,
            reasoning_output_tokens: 0,
            total_tokens: 100,
        },
    )
    .unwrap();
    assert_eq!(coverage["accountTotalTokens"], 1_000);
    assert_eq!(coverage["localAttributedTokens"], 100);
    assert_eq!(coverage["namedProjectTokens"], 80);
    assert_eq!(coverage["unassignedTokens"], 20);
    assert_eq!(coverage["standaloneConversationTokens"], 0);
    assert_eq!(coverage["unattributedTokens"], 0);
    assert_eq!(coverage["coverageRatio"], 1.0);
    assert_eq!(coverage["gapBuckets"][0]["tokens"], 100);
    assert_eq!(coverage["gapBuckets"][1]["tokens"], 600);
    assert_eq!(
        coverage["gapBuckets"][2]["id"],
        "overlap_and_unbucketed_gap"
    );
    assert_eq!(coverage["gapBuckets"][2]["tokens"], 200);
    assert_eq!(coverage["canAllocateGapToProjects"], false);
}

#[test]
fn thread_labels_never_expose_raw_delegation_or_long_prompt_text() {
    let base = CatalogThread {
        thread_id: "019f597f-7642-7b80-b3c9-30cb63764e15".to_owned(),
        parent_thread_id: None,
        project_id: None,
        project_name: None,
        title: Some("<codex_delegation>secret task payload</codex_delegation>".to_owned()),
        model: None,
        agent_nickname: None,
        agent_role: None,
        agent_path: None,
        depth: 0,
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
        archived: false,
        has_user_event: true,
        source_kind: "state_5".to_owned(),
        present_in_codex: true,
    };
    assert_eq!(thread_label(&base), "Session 019f597f-7642");

    let mut subagent = base.clone();
    subagent.depth = 1;
    assert_eq!(thread_label(&subagent), "Subagent 019f597f-7642");

    let mut concise = base;
    concise.title = Some("Fix account reconciliation".to_owned());
    assert_eq!(thread_label(&concise), "Fix account reconciliation");
    concise.title = Some("Role owner for /Users/g/private/project".to_owned());
    assert_eq!(thread_label(&concise), "Session 019f597f-7642");
    concise.title = Some("Configure key sk-example0123456789abcdefghijklmnopqrstuvwxyz".to_owned());
    assert_eq!(thread_label(&concise), "Session 019f597f-7642");
    concise.title = Some("Connect to internal service at 192.168.8.21".to_owned());
    assert_eq!(thread_label(&concise), "Session 019f597f-7642");
    concise.title = Some("Contact owner@example.internal for access".to_owned());
    assert_eq!(thread_label(&concise), "Session 019f597f-7642");
    concise.title = Some("Run tool --credential value --endpoint internal --verbose ".repeat(2));
    assert_eq!(thread_label(&concise), "Session 019f597f-7642");
}
