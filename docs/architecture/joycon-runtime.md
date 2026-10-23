# Joy-Con worker ownership

The app-owned Joy-Con module projects authored values and callbacks. Its native runtime owns
`joycon-rs`, discovery, calibration, report reads, rumble/lights, device recovery and cleanup.
The module uses public `golden_io` admission, payload, revision and worker primitives.

## Runtime and service

Four global lifetime slots include opening, active and retiring workers. A module also reserves
one connection lease through native destruction, so old cleanup cannot stop a replacement's
rumble. Cancellation never needs command-mailbox admission, a join or a retirement thread.
Termination and queued command disposition precede potentially blocked provider cleanup.
Native endpoint creation is disabled in test builds; qualification uses controlled providers.

The actor registers its worker wake when the node is ready and stays passive. Worker receipts,
input and termination notify it; processing caps,
activity, a connected-runtime watchdog and startup recovery use explicit deadlines.
Startup retries back off from 250 ms to five seconds, independently of unrelated wakes.
Discovery/attach retry, stale reports, stick calibration and IMU filtering remain native-owned.

## Admission and delivery

The command mailbox admits 64 records / 4 MiB. Captured sources charge 64 KiB plus three times
execute-override storage for bounded worker decoding. A module admits 64 lifetimes across local
inboxes, outgoing/native queues, completed receipts, queued projection and disposal. Global output
storage admits 512 lifetimes / 32 MiB. Text fields admit 256 UTF-8 bytes; numeric text admits
32 bytes, diagnostics 1024 bytes, and rumble numbers must be finite.
Native turns service eight commands before checking input. Completed outcomes retain their
original admission independently of input pressure, retirement and cleanup. Unstarted
cancellation is **not sent**; an interrupted started effect is **unconfirmed**. No command replays.

Command nodes retain canonical snapshots and execute payloads. The worker reads only consumed
fields and applies the last matching execute override, without constructing a snapshot overlay.
Both script methods retain engine-owned argument arrays, bounded to five arguments / 64 KiB.
Their defaults are static app policy; they do not need an otherwise unused module snapshot.
Target/LED validation, defaults, numeric parsing and native request construction run on the worker.
Calls acknowledge admission; worker validation and native results arrive through retained receipts
and module diagnostics.

Typed local inboxes retain original engine timestamps. A separate ingress permit tracks every
admitted source until arrival or disposal; native submission waits for those handoffs to settle,
then services the sorted prefix. This also preserves order when a caller dispatches an inbox
before the next ordinary tick. Ingress release wakes service rather than requiring polling.
Full native admission preserves the same source behind an absolute 250 ms retry deadline.
Refused unpublished receipts do not emit cancellation wakes.

Producer identity and actor/native/session guards fence unstarted work before preparation and
native dispatch. Any controller-slot connection change retires the captured pair's session;
reconnect cannot make an earlier request live again. Removing/disabling a command or retiring
its module also cancels its unstarted work. Completed effects retain their result independently.

The input mailbox admits 32 records / 256 KiB. Each publication charges 8 KiB; global input
storage admits 128 lifetimes / 1 MiB, retained through queued edit disposal. A native poll
returns at most eight bounded records, including at most one state frame, with at most sixteen
buttons in each of the six button lists, finite numeric samples and 1024-byte diagnostics.
Pending input prevents further native reads, while command
handling and rumble-stop maintenance continue. This prevents a stalled consumer from extending
a rumble simply because input publication is full. Pressure transitions notify a warning.
Failed rumble-stop writes keep a 250 ms retry deadline for that controller; other controllers
continue servicing their own deadlines. A new rumble replaces its previous stop deadline.

Actor turns project two input records and sixteen command outcomes. Button and connection
changes are applied in arrival order; analog-only pending state follows the processing cap.
Retired worker input carries invalid revision guards. Input projection retains its original
payload through queued application/rejection; completed effects use the current module identity.
Startup failure diagnostics retain the actor guard after native termination, so the cause can
still be published while a live runtime handle observes the failed start.

## Callback preparation and projection

The native worker prepares immutable callback candidates for the fixed two-controller vocabulary:
connection, eleven changed/edge button pairs, stick and optional motion per side. A draft admits
at most 50 candidates / 8 MiB. Unchanged per-side candidates are cached; changed rows release their
old cache before replacement admission. JSON construction, transformation and retained-byte
measurement happen on the worker. The actor compares the previous applied raw state and binds
only matching candidates. Preparing candidates rather than worker-selected transitions preserves
analog coalescing and cross-controller changes under the authored processing cap.

Candidates use the app's canonical native callback pool (16384 items / 256 MiB). Golden IO's
domain-neutral atomic Arc-batch admission registers all fresh allocations or none, so pressure
cannot strand a partially admitted frame. Ordinary event Arc copies keep this quota after the
input envelope is disposed. Callback pressure pauses input preparation while command service,
rumble-stop maintenance and cancellation remain available. The original input reservation also
follows analog state held for its processing deadline, through queued application or rejection.

Dead-zone and motion settings have a separate publication revision. A change cancels old drafts
and re-prepares the latest current-session raw frame; it does not open another native lifetime or
replay commands. Actor service additionally fences authored settings before any queued writes.
Every projection guards its fixed bound targets and their named ancestor paths, without scanning
the project. Node identity/structure guards are split into the public 32-read chunks, all checked
before values or callbacks. Native, session and configuration guards reject retired publications;
scalar value and warning changes do not masquerade as structural changes.

## Qualification boundary

The [execution ledger](../progress/implementation-status.md) records C08 local closure after 71
owning passes, including callback-capacity recovery, queued target/path/settings rejection,
continuous input, eight reconfigurations and recovery before blocked cleanup. Tests use controlled
providers and never open native endpoints. These runtime bounds do not qualify the SDK's transient discovery
allocation, physical devices, cross-platform behavior or full-product scale.
