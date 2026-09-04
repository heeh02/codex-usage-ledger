fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = schemars::schema_for!(codex_usage_ledger::api::wire::DashboardBundle);
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}
