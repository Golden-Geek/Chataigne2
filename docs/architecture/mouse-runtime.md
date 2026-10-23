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

## Engine service and remaining C09 work

Mouse uses worker wakes rather than periodic engine updates. Startup and input failures use
an explicit two-second retry deadline. Pointer delta clears through one short deadline after
motion; a healthy idle module has no recurring actor callbacks or snapshots. Receive-disabled
and module-disabled states stop input; the No Input Device selection preserves discovery.

`input.rs` projects the bounded input turn. `devices.rs` owns selection and device options;
`script.rs` preserves script request parsing. Commands, script methods, callbacks and app-owned
template snippets remain at the Mouse module boundary. Native input and output are disabled
in test builds; qualification uses controlled providers.

This is the native input ownership milestone, not C09 closure. Output still uses the old
synchronous `output_native.rs` path. Remaining work includes canonical command/script capture,
bounded ordered output and receipts, worker-prepared callback/catalog publication, retained
publication quotas and queued projection guards, and combined pressure/retirement/recovery
qualification. The [ledger](../progress/implementation-status.md) owns current acceptance.
Catalog/readiness projection is still actor-authored: configuration restarts must publish
metadata and readiness from the matching current input generation before C09 closes.
Physical/platform and product-scale checks retain P17/P18 and P16 ownership.
