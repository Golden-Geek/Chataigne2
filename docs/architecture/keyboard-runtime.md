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

The owner publishes through a 32-record / 8 MiB mailbox. A global payload pool admits 128
lifetimes / 32 MiB, charging 4 KiB per input and 2 MiB per catalog publication. Input forwarding
pauses on mailbox or lifetime pressure and resumes with the same prepared record. Cancellation
does not require a free mailbox slot. At most one bounded provider turn remains staged.

Catalogs admit 64 keyboards and 1024 UTF-8 bytes per identity, label or detail field. Windows
raw-device enumeration admits at most 1024 entries before allocating its list, and device names
admit 1024 UTF-16 units before allocation. Keyboard packets use fixed aligned storage and reject
invalid sizes. These bounds do not qualify device-query library internals or physical hardware.

## Output capture and service

Output has a separate app-owned worker. Four global lifetimes and one per-module lease include
opening, execution and cleanup. The existing system-wide key injection policy remains independent
of the selected input keyboard. Native calls and request preparation run on that worker.

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

## Actor input projection and remaining C09 work

Actor turns drain at most 16 publications. The original publication reservation follows queued
application, rejection or disposal; the current catalog also retains its original publication
while the module keeps its catalog copy. Before catalog/value/callback work, input projection
fences actor/native lifetimes, bound targets and their named ancestors, and authored device,
receive and send settings. Renaming a queued target, disabling receive or retiring the source
rejects the stale projection before its values and callbacks apply.
Device choices use the public enum-options edit, preserving parameter identity and read guards
instead of replacing the parameter. The existing narrow normalization of persisted label-suffixed
device IDs remains in this backend boundary.

Input and output lifecycle ownership and canonical output capture are implemented within C09.
Native input callback preparation, canonical published choice lifetimes, passive input service and
full package recovery qualification remain open. Input still uses its existing periodic service
and retry clock; output uses notifications and explicit deadlines.
Windows uses process-owned raw-input registration and bounded fan-out because
[only one window per device class can receive raw input in a process](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices).
Shared input registration is locally qualified with controlled providers; shared output endpoint
coordination and full package recovery qualification remain open.
Mouse has not yet migrated. The [execution ledger](../progress/implementation-status.md) owns
package counts and qualification; this page does not claim C09 closure.
