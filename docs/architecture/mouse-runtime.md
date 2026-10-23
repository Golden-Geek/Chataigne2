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

Input publications use a shared 128-item / 32 MiB payload pool. The original reservation
survives mailbox draining and shared copies until final disposal. Input records weigh 4 KiB;
catalogs weigh 512 KiB and admit at most 64 devices, with 1024 bytes of backing storage per
string field. The actor mailbox admits 32 records / 8 MiB; a service turn drains at most 16.
Providers also return at most 16 records per poll. Full capacity pauses the IO owner with
cancellable waits; the engine never waits for payload or mailbox capacity.

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

## Engine service and remaining C09 work

Mouse uses worker wakes rather than periodic engine updates. Startup and input failures use
an explicit two-second retry deadline. Pointer delta clears through one short deadline after
motion; a healthy idle module has no recurring actor callbacks or snapshots. Receive-disabled
and module-disabled states stop input; the No Input Device selection preserves discovery.

`input.rs` projects the bounded input turn. `devices.rs` owns selection and device options;
`output/preparation.rs` owns command/script parsing. Commands, script methods, callbacks and app-owned
template snippets remain at the Mouse module boundary. Native input and output are disabled
in test builds; qualification uses controlled providers.

Native input and output ownership are implemented. This does not close C09. Remaining work includes
worker-prepared input callback/catalog publication, retained
publication quotas and queued projection guards, and combined pressure/retirement/recovery
qualification. The [ledger](../progress/implementation-status.md) owns current acceptance.
Catalog/readiness projection is still actor-authored: configuration restarts must publish
metadata and readiness from the matching current input generation before C09 closes. Input restart
now drops its previous catalog before asynchronous replacement.
Physical/platform and product-scale checks retain P17/P18 and P16 ownership.
