//! Correctness-first local accounting for Codex usage logs.
//!
//! The ledger stores event deltas, provenance and uncertainty. It never treats
//! a thread's final cumulative total as newly sampled usage.

mod account_history;
pub mod api;
mod identity;
// Retained parser entry points support synthetic diagnostics and future source
// adapters even when the production CLI uses the narrower sampling path.
#[allow(dead_code)]
mod ingest;
mod official_usage;
mod project;
mod quota;
mod reconstruction;
// Replay primitives remain private implementation detail; some are exercised
// only by invariant tests and recovery tooling.
#[allow(dead_code)]
mod replay;
#[allow(dead_code)]
mod runtime;
mod sampling;
mod store;
mod types;

pub use store::{
    AggregateDimension, AggregateFilter, CollectorStatus, LedgerStore, LedgerTableCounts,
};
pub use types::{
    AttributionConfidence, DataQuality, EventProvenance, ProjectAttribution, TokenUsage, UsageEvent,
};

/// Binary-only integration surface. This is public because Cargo builds the CLI
/// as a separate crate; it is not a stable third-party SDK contract.
#[doc(hidden)]
pub mod cli_support {
    pub use crate::account_history::sync_account_history;
    pub use crate::official_usage::fetch_official_account_usage;
    pub use crate::reconstruction::{
        ingest_reconstruction_batch, ingest_reconstruction_batch_for_project,
    };
    pub use crate::runtime::{
        AccountBinding, compact_expired_raw_events, discover_rollouts, ingest_quota_tails,
        load_or_create_hmac_key, load_or_create_machine_id, observe_auth, prepare_fast_ledger,
        prepare_store, sync_native_catalog,
    };
    pub use crate::sampling::{POST_SAMPLING_SOURCE_ID, ingest_post_sampling};
    pub use crate::store::{
        AggregateDimension, AggregateFilter, CollectorStatus, LedgerStore, LedgerTableCounts,
    };
}
