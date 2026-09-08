# Automatic usage reconciliation

The accounting rules are shared code, not individual session judgments:

1. Identify the canonical stream and inherited/foreign history. Replay advances
   a baseline but does not emit new usage.
2. Emit valid new request usage. Unchanged counters do not create another event;
   counter resets start a new epoch instead of producing a negative delta.
3. Match sources by preserved record identity, never merely equal Token amounts.
4. Assign account from time-scoped evidence and project from the indexed thread
   hierarchy. Unknown identity stays unknown; the current login does not relabel
   history. Descendant rollups must not add the parent aggregate twice.
5. Preserve input, cache read/write and output components; reasoning is inside
   output, not another addition. Official totals stay separate from local facts.
6. Persist cursors and parser state. Normal collection processes appended bytes;
   historical migration is a one-time pass using the same parser rules.

`draft-reconstruction-batch --automatic` executes the existing deterministic
parser, historical correction and sampling-association steps without manual
per-session approval or copying seals. It currently targets an isolated shadow;
production promotion remains a separate operation. It never imports newly found
candidates as part of correcting existing records.

Use `--automatic-batches 1000 --limit 10` for a bounded continuous job. It advances
the saved cursor automatically, stops when the inventory ends, and returns only
aggregate source/error counts. The default is one batch; the job cap does not
change the accounting rules or authorize a production switch.

The output directory stores a private `automatic-history-progress.json`, bound
to its ledger and source home. Runs resume the saved cursor when `--after` is
omitted. A process lock prevents overlapping batches in the same directory. Progress
is atomically replaced after the batch; crashes can safely replay receipts.
Failed/ambiguous sources are isolated and retained in the checkpoint, not guessed
or silently converted to zero. Machine receipts remain diagnostic artifacts, not
an invitation to audit each conversation manually. `hasMore=false` ends the
inventory pass, not a claim that isolated errors or production promotion are done.

Run counters describe this invocation. `outstandingIsolatedSources` includes
unresolved sources saved by previous runs, including older checkpoints without
error text. Private checkpoints retain each newly observed isolation reason;
successful retries remove its saved isolation entry. An empty new batch must
not reset the historical outstanding count or imply complete evidence coverage.
