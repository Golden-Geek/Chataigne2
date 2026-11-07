# Outstanding operation feedback

Golden owns user-operation lifecycle tracking in `store/session/action-activity.svelte.ts`.
The intent pipeline reports real queue, receipt, application and rejection transitions;
the coordinator also waits for the committed event boundary and staged UI update. Existing
background inspections and silent streaming/framing intents keep their current classification.
MapGyver consumes the same public session contract as other Golden applications.

`PendingFeedback` in `store/session/pending-feedback.ts` owns presentation deadlines for
`EngineStatus`. It compares the oldest outstanding operation's monotonic timestamp with
the current clock. It does not measure how long a gesture or a continuously occupied queue
has existed. A succession of fast acknowledged operations therefore stays quiet.

Progress appears after 300 ms of genuinely outstanding tracked work. Once shown, it stays
legible for at least 350 ms. Completion holds a truthful “Changes applied / Done” caption
through a 150 ms coalescing grace; it does not leave a spinner claiming unfinished work.
An overlapping successor has one eligibility interval to become slow without an intervening
hide, while a fresh stream of fast work cannot perpetually extend the previous notice.
Rejections take priority over older pending work and remain visible for the coordinator's
1,200 ms failure lifetime. When that lifetime ends, any remaining slow work resumes its
own label and phase; the oldest operation timestamp still determines progress eligibility.

One-shot deadlines replace polling. Session replacement, disconnection/loading and component
disposal cancel presentation timers. Losing the transport connection also resets the
coordinator's activity lifetime, cancelling event-boundary, failure-retention and paint
waits. Late completion/rejection from that lifetime cannot recreate feedback after reconnect.
Errors still propagate to their callers and the existing workbench log/toast path.
These rules affect notification presentation only; they never delay intent dispatch,
alter edit-session ownership, suppress errors or change
history, acknowledgement fencing, renderer cadence or authored settings.

Controlled-clock tests cover continuous fast work, delayed/overlapping work, minimum visible
lifetime, completion, rejection, cancellation and owner/connection teardown. The native
MapGyver fixture routes the owned WebView's WebSocket to its real server, holding selected
requests or closing a connection to verify actual feedback and recovery. Those injected
delays establish behavior, not physical input latency or high-refresh presentation quality.
