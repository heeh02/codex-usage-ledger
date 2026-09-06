# Native stop deadlines

Mode transitions and startup health failure request SIGTERM, then schedule a
single 800ms grace deadline on the native main actor. If the original Process
is still live, still owned by the controller and in the same generation, the
deadline escalates to SIGKILL. Cancellation, loss of ownership, normal exit or
application termination prevents escalation. The existing termination callback
still decides whether to launch the requested mode; the timer never launches
a second service itself. Signal failure cancels the pending mode and shows a
localized error rather than pretending the child stopped.

Repeated mode selection during an in-progress stop updates only the pending
destination; it does not extend the stop deadline or ignore a request to return
to the old mode. App exit retains its separate bounded synchronous shutdown
because the application may not service async tasks after termination begins.

`bash macos/test-process-stop.sh` compiles an isolated executable that launches
only its own test children. Cases cover normal SIGTERM exit, a child explicitly
ignoring SIGTERM, deadline cancellation and ownership loss. Each test cleans up
its own child and temporary executable. `bash macos/test-pure.sh` covers pending
mode retargeting. This is real child-process/helper evidence, not full installed
application mode-switch or UI acceptance. A process stuck uninterruptibly in the
kernel may still delay actual exit after SIGKILL; the code does not promise an
OS-independent termination-time guarantee.
