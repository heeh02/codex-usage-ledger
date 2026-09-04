#[test]
fn checked_in_dashboard_schema_matches_rust_dtos() {
    let generated = serde_json::to_value(schemars::schema_for!(
        codex_usage_ledger::api::wire::DashboardBundle
    ))
    .expect("serialize generated dashboard schema");
    let checked_in: serde_json::Value =
        serde_json::from_str(include_str!("../web/src/api/dashboard-bundle.schema.json"))
            .expect("parse checked-in dashboard schema");
    assert_eq!(
        generated, checked_in,
        "regenerate the dashboard API contract"
    );
}
