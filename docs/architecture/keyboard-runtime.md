# Keyboard worker ownership

The app-owned Keyboard module projects authored values, device choices and script callbacks.
Its input and output owners use public Golden IO workers, revision fences and payload pools. Golden packages
contain no Keyboard types or policy. Platform files own raw-input discovery, native message
processing and the existing key mappings.

## Input lifetimes

Four global input-owner slots include startup, running and cleanup. Each module also reserves
one owner slot, held until the provider and its native child thread finish cleanup. The actor
never waits for native readiness or joins a native thread. A bounded IO owner performs those
operations; dropping the actor handle invalidates its publications and cancels forwarding.
A blocked open or join keeps both slots, so retries cannot grow an unbounded set of workers.

The platform thread remains separate because Windows requires a thread-affine message pump,
while the other platform adapter owns its device-query loop. Native shutdown, startup failure
joins and window destruction run behind the IO owner. Windows window ownership uses RAII.
Foreground raw-input packets run the default procedure for system cleanup, as required by
[the Windows contract](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-input).
Native input creation and output injection are disabled in test builds; controlled providers
exercise the handoff and lifecycle.

### Process-owned Windows registration

Windows input owners subscribe to one app-owned process registry and raw-input message pump.
The registry runs only on IO owner threads. It serializes opening, shares the running endpoint,
and holds one native lifetime permit until the last subscriber's endpoint cleanup and join finish.
A replacement open is refused during retirement and uses the existing module recovery path.
Dropping one subscriber leaves the other subscribers and their native registration intact.
The last subscriber requests shutdown; the window owner removes registration before destroying
the window. No control-engine callback acquires the registry lock or joins the message pump.

Fan-out admits four subscriber lifetimes, each with its own 64-record / 2 MiB native queue.
Failed subscribers occupy their slot until disposal, including their accepted queued prefix.
One slow subscriber receives an explicit overload failure while accepting peers continue in order.
All subscribers keep the original device identities, so module selection remains independent.
A bounded latest catalog seeds a new subscription under the fan-out lock before later edges.
Native failure or panic stops publication and exposes terminal state to every subscriber before window cleanup.
Controlled endpoint tests cover sharing, pressure, ordering and retirement; they do not qualify
the physical Windows message pump.

## Bounded handoffs

Each native channel admits 64 records / 2 MiB. Input records charge 4 KiB; native catalogs charge
512 KiB. A full channel rejects the occurrence explicitly, records overload and ends the native
producer's subscription on Windows, or the producer on other platforms. It does not block the
message pump or silently discard a successful admission.
The owner drains at most 16 records per poll. Terminal state is observable independently of
sender destruction, including when native teardown still owns the sender.

The owner publishes through a 32-record / 8 MiB mailbox. A global source pool admits 128
lifetimes / 32 MiB. Every prepared envelope charges 64 KiB; each canonical shared catalog
metadata allocation independently charges 2 MiB in the same pool. A frame that outlives its
catalog envelope retains the original metadata reservation through its shared catalog Arc.
Input forwarding pauses on mailbox, source, choice-domain or callback lifetime pressure and
resumes with the same prepared occurrence. Cancellation does not require a free mailbox slot.
At most one 16-record provider turn and one prepared candidate batch remain staged.

Catalogs admit 64 keyboards and 1024 bytes per identity, label or detail backing allocation;
native-to-owner validation also bounds spare vector capacity. Windows
raw-device enumeration admits at most 1024 entries before allocating its list, and device names
admit 1024 UTF-16 units before allocation. Keyboard packets use fixed aligned storage and reject
invalid sizes. These bounds do not qualify device-query library internals or physical hardware.

## Prepared input and passive service

The input worker owns device matching, held-key state, modifier derivation, callback JSON,
message formatting and native choice preparation. It publishes immutable typed key frames and
catalogs. Each key occurrence has changed/edge callbacks for both output-status candidates;
the actor chooses the observed output status and binds existing shared payloads without JSON
construction. Four candidates admit atomically under a 1 MiB batch cap, using the app's shared
native-callback pool (16384 payloads / 256 MiB). Ordinary event, replay and exported Arc copies
retain the same canonical quota after the input envelope is disposed.

Native device choices admit 67 rows / 2 MiB per domain in a separate 128-domain / 64 MiB pool.
Identical domains reuse immutable backing storage, including selection changes that leave the
choices unchanged. Live parameters, snapshots and history retain original quota through their
shared Golden domain allocation. Missing selection and the narrow persisted label-suffix
normalization remain backend responsibilities. Publication atomically applies the canonical domain
and normalized selection through the public enum-options edit, preserving parameter identity and
history even when the old and new domains do not overlap. Disabled input uses a bounded authored fallback
of two fixed choices plus at most one missing-selection row; it does not rebuild native domains.

Selection configuration is at most 1024 bytes. Updating it advances a revision before replacing
the worker's desired settings. Old staged or queued publications are cancelled without reopening
the native endpoint; the worker resets held state and prepares a catalog for the latest selection.
Losing the selected device also clears held state. Choosing No Input Device keeps discovery and
device choices available while filtering key occurrences on the worker.

The actor is passive: input notifications drain at most 16 publications per turn. Startup and
worker failure use absolute recovery deadlines with backoff from 250 ms to five seconds.
Blocked cleanup retains native and per-module lifetime slots; admission refusal backs off rather
than growing native threads. Healthy idle input produces no actor updates, snapshots or wakes.
The remaining 8 ms native-owner poll and 10 ms pressure wait run on IO workers.

## Output capture and service

Output has a separate app-owned worker. Four global lifetimes and one per-module lease include
opening, execution and cleanup. The existing system-wide key injection policy remains independent
of the selected input keyboard. Native calls and request preparation run on that worker.

### Shared output ordering

Keyboard and Mouse output workers share one app-owned process coordinator in
`src/module/common/desktop_output.rs` because both families' effects
target the same system input stream. Capture reserves one of 512 ordering claims. Pending claims
sort by original engine time and then capture sequence, including sources still in local staging;
an already started native prefix is never reordered. Module ingress settlement and local sorting
keep each native mailbox in that order, so a later local request cannot block its own predecessor.
The coordinator stores only weak claims and small lifetime fences, never invocation snapshots or
payload buffers. Original source item/byte quotas remain authoritative through receipt disposal.

One permit covers a complete native request, including both press and release of an app-owned tap.
Registration is bounded work in the actor; all waits and native calls run on IO workers. A waiting
request checks cancellation and source/producer liveness before being marked started. Retired
unsubmitted heads are pruned without waiting for their retained data or native cleanup. Waiting
cancellation is not sent; started panic remains unconfirmed. Completion releases ordering before
actor report disposal. A permit stays held through a blocked native call, and unwinding releases
it before provider cleanup, allowing peers to progress while the old worker still holds its IO leases.
This orders app-owned keyboard and mouse requests; it does not control physical input or other processes.

Command nodes capture the canonical snapshot and original execute payload; script methods retain
engine-owned argument arrays. The worker reads consumed fields, applies the last matching override
and constructs one typed key/action request. It does not build an execute-overlay snapshot or relay
another request JSON object through the engine. Key/action text admits 64 UTF-8 bytes; scripts admit
one argument / 64 KiB. Methods acknowledge admission; validation/native outcomes arrive separately.

Each module admits 64 source lifetimes across local inboxes, staging, native execution, receipts
and queued application or disposal. Global source storage admits 512 / 32 MiB. Sources charge
64 KiB plus three times retained execute-override storage. The native mailbox admits 64 / 4 MiB.
Original engine timestamps order staging, and ingress permits hold submission until deferred
handoffs have arrived or been disposed. Full mailbox admission retains the same source behind an
absolute 250 ms retry deadline; refusal does not publish a spurious cancellation.

Producer identity and actor/native lifetime guards fence preparation and execution. Stopping
output cancels unstarted work as **not sent**; an interrupted started call is **unconfirmed**.
Completed effects retain their result independently of retirement. There is no automatic replay.
Termination and queued disposition precede provider cleanup, and cleanup retains both leases.

Output service uses worker notifications and explicit deadlines, with at most eight submissions
and sixteen completions per actor turn. Startup/stopped-worker recovery backs off from 250 ms to
five seconds. Previously accepted results can drain while the module is disabled. Queued result
projection checks current target identity and named ancestors independently of retired native
sources, retaining the original source reservation through application, rejection or disposal.
Ready-status projection has a separate native fence after completion messages, so retirement
rejects stale readiness while preserving an already completed effect's message prefix.
Combined worker turns project completed output reports, then input values/callbacks, then derived
connection status. The final status group fences every native source contributing an active flag;
output readiness is sampled once for that projection. Input projection never authors output-ready
state. Rejecting a readiness suffix therefore preserves input effects, and rejecting input cannot
erase an earlier output report. Golden edit fences stop scalar/removal coalescing across these
boundaries; see [application contracts](application-contracts.md).

## Actor input projection

The original publication reservation follows queued application, rejection or disposal; the
current catalog also retains its envelope. Before any writes in a bounded input turn, projection
checks authored device/receive/send settings and current target identities and named ancestors.
This prelude precedes the turn's own enum-constraint edits, so a new catalog cannot invalidate
later frames in that turn. Every publication also has a separate actor/native/configuration fence
at its projection position. A current snapshot with a changed selection or disabled receive
rejects an old frame even before its parameter notification has dispatched. Renaming a queued
target or retiring a source rejects stale values and callbacks.
Settings notifications capture their authored guard prelude before changing device warnings and
preserve values from an already reconciled input configuration.
Device choices use the public enum-options edit, preserving parameter identity and read guards
instead of replacing the parameter. The existing narrow normalization of persisted label-suffixed
device IDs remains in this backend boundary.

Input/output lifecycle ownership, canonical output capture, prepared input callbacks and choice
lifetimes, and passive input/output service are implemented within C09. Both services use
notifications and explicit deadlines; Keyboard no longer has a periodic actor update.
Windows uses process-owned raw-input registration and bounded fan-out because
[only one window per device class can receive raw input in a process](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices).
Shared input registration and output coordination use controlled providers for local qualification.
Mouse uses the same publication and ordering contracts; see [Mouse ownership](mouse-runtime.md).
The [execution ledger](../progress/implementation-status.md) owns package counts, current local
qualification and the remaining platform/product-scale gates.
