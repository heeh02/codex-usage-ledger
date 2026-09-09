//! Separate history admission from live-tail service. Token facts do not depend
//! on scheduling order, and neither lane may permanently consume every slot.
use super::*;

type Work = (ReconstructionSourceStatus, Target, u64);
const SOURCE: &str = "reconstruction-scheduler-v1";

#[derive(Default, Serialize, Deserialize)]
struct Schedule {
    last_tail: Option<String>,
    next_single_tail: bool,
}

pub(super) fn select(
    store: &mut LedgerStore,
    machine: &str,
    queue: Vec<Work>,
    upgrades: &HashSet<String>,
    budget: usize,
) -> Result<Vec<Work>> {
    if queue.is_empty() {
        return Ok(Vec::new());
    }
    let saved = store.get_cursor(machine, SOURCE)?;
    let mut state: Schedule = match saved
        .as_ref()
        .and_then(|cursor| cursor.parser_state_json.as_deref())
    {
        Some(value) => {
            serde_json::from_str(value).map_err(|_| SourceContinuityError::CheckpointUnavailable)?
        }
        None => Schedule::default(),
    };
    let selected = choose(queue, upgrades, budget.max(1), &mut state);
    let turn = saved
        .as_ref()
        .map_or(0, |cursor| cursor.byte_offset)
        .checked_add(1)
        .ok_or_else(|| anyhow!("reconstruction scheduler sequence exhausted"))?;
    // This cursor records an allocation attempt, not consumed source bytes.
    store.advance_cursor(&FileCursor {
        machine_id: machine.into(),
        source_id: SOURCE.into(),
        file_identity: SOURCE.into(),
        byte_offset: turn,
        line_number: turn,
        parser_state_json: Some(serde_json::to_string(&state)?),
        updated_at: Utc::now(),
    })?;
    Ok(selected)
}

fn choose(
    queue: Vec<Work>,
    upgrades: &HashSet<String>,
    budget: usize,
    state: &mut Schedule,
) -> Vec<Work> {
    let budget = budget.max(1).min(queue.len());
    let (mut history, mut tails): (Vec<_>, Vec<_>) =
        queue.into_iter().partition(|(source, _, _)| {
            upgrades.contains(&source.source_id)
                || source.status == ReconstructionStatus::Pending
                || source.bytes_processed < source.bytes_total
        });
    // Keep finishing admitted historical files; do not round-robin all large
    // pending files and multiply unfinished parser buffers.
    history.sort_by(|(left, _, _), (right, _, _)| {
        let active =
            |source: &ReconstructionSourceStatus| source.status != ReconstructionStatus::Pending;
        active(right)
            .cmp(&active(left))
            .then_with(|| right.bytes_processed.cmp(&left.bytes_processed))
            .then_with(|| left.updated_at.cmp(&right.updated_at))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    tails.sort_by(|left, right| left.0.source_id.cmp(&right.0.source_id));
    let history_slots = if history.is_empty() {
        0
    } else if tails.is_empty() {
        budget
    } else if budget == 1 {
        let slots = usize::from(!state.next_single_tail);
        state.next_single_tail = !state.next_single_tail;
        slots
    } else {
        budget - 1
    };
    let history_count = history_slots.min(history.len());
    let tail_count = (budget - history_count).min(tails.len());
    if let Some(last) = &state.last_tail {
        let pivot = tails.partition_point(|entry| entry.0.source_id <= *last);
        if !tails.is_empty() {
            let len = tails.len();
            tails.rotate_left(pivot % len);
        }
    }
    let mut selected = Vec::with_capacity(budget);
    selected.extend(history.drain(..history_count));
    for entry in tails.into_iter().take(tail_count) {
        state.last_tail = Some(entry.0.source_id.clone());
        selected.push(entry);
    }
    selected.extend(history.into_iter().take(budget - selected.len()));
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn busy_sources_do_not_keep_a_new_source_uncollected() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("sessions")).unwrap();
        let index = Connection::open(temp.path().join("state_5.sqlite")).unwrap();
        index
            .execute_batch(
                "CREATE TABLE threads(id TEXT,rollout_path TEXT,cwd TEXT,model TEXT,source TEXT)",
            )
            .unwrap();
        let record = |at: &str, total: u64, last: u64| {
            serde_json::json!({"timestamp":at,"type":"event_msg","payload":{"type":"token_count","info":{
            "total_token_usage":TokenUsage{input_tokens:total,cached_input_tokens:total,total_tokens:total,..TokenUsage::default()},
            "last_token_usage":TokenUsage{input_tokens:last,cached_input_tokens:last,total_tokens:last,..TokenUsage::default()}}}})
        };
        let create = |id: &str, total: u64| {
            let path = temp.path().join(format!("sessions/rollout-{id}.jsonl"));
            fs::write(&path,format!("{}\n{}\n",serde_json::json!({"timestamp":"2026-09-01T00:00:00Z","type":"session_meta","payload":{"id":id}}),record("2026-09-01T00:00:01Z",total,total))).unwrap();
            index
                .execute(
                    "INSERT INTO threads VALUES(?1,?2,NULL,'model','{}')",
                    rusqlite::params![id, path.to_str().unwrap()],
                )
                .unwrap();
            path
        };
        let hot0 = create("hot-0", 100);
        let hot1 = create("hot-1", 100);
        let db = temp.path().join("ledger.sqlite3");
        let mut store = LedgerStore::open(&db).unwrap();
        ingest_reconstruction_batch(&mut store, temp.path(), "machine", 2).unwrap();
        assert_eq!(
            store
                .aggregate_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            200
        );
        create("cold", 1000);
        for path in [&hot0, &hot1] {
            writeln!(
                fs::OpenOptions::new().append(true).open(path).unwrap(),
                "{}",
                record("2026-09-01T00:00:02Z", 110, 10)
            )
            .unwrap();
        }
        ingest_reconstruction_batch(&mut store, temp.path(), "machine", 2).unwrap();
        let cold = store
            .reconstruction_sources()
            .unwrap()
            .into_iter()
            .find(|source| source.thread_id == "cold")
            .unwrap();
        assert_eq!(
            cold.status,
            ReconstructionStatus::Reconstructed,
            "both busy sources must not consume all history slots"
        );
        assert_eq!(
            store
                .aggregate_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            1210
        );
        drop(store);
        let mut store = LedgerStore::open(&db).unwrap();
        ingest_reconstruction_batch(&mut store, temp.path(), "machine", 2).unwrap();
        assert_eq!(
            store
                .aggregate_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            1220
        );
        let idle = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 2).unwrap();
        assert_eq!((idle.bytes_read, idle.files_advanced), (0, 0));
    }

    #[test]
    fn empty_and_partial_eof_do_not_report_endless_actionable_backfill() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("sessions")).unwrap();
        let path = temp.path().join("sessions/rollout-empty.jsonl");
        fs::write(&path, "").unwrap();
        let index = Connection::open(temp.path().join("state_5.sqlite")).unwrap();
        index
            .execute_batch(
                "CREATE TABLE threads(id TEXT,rollout_path TEXT,cwd TEXT,model TEXT,source TEXT)",
            )
            .unwrap();
        index
            .execute(
                "INSERT INTO threads VALUES('empty',?1,NULL,'model','{}')",
                [path.to_str().unwrap()],
            )
            .unwrap();
        let mut store = LedgerStore::open_in_memory().unwrap();
        let empty = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(empty.pending_sources, 0);
        assert_eq!(empty.inserted_events, 0);
        let meta = serde_json::json!({"type":"session_meta","timestamp":"2026-09-01T00:00:00Z","payload":{"id":"empty"}});
        let usage = TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 100,
            total_tokens: 100,
            ..TokenUsage::default()
        };
        let token = format!(
            "{}\n",
            serde_json::json!({"timestamp":"2026-09-01T00:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":usage,"last_token_usage":usage}}})
        );
        let mut append = fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(append, "{meta}").unwrap();
        append.write_all(&token.as_bytes()[..40]).unwrap();
        drop(append);
        let partial = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!((partial.pending_sources, partial.inserted_events), (0, 0));
        assert_eq!(
            store.reconstruction_sources().unwrap()[0].status,
            ReconstructionStatus::Reconstructing,
            "the incomplete evidence state is preserved even while no work can run"
        );
        let idle = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(
            (idle.pending_sources, idle.bytes_read, idle.files_advanced),
            (0, 0, 0)
        );
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&token.as_bytes()[40..])
            .unwrap();
        let completed = ingest_reconstruction_batch(&mut store, temp.path(), "machine", 1).unwrap();
        assert_eq!(
            (completed.pending_sources, completed.inserted_events),
            (0, 1)
        );
        assert_eq!(
            store
                .aggregate_usage(&crate::store::AggregateFilter::default())
                .unwrap()
                .usage
                .total_tokens,
            100
        );
    }

    fn work(id: &str, status: ReconstructionStatus, processed: u64, total: u64) -> Work {
        (
            ReconstructionSourceStatus {
                machine_id: "machine".into(),
                source_id: id.into(),
                thread_id: id.into(),
                file_identity: id.into(),
                status,
                bytes_total: total,
                bytes_processed: processed,
                prefix_events: 0,
                unchanged_events: 0,
                counter_resets: 0,
                last_error: None,
                updated_at: Utc::now(),
            },
            Target {
                thread_id: id.into(),
                parent_thread_id: None,
                path: PathBuf::from(id),
                cwd: None,
                model: None,
            },
            total + 1,
        )
    }

    #[test]
    fn allocation_never_exceeds_available_work_or_allocates_the_requested_limit() {
        let chosen = choose(
            vec![work("only", ReconstructionStatus::Pending, 0, 10)],
            &HashSet::new(),
            usize::MAX,
            &mut Schedule::default(),
        );
        assert_eq!(chosen.len(), 1);
        assert!(
            choose(
                Vec::new(),
                &HashSet::new(),
                usize::MAX,
                &mut Schedule::default()
            )
            .is_empty()
        );
    }

    #[test]
    fn hot_tails_cannot_starve_history_and_each_tail_gets_a_turn() {
        let mut state = Schedule::default();
        let mut seen = HashSet::new();
        for _ in 0..4 {
            let mut queue = (0..4)
                .map(|index| {
                    work(
                        &format!("hot-{index}"),
                        ReconstructionStatus::Reconstructed,
                        100,
                        100,
                    )
                })
                .collect::<Vec<_>>();
            queue.push(work("cold", ReconstructionStatus::Pending, 0, 100));
            let chosen = choose(queue, &HashSet::new(), 2, &mut state);
            assert_eq!(chosen.len(), 2);
            assert!(chosen.iter().any(|entry| entry.0.source_id == "cold"));
            seen.extend(
                chosen
                    .into_iter()
                    .filter(|entry| entry.0.source_id != "cold")
                    .map(|entry| entry.0.source_id),
            );
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn single_slot_alternates_across_serialized_restart() {
        let mut state = Schedule::default();
        let mut ids = Vec::new();
        for _ in 0..4 {
            let chosen = choose(
                vec![
                    work("cold", ReconstructionStatus::Pending, 0, 100),
                    work("hot", ReconstructionStatus::Reconstructed, 100, 100),
                ],
                &HashSet::new(),
                1,
                &mut state,
            );
            ids.push(chosen[0].0.source_id.clone());
            state = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        }
        assert_eq!(ids, vec!["cold", "hot", "cold", "hot"]);
    }

    #[test]
    fn partial_eof_is_a_tail_but_policy_requalification_is_history() {
        let queue = vec![
            work(
                "partial-eof",
                ReconstructionStatus::Reconstructing,
                100,
                100,
            ),
            work("upgrade", ReconstructionStatus::Reconstructed, 100, 100),
            work("new-history", ReconstructionStatus::Pending, 0, 100),
        ];
        let chosen = choose(
            queue,
            &HashSet::from(["upgrade".into()]),
            2,
            &mut Schedule::default(),
        );
        assert_eq!(
            chosen
                .iter()
                .map(|entry| entry.0.source_id.as_str())
                .collect::<Vec<_>>(),
            vec!["upgrade", "partial-eof"]
        );
    }
}
