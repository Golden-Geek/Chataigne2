# Mouse worker ownership

Mouse belongs to the app under `apps/chataigne/src/module/modules/controllers/mouse`.
Golden IO provides admission, lifetime slots, retained payloads and worker notifications;
it contains no Mouse selection or capture policy.

## Native input

`mouse/mouse_runtime.rs` owns the input boundary. The module reserves one lifetime slot
before starting an IO owner; the process permits four such owners. Endpoint discovery,
readiness waits, platform polling and final joins happen on those owners. Cancellation
invalidates retained input immediately, and termination becomes visible before endpoint
cleanup. A blocked startup or cleanup retains both slots and prevents replacement growth.

Prepared publications use a shared 128-item / 32 MiB payload pool. Each envelope charges
64 KiB and each canonical shared catalog metadata allocation independently charges 2 MiB.
Frames retain the original metadata quota after their catalog envelope is disposed. Raw native
records weigh 4 KiB and raw catalogs 512 KiB; catalogs admit at most 64 devices, with 1024 bytes
of backing storage per string field. The actor mailbox admits 32 records / 8 MiB; a service turn
drains at most 16. Providers also return at most 16 records per poll. Full capacity pauses the
IO owner with cancellable waits; the engine never waits for payload or mailbox capacity.

Windows uses one process-owned raw-input registration and up to four bounded subscriptions.
Each subscription admits 64 records / 2 MiB. Late subscribers receive the latest catalog
before subsequent input. Motion deltas and button occurrences retain source order; no
latest-value overwrite silently removes accepted events. A full subscription explicitly
rejects further native input, drains its accepted prefix and reports overload. Its peers
continue independently. The final registration slot survives the endpoint's final join.

Capture settings belong to subscriptions. A native message updates the optional hook when
capture subscribers join or leave. Only a live capture subscriber's selected device can arm
the existing 250 ms activity window; injected events pass through. A retired or overloaded
subscriber cannot keep capture enabled. The actor lifetime guard revokes capture permission
immediately, before the subscriber or endpoint is disposed. Native cleanup removes the hook, unregisters raw
input and destroys the message window on success, failure or unwind. Raw packets use fixed,
aligned storage; native inventory and name sizes are checked before allocation.

On other platforms the input owner directly owns `device_query::DeviceState` and polls at
8 ms on that IO thread. It creates no additional producer thread. OS capture remains Windows-only.

## Output capture and native service

`output/` owns canonical request capture, worker preparation, native effects and receipts.
The old actor-side request parsing, second JSON relay and synchronous native calls are removed.
Command snapshots and original execute payloads remain leased; the worker reads consumed fields
and the last matching override without constructing a snapshot overlay. Script calls capture
engine-owned arrays (up to four arguments / 64 KiB). Button/coordinate option text admits 64 bytes;
move coordinates must be finite, pixel coordinates and scroll amounts must fit signed 32-bit
values, and normalized movement remains absolute-only. All five script methods and three command
node types retain their existing authored controls and app-owned snippets.

Each module admits 64 original sources across local ingress, native queues, receipts and queued
reports. Shared output sources admit 512 items / 32 MiB; native mailboxes admit 64 items / 4 MiB.
Sources weigh 64 KiB plus three times retained override bytes. Original source and command/script
leases survive mailbox draining, shared copies, cancellation and queued reports until disposal.
An actor turn submits at most eight sources and services at most sixteen receipts. Full admission
rejects explicitly; mailbox retry retains the same source. Four process workers and one per-module
lifetime include blocked open, execution and cleanup. Recovery backs off from 250 ms to five seconds
without replaying native effects.

Keyboard and Mouse share the app-owned `common/desktop_output.rs` coordinator, with 512 weak
ordering claims. Pending claims sort by original engine time and capture sequence; local ingress
settles before native submission. One permit covers a complete native request, including both
halves of a click. Pointer/display queries for relative/normalized moves also run inside that permit
on the IO worker. Already-started effects are never reordered. Source retirement unblocks waiting
peers before cleanup; waiting cancellation is not sent and a started panic is unconfirmed.
Finished receipts retain source quotas while releasing ordering. No payload buffers live in the coordinator.

Completed reports precede independently fenced readiness. Queued structural/settings guards reject
stale targets; readiness additionally fences the current output runtime. Successful outgoing logs
follow the existing module flag. Disabled worker wakes drain accepted receipts without admitting
new work. Input service never writes output readiness or shared connection state. Native input and
output, including direct output-provider calls, are disabled in test builds.

## Prepared input and engine service

The input worker owns selection, pointer/button state, callback JSON, messages and native device
choices. Motion prepares two callback candidates, and button edges prepare four: changed and
pressed/released callbacks for both output-readiness states. Admission is atomic under a 1 MiB
batch cap in the shared native-callback pool (16384 payloads / 256 MiB). The actor selects existing
shared payloads without constructing JSON. Events, replay and exported payload Arcs retain the
original callback quota through final disposal.

Native choices admit 67 rows / 2 MiB per domain in a separate 128-domain / 64 MiB pool. Identical
domains reuse canonical backing storage; live parameters, snapshots and history retain that
original quota. The public enum-options edit atomically applies a canonical domain and an optional
normalized selection behind UUID/current-value guards. Disjoint old and new choice lists remain
valid without replacing the parameter. Disabled input uses the bounded fixed/missing-choice fallback.

Selection changes advance a configuration revision before replacing desired settings, reset
worker state and fence old frames. Non-capturing selection changes reuse the native endpoint;
capture-mode and captured-device changes retire it immediately and open a bounded replacement.
Parameter notifications and worker service reconcile the same applied configuration; a delayed
notification preserves an owner that already matches it.
It also preserves values already received from that owner. Settings notifications capture their
authored guard prelude before queuing their own device-warning edits.
No Input Device preserves discovery while filtering input. Dropping the actor handle clears its
retained catalog. Readiness and device metadata require a matching current publication.

`input.rs` projects typed values and shared callbacks behind actor/native/configuration fences,
authored settings and target/ancestor identity guards. Queued edits retain the original source
through application, rejection or disposal. Input frames do not write output readiness. Final
connection status fences both current services independently of completed input/output prefixes.

Mouse uses worker wakes rather than periodic engine updates. Startup and failure recovery use
absolute deadlines with backoff from 250 ms to five seconds. The worker emits one value-only
delta reset after motion; the actor has no delta timer. Healthy idle service has no recurring
callbacks, snapshots or wakes. The 8 ms provider poll and 10 ms pressure waits run on IO owners.
Receive-disabled and module-disabled states stop input. Callback/domain pressure preserves the
same staged occurrence and pauses provider polling; retirement cancels without capacity becoming free.

`devices.rs` owns selection policy; `output/preparation.rs` owns command/script parsing. Commands,
script methods, callbacks and app-owned snippets remain at the Mouse boundary. Native input and
output are disabled in test builds; local qualification uses controlled providers. The
[ledger](../progress/implementation-status.md) owns current acceptance. Physical/platform and
product-scale checks retain P17/P18 and P16 ownership.
