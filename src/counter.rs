//! Numeric normalization shared by rollout consumers. Stream ownership and
//! replay boundaries must be established by adapters before calling this code.
use crate::types::TokenUsage;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CounterStep {
    pub usage: Option<TokenUsage>,
    pub initial_prefix: Option<TokenUsage>,
    pub unchanged: bool,
    pub reset: bool,
}

fn amounts(mut usage: TokenUsage) -> TokenUsage {
    usage.cache_write_observed_input_tokens = 0;
    usage
}

fn delta(total: TokenUsage, previous: TokenUsage) -> Option<TokenUsage> {
    let mut delta = amounts(total).checked_delta(amounts(previous))?;
    // Coverage is metadata, not consumption. Missing/decreasing coverage must
    // not make an increasing numeric counter look like a reset/model call.
    delta.cache_write_observed_input_tokens = total
        .cache_write_observed_input_tokens
        .checked_sub(previous.cache_write_observed_input_tokens)
        .filter(|weight| *weight <= delta.input_tokens)
        .unwrap_or(0);
    delta.validate().ok()?;
    Some(delta)
}

pub(crate) fn normalize_counter(
    previous: Option<TokenUsage>,
    total: TokenUsage,
    last: Option<TokenUsage>,
) -> CounterStep {
    let last = last.filter(|usage| usage.validate().is_ok());
    match previous {
        None => CounterStep {
            usage: last,
            initial_prefix: last.map_or(Some(total), |last| delta(total, last)),
            unchanged: false,
            reset: false,
        },
        Some(previous) if amounts(total) == amounts(previous) => CounterStep {
            usage: None,
            initial_prefix: None,
            unchanged: true,
            reset: false,
        },
        Some(previous) => match delta(total, previous) {
            Some(usage) => CounterStep {
                usage: Some(usage),
                initial_prefix: None,
                unchanged: false,
                reset: false,
            },
            None => CounterStep {
                usage: last,
                initial_prefix: None,
                unchanged: false,
                reset: true,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(input: u64, output: u64, weight: u64) -> TokenUsage {
        TokenUsage {
            input_tokens: input,
            output_tokens: output,
            cached_input_tokens: input / 2,
            total_tokens: input + output,
            cache_write_observed_input_tokens: weight,
            ..TokenUsage::default()
        }
    }

    #[test]
    fn counters_ignore_stale_last_and_coverage_only_changes() {
        let original = usage(100, 10, 100);
        let without_coverage = usage(100, 10, 0);
        let repeated = normalize_counter(Some(original), without_coverage, Some(original));
        assert!(repeated.unchanged);
        assert_eq!(repeated.usage, None);
        let increased =
            normalize_counter(Some(original), usage(140, 20, 0), Some(usage(900, 90, 0)));
        assert!(!increased.reset);
        assert_eq!(increased.usage, Some(usage(40, 10, 0)));
    }

    #[test]
    fn initial_prefix_and_reset_never_emit_an_unidentified_total() {
        let total = usage(1000, 100, 0);
        let first = normalize_counter(None, total, Some(usage(100, 10, 0)));
        assert_eq!(first.initial_prefix, Some(usage(900, 90, 0)));
        assert_eq!(first.usage, Some(usage(100, 10, 0)));
        assert_eq!(normalize_counter(None, total, None).usage, None);
        let reset = normalize_counter(Some(total), usage(50, 5, 0), None);
        assert!(reset.reset);
        assert_eq!(reset.usage, None);
    }
}
