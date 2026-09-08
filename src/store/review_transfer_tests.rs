use super::*;
use crate::store::tests::event;

fn insert(store: &LedgerStore, value: UsageEvent) {
    let tx = store.connection.unchecked_transaction().unwrap();
    upsert_reconstruction_event_in(
        &tx,
        &ReconstructionEvent {
            event: value,
            counter_epoch: 0,
        },
    )
    .unwrap();
    tx.commit().unwrap();
}

#[test]
fn transfer_preserves_new_and_edited_rows_and_never_changes_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.sqlite3");
    let review = temp.path().join("review.sqlite3");
    let output = temp.path().join("candidate.sqlite3");
    let mut original = LedgerStore::open(&source).unwrap();
    for (id, offset) in [("replace", 1), ("remove", 2), ("keep", 3)] {
        insert(&original, event(id, DataQuality::Confirmed, offset));
    }
    original
        .upsert_event(&event("sample", DataQuality::Confirmed, 4))
        .unwrap();
    original.set_user_confirmed_account_count(Some(4)).unwrap();
    drop(original);
    create_review_shadow(&source, &review).unwrap();
    let mut corrected = LedgerStore::open(&review).unwrap();
    corrected.connection.execute_batch("INSERT INTO review_old_reconstruction SELECT 'test-seal',r.* FROM reconstruction_usage_events r;
        DELETE FROM reconstruction_usage_events WHERE event_id IN ('replace','remove');").unwrap();
    let mut replacement = event("replace", DataQuality::Confirmed, 1);
    replacement.usage.input_tokens = 80;
    replacement.usage.total_tokens = 100;
    replacement.usage.cache_write_observed_input_tokens = 0;
    replacement.provenance.source_record_key = Some("replacement-key".into());
    insert(&corrected, replacement);
    let mut sample = event("sample", DataQuality::Confirmed, 4);
    sample.provenance.source_record_key = Some("sample-key".into());
    corrected.upsert_event(&sample).unwrap();
    corrected.set_user_confirmed_account_count(Some(9)).unwrap();
    for _ in 0..10 {
        if corrected
            .stage_source_union_batch(100, 100, 1000)
            .unwrap()
            .projection_ready
        {
            break;
        }
    }
    drop(corrected);
    let original = LedgerStore::open(&source).unwrap();
    original
        .connection
        .execute(
            "UPDATE reconstruction_usage_events SET model='user-edited' WHERE event_id='keep'",
            [],
        )
        .unwrap();
    insert(&original, event("new", DataQuality::Confirmed, 5));
    drop(original);
    let source_hash = fingerprint(&source).unwrap();
    let review_hash = fingerprint(&review).unwrap();
    assert!(prepare_review_transfer(&source, &review, &output, "wrong", &review_hash).is_err());
    assert!(!output.exists());
    let report =
        prepare_review_transfer(&source, &review, &output, &source_hash, &review_hash).unwrap();
    assert_eq!(
        (
            report.source_rows,
            report.matched_rows,
            report.replacement_rows,
            report.replay_rows,
            report.preserved_rows,
            report.output_rows
        ),
        (4, 2, 1, 1, 2, 3)
    );
    assert_eq!(report.sampling_keys_added, 1);
    assert_eq!(fingerprint(&source).unwrap(), source_hash);
    assert_eq!(fingerprint(&review).unwrap(), review_hash);
    let result = LedgerStore::open_read_only(&output).unwrap();
    assert_eq!(result.user_confirmed_account_count().unwrap(), Some(4));
    let (model, total): (String, i64) = result
        .connection
        .query_row(
            "SELECT model,total_tokens FROM reconstruction_usage_events WHERE event_id='keep'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(model, "user-edited");
    assert_eq!(total, 120);
    assert_eq!(
        count(
            &result.connection,
            "SELECT COUNT(*) FROM transfer_original_reconstruction"
        )
        .unwrap(),
        2
    );
    assert_eq!(
        count(
            &result.connection,
            "SELECT total_tokens FROM reconstruction_usage_events WHERE event_id='replace'"
        )
        .unwrap(),
        100
    );
    assert!(
        prepare_review_transfer(&source, &review, &output, &source_hash, &review_hash).is_err()
    );
}
