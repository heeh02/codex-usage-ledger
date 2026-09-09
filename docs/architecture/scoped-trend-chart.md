# Scoped trend chart contract

Question: how did the selected local scope's recorded Token usage vary across the
requested calendar interval? The chart describes retained evidence, not complete
server inference or productivity.

Surface: reuse the existing application's `UsageTrendChart`, its responsive SVG,
keyboard readout and exact-value table. No extra renderer, dependency or report
runtime. Inputs are the same validated scope response as the model/account tables.

- Eight or more observed buckets: existing unfilled line, split across missing
  calendar buckets. Fewer buckets: discrete stem-and-dot marks, not an interpolated
  trend. Users can choose finer date grouping in the existing form.
- Keep the requested civil-calendar domain and source timezone. Normalize month
  keys to month starts for plotting; no invented daily values or zero-filled gaps.
- Total is the sum of observed buckets. An all-unknown metric stays unavailable;
  measured zero remains zero. Reasoning remains inside output.
- Preserve the established local-evidence series color and CSS tokens, neutral
  axes/grid and explicit labels; no additional categorical palette or gradient.
- Show applied scope/date context above the chart, M units, peak and exact-value
  readout. No official/comparison series is synthesized for a local-only query.
- At 560/1280 CSS pixels, retain scroll access and check labels/geometry. This
  does not substitute for installed native acceptance or complete data coverage.

The source response still retains all Token components, record counts and the
account/project/model/thread dimensions for exact lookup and subsequent analysis.
