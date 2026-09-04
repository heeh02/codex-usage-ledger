use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Raw token dimensions emitted by Codex. Cache reads and cache writes are
/// mutually exclusive subsets of input. `cache_write_observed_input_tokens`
/// is a coverage weight: it equals input for events whose source exposed the
/// cache-write field, and zero for legacy events where that split is unknown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub cache_write_input_tokens: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub cache_write_observed_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TokenUsageInvariantError {
    #[error("cache-read plus cache-write input exceeds total input")]
    CacheBucketsExceedInput,
    #[error("cache-write coverage weight exceeds total input")]
    CacheWriteCoverageExceedsInput,
    #[error("total tokens do not equal input plus output")]
    TotalDoesNotConserve,
    #[error("reasoning tokens exceed output tokens")]
    ReasoningExceedsOutput,
}

impl TokenUsage {
    pub fn uncached_input_tokens(self) -> u64 {
        self.input_tokens
            .saturating_sub(self.cached_input_tokens)
            .saturating_sub(self.cache_write_input_tokens)
    }

    pub fn cache_write_coverage(self) -> f64 {
        if self.input_tokens == 0 {
            0.0
        } else {
            (self.cache_write_observed_input_tokens as f64 / self.input_tokens as f64)
                .clamp(0.0, 1.0)
        }
    }

    pub fn checked_delta(self, previous: Self) -> Option<Self> {
        Some(Self {
            input_tokens: self.input_tokens.checked_sub(previous.input_tokens)?,
            cached_input_tokens: self
                .cached_input_tokens
                .checked_sub(previous.cached_input_tokens)?,
            cache_write_input_tokens: self
                .cache_write_input_tokens
                .checked_sub(previous.cache_write_input_tokens)?,
            cache_write_observed_input_tokens: self
                .cache_write_observed_input_tokens
                .checked_sub(previous.cache_write_observed_input_tokens)?,
            output_tokens: self.output_tokens.checked_sub(previous.output_tokens)?,
            reasoning_output_tokens: self
                .reasoning_output_tokens
                .checked_sub(previous.reasoning_output_tokens)?,
            total_tokens: self.total_tokens.checked_sub(previous.total_tokens)?,
        })
    }

    pub fn is_zero(self) -> bool {
        self.input_tokens == 0
            && self.cached_input_tokens == 0
            && self.cache_write_input_tokens == 0
            && self.cache_write_observed_input_tokens == 0
            && self.output_tokens == 0
            && self.reasoning_output_tokens == 0
            && self.total_tokens == 0
    }

    pub fn validate(self) -> Result<(), TokenUsageInvariantError> {
        if self
            .cached_input_tokens
            .checked_add(self.cache_write_input_tokens)
            .is_none_or(|cached| cached > self.input_tokens)
        {
            return Err(TokenUsageInvariantError::CacheBucketsExceedInput);
        }
        if self.cache_write_observed_input_tokens > self.input_tokens {
            return Err(TokenUsageInvariantError::CacheWriteCoverageExceedsInput);
        }
        if self.input_tokens.checked_add(self.output_tokens) != Some(self.total_tokens) {
            return Err(TokenUsageInvariantError::TotalDoesNotConserve);
        }
        if self.reasoning_output_tokens > self.output_tokens {
            return Err(TokenUsageInvariantError::ReasoningExceedsOutput);
        }
        Ok(())
    }
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataQuality {
    Confirmed,
    Quarantined,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionConfidence {
    Verified,
    Inferred,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventProvenance {
    pub machine_id: String,
    pub source_id: String,
    pub rollout_id: String,
    pub file_identity: String,
    pub byte_offset: u64,
    pub line_number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAttribution {
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub confidence: AttributionConfidence,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageEvent {
    pub event_id: String,
    pub observed_at: DateTime<Utc>,
    pub source_timestamp: Option<DateTime<Utc>>,
    pub thread_id: Option<String>,
    pub parent_thread_id: Option<String>,
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub account_fingerprint: Option<String>,
    pub account_confidence: AttributionConfidence,
    pub project: ProjectAttribution,
    pub usage: TokenUsage,
    pub quality: DataQuality,
    pub quality_reason: Option<String>,
    pub provenance: EventProvenance,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_usage() -> TokenUsage {
        TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 60,
            cache_write_input_tokens: 10,
            cache_write_observed_input_tokens: 100,
            output_tokens: 20,
            reasoning_output_tokens: 5,
            total_tokens: 120,
        }
    }

    #[test]
    fn token_usage_validation_covers_every_conservation_invariant() {
        assert_eq!(valid_usage().validate(), Ok(()));

        let mut invalid = valid_usage();
        invalid.total_tokens = 121;
        assert_eq!(
            invalid.validate(),
            Err(TokenUsageInvariantError::TotalDoesNotConserve)
        );

        let mut invalid = valid_usage();
        invalid.reasoning_output_tokens = 21;
        assert_eq!(
            invalid.validate(),
            Err(TokenUsageInvariantError::ReasoningExceedsOutput)
        );

        let mut invalid = valid_usage();
        invalid.cache_write_input_tokens = 41;
        assert_eq!(
            invalid.validate(),
            Err(TokenUsageInvariantError::CacheBucketsExceedInput)
        );
    }
}
