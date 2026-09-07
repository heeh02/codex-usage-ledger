use super::*;

fn fact(id: &str, at: &str, account: Option<&str>, model: &str) -> ReconstructionAuditFact {
    ReconstructionAuditFact {
        event_id: id.into(),
        stored_hash: None,
        at: at.parse().unwrap(),
        thread: Some("thread".into()),
        model: Some(model.into()),
        account: account.map(str::to_owned),
        project: Some("project".into()),
        record_key: None,
        usage: TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 40,
            cache_write_input_tokens: 10,
            cache_write_observed_input_tokens: 100,
            output_tokens: 20,
            reasoning_output_tokens: 5,
            total_tokens: 120,
        },
    }
}
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("preview.sqlite3");
    let connection = new_preview(&path).unwrap();
    let a = fact("a", "2026-01-31T23:59:59.950Z", None, "model-a");
    let b = fact("b", "2026-02-01T00:00:00.050Z", Some("unknown"), "model-b");
    let c = fact("c", "2026-03-08T07:00:00Z", Some("account"), "model-b");
    for (i, item) in [&a, &b, &c].into_iter().enumerate() {
        insert_fact(&connection, "old", i as u64, item).unwrap();
    }
    for (i, item) in [&b, &c].into_iter().enumerate() {
        insert_fact(&connection, "candidate", i as u64, item).unwrap();
    }
    connection
        .execute(
            "UPDATE preview_meta SET ready=1,manifest_sha256=?1",
            ["a".repeat(64)],
        )
        .unwrap();
    (temp, path)
}
fn conserves(side: &PreviewSide) {
    for buckets in [
        &side.by_time,
        &side.by_account,
        &side.by_project,
        &side.by_model,
        &side.by_thread,
    ] {
        assert_eq!(buckets.iter().map(|b| b.events).sum::<u64>(), side.events);
        let mut total = None;
        for bucket in buckets {
            crate::source_union::add(total.get_or_insert_default(), bucket.usage.unwrap()).unwrap();
        }
        assert_eq!(total, side.usage);
    }
}
#[test]
fn all_grains_timezones_and_dimensions_preserve_both_alternatives() {
    let (_temp, path) = fixture();
    let before = std::fs::read(&path).unwrap();
    for timezone in ["UTC", "Asia/Shanghai", "America/New_York"] {
        for grain in [
            CorrectionPreviewGrain::Day,
            CorrectionPreviewGrain::Week,
            CorrectionPreviewGrain::Month,
            CorrectionPreviewGrain::Year,
        ] {
            let report = read_correction_preview(
                &path,
                &CorrectionPreviewFilter {
                    timezone: timezone.into(),
                    grain,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(report.old.usage.unwrap().total_tokens, 360);
            assert_eq!(report.candidate.usage.unwrap().total_tokens, 240);
            conserves(&report.old);
            conserves(&report.candidate);
            assert!(!report.production_policy_changed && !report.migration_authorized);
        }
    }
    let utc = read_correction_preview(
        &path,
        &CorrectionPreviewFilter {
            timezone: "UTC".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(utc.old.by_time.len(), 3);
    let china = read_correction_preview(&path, &CorrectionPreviewFilter::default()).unwrap();
    assert_eq!(china.old.by_time.len(), 2);
    assert_eq!(
        china
            .old
            .by_account
            .iter()
            .map(|b| b.key.clone())
            .collect::<Vec<_>>(),
        vec![None, Some("account".into()), Some("unknown".into())]
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn exact_half_open_filters_do_not_round_or_turn_missing_into_zero() {
    let (_temp, path) = fixture();
    let filter = CorrectionPreviewFilter {
        start: Some("2026-01-31T23:59:59.950Z".parse().unwrap()),
        end: Some("2026-02-01T00:00:00.050Z".parse().unwrap()),
        ..Default::default()
    };
    let report = read_correction_preview(&path, &filter).unwrap();
    assert_eq!(report.old.events, 1);
    assert_eq!(report.candidate.events, 0);
    assert!(report.candidate.usage.is_none());
    conserves(&report.old);
    conserves(&report.candidate);
    for filter in [
        CorrectionPreviewFilter {
            model: Some("model-a".into()),
            ..Default::default()
        },
        CorrectionPreviewFilter {
            account: Some("unknown".into()),
            ..Default::default()
        },
        CorrectionPreviewFilter {
            project: Some("project".into()),
            ..Default::default()
        },
    ] {
        let report = read_correction_preview(&path, &filter).unwrap();
        conserves(&report.old);
        conserves(&report.candidate);
    }
    let unknown = read_correction_preview(
        &path,
        &CorrectionPreviewFilter {
            account: Some("unknown".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(unknown.old.events, 1, "literal unknown is not NULL");
    let injection = read_correction_preview(
        &path,
        &CorrectionPreviewFilter {
            model: Some("x' OR 1=1 --".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(injection.old.usage.is_none());
}
#[test]
fn observed_zero_invalid_ranges_incomplete_and_foreign_files_stay_distinct() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("preview.sqlite3");
    assert!(read_correction_preview(&path, &CorrectionPreviewFilter::default()).is_err());
    assert!(!path.exists());
    let connection = new_preview(&path).unwrap();
    assert!(read_correction_preview(&path, &CorrectionPreviewFilter::default()).is_err());
    let mut zero = fact("zero", "2026-01-01T00:00:00Z", None, "model");
    zero.usage = TokenUsage::default();
    insert_fact(&connection, "old", 0, &zero).unwrap();
    connection
        .execute(
            "UPDATE preview_meta SET ready=1,manifest_sha256=?1",
            ["b".repeat(64)],
        )
        .unwrap();
    let report = read_correction_preview(&path, &CorrectionPreviewFilter::default()).unwrap();
    assert_eq!(report.old.events, 1);
    assert_eq!(report.old.usage, Some(TokenUsage::default()));
    assert!(report.candidate.usage.is_none());
    assert!(
        read_correction_preview(
            &path,
            &CorrectionPreviewFilter {
                timezone: "bad".into(),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        read_correction_preview(
            &path,
            &CorrectionPreviewFilter {
                start: Some(zero.at),
                end: Some(zero.at),
                ..Default::default()
            }
        )
        .is_err()
    );
    connection.pragma_update(None, "application_id", 0).unwrap();
    assert!(read_correction_preview(&path, &CorrectionPreviewFilter::default()).is_err());
}
