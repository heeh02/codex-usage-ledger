# Bind official reads to the observed account source

Official activity is an account ledger, not project usage. Fetching the current
app-server account and then attaching a previously cached local fingerprint can
mislabel a response after a login change. An implicit Codex home can also select
a different profile from the collector's explicit source directory. Neither a
successful HTTP response nor matching totals proves the intended account.

The runtime now associates its existing identity observation with a metadata-only
file stamp captured before/after that observation. The stamp contains size,
modification/creation times and platform file identity, never file contents.
Old serialized bindings without a stamp cannot authorize official fetching.
A shared in-memory scope supplies the same observed account/source to automatic,
manual and thread-detail reads; failure/logout withdraws that scope.

Before spawning app-server and after its response, the current source stamp must
still match. A changed scope revision also rejects the result, including A/B/A
changes. The child receives the explicit `CODEX_HOME` and a process-only file-store
selection, matching the source used by the existing file-identity observer. No
configuration file, login, keyring entry or credential is copied or edited by
this adapter. Credential-owning authentication stays with Codex; managed token
rotation may cause rejection until the next ordinary identity observation.

`account/read` with `refreshToken:false` brackets `account/usage/read` in the same
child. Missing/non-ChatGPT or changed account metadata rejects the result, and
thread responses must name the requested thread. Accepted usage carries its
captured account identifier to persistence, not a later global active-account
lookup. Failures retain previous official rows rather than writing zero or
relabeling them. RPC waits use a fixed deadline, not a new timeout per unrelated
notification.

This guards normal source/login changes, not a hostile same-user process that
can forge observations or restore metadata. App-server account metadata is not
a cryptographic account/workspace attestation. Historical official snapshots
remain unmodified and require a separate review; the existence of this risk
does not prove past snapshots were actually misassigned.

The official [app-server documentation](https://learn.chatgpt.com/docs/app-server)
defines the account read/usage methods and non-forced refresh flag. The official
[authentication documentation](https://learn.chatgpt.com/docs/auth) explains
file/keyring storage and `CODEX_HOME`. The installed protocol schema was checked
without starting an authenticated server: account metadata does not provide a
stable workspace ID, so email/plan equality alone is not treated as attestation.
