use super::*;

pub(super) fn deduplicate_sampling_receipt_in(
    transaction: &rusqlite::Transaction<'_>,
    event: &UsageEvent,
) -> StoreResult<Option<UpsertOutcome>> {
    let Some(receipt) = event.provenance.sampling_receipt_key.as_deref() else {
        return Ok(None);
    };
    if event.quality == DataQuality::Confirmed {
        event
            .usage
            .validate()
            .map_err(StoreError::InvalidConfirmedUsage)?;
    }
    let previous: Option<String> = transaction
        .query_row(
            "SELECT receipt_key FROM sampling_source_receipts WHERE event_id=?1",
            params![event.event_id],
            |row| row.get(0),
        )
        .optional()?;
    if previous.as_deref().is_some_and(|value| value != receipt) {
        return Err(StoreError::SamplingReceiptConflict(event.event_id.clone()));
    }
    let owner: Option<String> = transaction
        .query_row(
            "SELECT event_id FROM sampling_receipt_owners WHERE receipt_key=?1",
            params![receipt],
            |row| row.get(0),
        )
        .optional()?;
    let Some(owner) = owner else {
        let legacy_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM sampling_source_receipts WHERE receipt_key=?1",
            params![receipt],
            |row| row.get(0),
        )?;
        if legacy_count > 0 {
            return Err(StoreError::SamplingReceiptConflict(event.event_id.clone()));
        }
        transaction.execute(
            "INSERT INTO sampling_receipt_owners(receipt_key,event_id) VALUES (?1,?2)",
            params![receipt, event.event_id],
        )?;
        return Ok(None);
    };
    let existing = transaction
        .query_row(
            "SELECT quality,input_tokens,cached_input_tokens,cache_write_input_tokens,
            cache_write_observed_input_tokens,output_tokens,reasoning_output_tokens,total_tokens,
            thread_id,effective_at,model FROM retained_request_evidence WHERE event_id=?1",
            params![owner],
            |row| {
                Ok((
                    parse_quality_column(&row.get::<_, String>(0)?, 0)?,
                    TokenUsage {
                        input_tokens: u64_from_sql(row.get(1)?, 1)?,
                        cached_input_tokens: u64_from_sql(row.get(2)?, 2)?,
                        cache_write_input_tokens: u64_from_sql(row.get(3)?, 3)?,
                        cache_write_observed_input_tokens: u64_from_sql(row.get(4)?, 4)?,
                        output_tokens: u64_from_sql(row.get(5)?, 5)?,
                        reasoning_output_tokens: u64_from_sql(row.get(6)?, 6)?,
                        total_tokens: u64_from_sql(row.get(7)?, 7)?,
                    },
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::SamplingReceiptConflict(event.event_id.clone()))?;
    if existing.2 != event.thread_id
        || existing.3 != timestamp(event.source_timestamp.unwrap_or(event.observed_at))
        || existing.4 != event.model
    {
        return Err(StoreError::SamplingReceiptConflict(event.event_id.clone()));
    }
    if owner == event.event_id {
        let raw_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM usage_events WHERE event_id=?1)",
            params![owner],
            |row| row.get(0),
        )?;
        if existing.0 == DataQuality::Confirmed && event.quality == DataQuality::Unknown {
            return Ok(Some(UpsertOutcome::Unchanged));
        }
        if existing.0 == event.quality && existing.1 == event.usage {
            if !raw_exists {
                retain_request_evidence_in(transaction, event, false)?;
                return Ok(Some(UpsertOutcome::Unchanged));
            }
            return Ok(None);
        }
        if raw_exists
            && existing.0 == DataQuality::Unknown
            && event.quality == DataQuality::Confirmed
        {
            return Ok(None);
        }
        return Err(StoreError::SamplingReceiptConflict(event.event_id.clone()));
    }
    let outcome = if existing.0 == event.quality && existing.1 == event.usage
        || existing.0 == DataQuality::Confirmed && event.quality == DataQuality::Unknown
    {
        UpsertOutcome::Unchanged
    } else if existing.0 == DataQuality::Unknown && event.quality == DataQuality::Confirmed {
        let mut canonical = transaction
            .query_row(
                &format!("SELECT {EVENT_SELECT_COLUMNS} FROM usage_events WHERE event_id=?1"),
                params![owner],
                row_to_event,
            )
            .optional()?
            .ok_or_else(|| StoreError::SamplingReceiptConflict(event.event_id.clone()))?;
        canonical.usage = event.usage;
        canonical.quality = DataQuality::Confirmed;
        canonical.quality_reason = None;
        canonical.provenance.source_turn_id = event.provenance.source_turn_id.clone();
        canonical.provenance.candidate_rollout_event_id =
            event.provenance.candidate_rollout_event_id.clone();
        canonical.provenance.sampling_receipt_key = Some(receipt.to_owned());
        canonical.provenance.source_record_key = event.provenance.source_record_key.clone();
        upsert_event_in(transaction, &canonical)?
    } else {
        return Err(StoreError::SamplingReceiptConflict(event.event_id.clone()));
    };
    transaction.execute(
        "INSERT INTO sampling_source_receipts(event_id,receipt_key) VALUES (?1,?2)
         ON CONFLICT(event_id) DO NOTHING",
        params![event.event_id, receipt],
    )?;
    Ok(Some(outcome))
}
