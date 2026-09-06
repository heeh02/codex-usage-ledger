# Optional presentation preferences

Session storage is a convenience, not a dependency of the dashboard. A denied
Storage getter, failed read, malformed JSON or quota-exceeded write must not
throw out of the React render/effect path. Failed persistence leaves the current
in-memory view usable; reload may return to defaults. Language persistence has
its own guarded local-storage path.

Only known preference fields are restored. Types, period/metric/grain/tab/scope
enums, pagination bounds and search lengths are validated. Identifier fields
are nonempty strings, retained without truncation up to 4096 characters. Search
strings retain the API's 256-character bound. Objects, arrays, unknown fields
and prototype-like keys are not merged into application state. Invalid custom
date ranges fall back to the default period before any usage request; impossible
dates do not silently normalize to a different day.

These are view preferences, not a persisted copy of usage responses. Applied
query filters still advance together with the accepted server response.
Storage validation does not authenticate account/project IDs or change ledger
attribution. The backend remains authoritative for source facts and query
validation. No preference failure is converted into zero usage.

Unit tests cover storage getter/read/write failures, malformed shapes, enum/type
rejection, valid optional fields, all supported presets and custom dates.
Browser task regressions additionally exercise overview → project → conversation
under denied storage, write exhaustion and corrupt settings; registration or
unit success is not evidence that these browser journeys were executed.
