# Interaction response

Account selection is a compact clickable list with hover/focus and selection
feedback, not a select followed by duplicate account records. Tier badges use
observed plan metadata; generic Pro does not imply either 5x or 20x. Current-login
status is a small annotation, not a second control. Account management remains
on its existing page.

Requested controls update immediately. Data headings, amounts and charts retain
their applied scope until a corresponding result is available. An updating note
separates requested versus displayed data; clicking a second choice is permitted
while the first request is pending.

The application keeps at most six recent scope bundles for 60 seconds in process
memory only. Returning to exactly the same filters can show that snapshot while
refreshing; neither disk nor browser storage receives usage data. Existing source
timestamps remain visible. This improves revisits, not uncached query time.

Background change notifications no longer abort an in-flight scope request every
ten seconds. Explicit user changes still cancel obsolete results through the
existing request lifecycle guard. Query-side optimization remains a separate
concern; do not replace exact accounting with client-side recomputation.

Validation: 101 Web unit tests and 33 browser E2E tests pass, including direct
account choices at four widths, hover/selected state, unchanged login annotation,
scope preservation across project/chat navigation and keyboard chart traversal.
