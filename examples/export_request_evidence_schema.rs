fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = if std::env::args().any(|arg| arg == "--quota-interval-usage") {
        schemars::schema_for!(codex_usage_ledger::api::wire::QuotaIntervalUsageResponse)
    } else if std::env::args().any(|arg| arg == "--quota-history") {
        schemars::schema_for!(codex_usage_ledger::api::wire::QuotaHistoryResponse)
    } else if std::env::args().any(|arg| arg == "--turns") {
        schemars::schema_for!(codex_usage_ledger::api::wire::TurnEvidenceResponse)
    } else {
        schemars::schema_for!(codex_usage_ledger::api::wire::RequestEvidenceResponse)
    };
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}
