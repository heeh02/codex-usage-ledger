fn main() {
    // The CLI's async command dispatcher exceeds Windows' default 1 MiB main
    // stack in debug builds. Match the 8 MiB main-stack budget used on Unix.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bin=codex-usage-ledger=/STACK:8388608");
    }
}
