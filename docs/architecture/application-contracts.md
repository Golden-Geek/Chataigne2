# Application Contracts

`golden_application` is the app-agnostic contract layer. It owns public project-transaction,
graph-editing, runtime-value, observation, module-I/O, persistence, and host-lifecycle traits.
`ApplicationFacades` composes those concerns independently so applications and hosts do not depend
on engine internals.

`golden_engine::app::ProjectLifecycle` supplies pure product metadata: application ID, display
name, release version, app-data directory and project-file specification. Chataigne and MapGyver
declare their own identities. Listener and frontend-process policy belongs to the shared
`golden_host_desktop::DesktopHostConfig`, whose `ui_server` and optional `dev_server` feed
`run_default_with_config` and the configured launch APIs. Both apps use that runtime; the root
workspace default remains Chataigne. MapGyver selects backend port 7011 and development UI port
5174 without changing process environment variables.

`golden_protocol` owns `UiApplicationInfoDto`, `UiDiscoveryDto` and UI protocol version `0.13.0`;
`golden_codegen_support` generates their TypeScript declarations. The transport exposes
`/.well-known/golden-ui` with service `golden-ui`, application metadata and the exact protocol
version. `ApplicationRoot` validates that discovery against the expected product before mounting
the workbench. Golden UI scopes browser storage and restoration to the application ID. Project
file actions wait for authoritative snapshot `project_file` metadata, required by the wire
contract, rather than inventing a default format.

`golden_persistence::ApplicationProjectDocument` wraps full application projects with required
`application_id` and a flattened generic `ProjectDocument`. Full app save/load APIs validate
that identity; runtime save captures retain it through off-thread encoding. Missing or foreign
IDs are rejected before node codec expansion, lifecycle callbacks or live-project replacement,
including recovery requests. Generic graph documents, subtrees and Preferences retain their
existing codecs and graph format version. There is no unmarked-file migration or compatibility
fallback. Four repository-owned Chataigne fixtures were stamped with their known identity;
user project files were not rewritten.

`golden_host_desktop::NativePresentation<R>` is the optional Windows underlay extension. The
presentation-aware launch APIs retain the ordinary `DesktopHostConfig` and default launch APIs.
Golden constructs the transparent editor window, applies native clipping and calls preparation
on its UI thread. Preparation returns an unstarted, `Send` `NativePresentationTask`; the host
starts its worker and delivers idempotent cancellation when an explicit exit is requested.
WebView COM access remains scoped to Tauri's UI-thread callbacks. Native renderer resources are
created and retired inside the worker, and task completion certifies that its window use ended.

The host defers exit while that worker retires and joins only after `run_return`. A task ending
on its own leaves the editor and unsaved work intact. The workbench still owns save/discard/cancel;
its confirmed `window_destroy` command routes through retirement. App capabilities omit direct
Tauri destroy permission. Headless/default launches create no presentation lifecycle. The current
extension covers the main editor underlay; renderer, preview geometry and output-window policy
retain their M03/M05 owners. See the [MapGyver evidence](../progress/mapgyver-m02-shell.md).

`golden_time` owns typed time coordinates, checked rational conversion, immutable show mappings
and authority/discontinuity epochs. Apps own transport policy; specialized schedulers own
preparation, lateness and once-only effects. `golden_video::runtime` uses that authority with
Golden IO workers for bounded preparation, frame-boundary installation and completion-based
resource retirement. Its control/events contain CPU inputs or metadata, never backend objects.
The null backend verifies ownership and scheduling. The optional Windows D3D11 backend now
executes composition and fixed window/Spout outputs through the same runtime, with an owned
shared-host smoke consumer. Persistent pools reserve a baseline before allocation; completion
records survive submission unwind. External sources, dynamic preview bindings, native recovery
and full GPU qualification remain open. See [Golden Video](../../crates/golden_video/README.md) and
[Golden Time](../../crates/golden_time/README.md) for the public boundaries.

For queued native-derived edits, `golden_engine::edit::EditQueue::retain_payload` attaches an
already-admitted Golden IO payload lease to the requests currently in the queue. Admission survives
producer removal, ingress transfer, deferred/atomic application, coalescing and discarded suffixes.
Final payload disposal stays on its pool worker. Ordinary `EditCompletion` observers retain no
payloads, and later queue additions are excluded. Retention establishes lifetime only; callers still
own validation, source fences, publication policy and their explicit applied-prefix failure behavior.

`Edit::CheckPreconditions` is also a coalescing boundary. Scalar writes and duplicate removals
merge only within the requests after the most recent fence. A later guarded update therefore
cannot erase an earlier applied prefix or borrow its successful completion when that fence fails.

`NodeReadGuard::capture` validates one node; `capture_subtree` additionally fences descendant
authored changes before their callbacks run. A subtree check shares the ordinary snapshot cache
and may rebuild a cold view. It does not promise constant-time snapshot construction.

Move events notify the child's current ancestry and the former parent's membership ancestry.
Shared ancestors and subscriptions receive the event once. This lets an owner react when a child
moves away, including when a later queued removal makes the child unavailable before dispatch.

`Edit::SetParamEnumOptions` publishes shared choice storage behind target UUID and current-value
guards; stale results are successful no-ops. Its optional `selected_value` validates and applies a
new selection atomically with the domain, including disjoint old/new choice lists. Failed validation
preserves both fields. Normal value/constraint events and one reversible history effect retain the
parameter's identity, metadata and canonical domain ownership. App-specific normalization policy
stays with the producer; the engine does not interpret device IDs.

`golden_engine::application::ProductionRuntime` is the production implementation. Desktop and
transport code call it through typed operations for transactions, ticks, project replacement,
persistence capture, and observation publication; they do not own or lock the engine directly.

Project replacement is a prepare/commit/retire transaction. Decode and host configuration happen
before the runtime call. The runtime then stabilizes and validates the detached candidate without
running `on_node_ready`, compiles it while the old project remains authoritative, releases the old
project's live resources, activates and recompiles the candidate, and prepares the complete UI
projection. The commit fences dense inputs and switches the engine, semantic generation, clean
history, project-file metadata, read model, and a monotonic `ProjectGeneration` in one control-actor
turn. Superseded candidates are discarded by generation before they can acquire devices.

If activation or publication preparation fails after old resources were released, the old authored
project and its projection remain authoritative but are explicitly `Paused`; ticks and UI edits are
rejected until a later project replacement succeeds. A committed replacement owns the new devices
before the old engine object is dropped outside the actor. At most two replacement callers may own
detached-engine retirement at once; overload is rejected before generation allocation or candidate
preparation, so stalled cleanup cannot create unlimited replacement work. Duplication lifecycle
failures run destroy callbacks and restore the pre-operation graph/history/event publication
boundary.

Project saves run through `golden_persistence::PersistenceCoordinator`. The production facade
incrementally publishes authored nodes into a 256-shard copy-on-write document projection. A save
clones only those immutable shard roots together with their project generation and document
revision in the actor turn; sparse document materialization, JSON encoding, and disk work happen
after leaving the actor. Opaque node mutation and script APIs explicitly mark their target dirty so
the projection cannot miss authored data that has no ordinary graph event. The coordinator orders
every accepted save for one normalized destination, permits bounded cross-destination concurrency,
and holds the transaction lease through path/saved-revision publication. Later edits therefore
remain dirty, and a slower earlier Save As cannot overwrite a newer successful request's metadata.

Replacement takes an exclusive persistence generation fence only after detached preparation.
Already-committing saves finish their complete file and metadata transaction before cutover;
accepted old-generation saves that have not started writing are rejected after cutover. Save code
never holds the control actor while waiting for a persistence lease, avoiding actor/coordinator lock
inversion.

Pure comparison evaluators have no effect authority. External output requires an
`AuthoritativeOutput` issued by the composed application facade, preventing comparison or
diagnostic paths from duplicating commands, triggers, effects, or device traffic.

`GraphEditing` and `ProjectTransactions` convert the same authoritative `UiAck` used by UI and
transport into typed Rust results. Success returns the acknowledgement or history revision captured
in the mutation's actor turn. Rejection returns `GraphEditError`, which retains the shared code,
message, acknowledgement, and post-operation history state. The public facade never performs a
second history read that could race a later edit, and unavailable undo/redo are rejections rather
than successful no-ops. Script or headless adapters should consume these application contracts
instead of inferring success from transport or engine side effects.

Live transports submit edits with `UiClientLease`, a connection lifetime independent of browser
and edit identifiers. The socket closes its lease before waiting for hub cleanup; eviction closes
the same signal. Binding handshake identity preserves that signal. Queued requests from a closed
connection complete as cancelled without an authoritative acknowledgement. On the next actor turn,
the runtime finalizes that connection's active edit into undo history and publishes its released
state. Accepted edits remain undoable. A replacement connection gets a fresh lifetime, so delayed
cleanup cannot cancel it even when browser and edit identifiers are reused. Project replacement
discards the previous session owner. Only one active session lease is retained; no disconnect retry
queue or historical client index grows with reconnects.

An anonymous edit is finalized before the same connection starts using its handshake identity.
The WebSocket host permits one stable identity binding per connection. A second different identity
closes that connection; repeated handshakes with the same identity are idempotent.

Worker ingress carries reserved node notifications through `NodeWaker`, never arbitrary edits,
boxed nodes, closures, or payload buffers. A module owns its bounded result mailbox, parsing,
timestamps, and recovery state. It reserves `ProcessCtx::node_waker` in an engine-owned callback
before starting a producer and publishes mailbox entries before signalling; hosts can reserve
`Engine::node_waker` for an attached node. Graph operations
from other threads use the public application actor contracts. Structural worker results become
node-owned edits, preferably a detached `NodeTree`, when the node consumes its mailbox in
`Node::on_worker_wake`.

Each engine permits 1024 live/retiring registrations and reuses the active reservation per node.
Queued notifications coalesce; a signal arriving during a callback survives its acknowledgement.
The receiver extracts one bounded turn before callbacks run, with 256 entries by default through
`RuntimeLimits::max_wake_callbacks_per_tick`. Timed work has a separate callback budget. Wake
callbacks synchronize parameter handles and request snapshots through
`worker_wake_requires_tree_snapshot`. They use their own hook with zero scheduled delta and never
advance the timed `update()` baseline, including for replay-policy nodes. Modules must re-arm
their waker if their own drain budget leaves pending mailbox work.
Worker callbacks are suppressed while disabled by default. A node can explicitly opt in through
`worker_wake_when_disabled` while it owns accepted asynchronous completions or retiring resources.
Such service uses the same callback budget, zero scheduled delta, and receipt handling; it never
enables timed updates. The node must reject new work and avoid starting disabled resources.
TCP client and UDP use this hook to finish graceful retirement while disabled. Removal still
closes the registration, and enum dispatch forwards the hook for built-in and consumer variants.
These short-lock notifications belong on host workers; realtime audio callbacks keep using their
realtime-safe queues.

App endpoint command nodes capture a leased canonical snapshot and original execute payload
through the app-owned `CapturedInvocation`. Consuming workers decode fields and the last matching
override; scripts retain engine-owned argument arrays. The old second command-request JSON relay
and actor-side execute-overlay helpers are removed. Native admission and completion are separate:
failed validation sends nothing, cancellation reports not sent, and interrupted started effects
remain unconfirmed. Keyboard and Mouse share app-owned OS output ordering; see their focused
[Keyboard](keyboard-runtime.md) and [Mouse](mouse-runtime.md) contracts.

## Parameter event attribution

`golden_engine` owns parameter-change provenance and callback scope. Trusted Rust producers use
`ProcessCtx::set_param_from` or `EditQueue::push_from` to attribute a direct `SetParam`,
`SetParamConstraints`, or parameter state change made by `CallNodeMutation`.
The emitted `EventKind::ParamChanged::source` belongs to that
particular applied change. Unchanged writes emit nothing, rejected suffixes leave no suppression
state, and queue coalescing replaces the source together with the value. Attribution does not
propagate into lifecycle callbacks or queued follow-up edits, including those queued by an
attributed node mutation.

The source is runtime metadata, excluded from serialized events and UI DTOs. Undo/redo emits a
new authored change without the original runtime source. It is not an authorization mechanism.
During the standard node inbox callbacks, `ProcessCtx::current_event` exposes the exact event,
including its captured old/new values; the shared tree snapshot contains the final batch value.
The dispatcher restores this scope after each callback without copying event payloads.

Modules use producer identity to suppress their own received-value echo, rather than counting
future changes by parameter ID. App policy, feedback message construction and native admission
remain in the app-owned module. MIDI's concrete contract is in [MIDI delivery](midi-runtime.md).

Sound Card runtime facades contain controls and observation readers. Native engine ownership,
blocking shutdown and joins remain on a prestarted owner with a reserved lifetime, including
startup failure, discarded results and node removal. Eight bounded startup workers and sixteen
active/retiring native owners cap resource churn. Full startup queues retain their worker;
replacements wait for the previous native cleanup within that controller lifetime. Producer exit
drains its published event prefix before retry. Event byte/service bounds, overload delivery
policy and configuration preparation remain separate P03 acceptance work.

Engine-owned inbox, script/mutation, scheduled and worker callbacks also carry wake registration
capability, allowing idle nodes to reserve only when admitting asynchronous work. A completed
command releases its source registration instead of reserving one for every authored command.
`ProcessCtx::node_waker_for` validates another target and reserves that exact attached lifetime;
`NodeWaker::is_closed` never reopens a restored identity. The registry tracks attached node IDs
separately from callback snapshots, so an old read model cannot register a removed target.

`CustomEvent::local` shares typed, immutable runtime data through the ordinary inbox without JSON
encoding/decoding. Its local attachment is skipped by serialization and always treated as transient,
including if public retention metadata is edited. It never enters UI replay or clears redo history.
The caller supplies admission and final-disposal ownership; Golden contains no app payload types.
Ordinary JSON events and generated wire declarations keep their existing representation.

TCP client and UDP prepare incoming stream values, callback JSON, and bounded diagnostic logs
on their native workers using app-owned common streaming code. Input plans separate live parsing
settings from socket identity: grammar/processing changes invalidate a partial frame; logging
changes preserve it. TCP also fences partial frames by connection generation and reports abandoned
frames on recovery or graceful retirement. Prepared results keep their original interpretation.
The owner retains at most one four-event drain, publishes callbacks through individual application
receipts, then submits the prepared values to the shared graph cursor. Callback-only service needs
no tree snapshot. Destroy/undo restores authored state with fresh runtime input, and final prepared
payload disposal uses the shared stream pool. TCP transport warnings belong on its connection folder.

Worker-prepared TCP/UDP script callback JSON uses `NativeCallbackPayload` in the app's module
script API. Its canonical Golden IO pool retains ordinary event `Arc` copies through routing,
UI replay, and exported event batches, independently of the prepared input envelope. Reservations
admit at most 16,384 allocations / 256 MiB across these callbacks; each payload is limited to
32 MiB, 262,144 JSON values, and 64 levels. Measurement and admission run on the worker, including
spare container/string storage. Owner binding uses the shared payload and cached weight. Capacity
exhaustion explicitly rejects preparation and recovers after consumers release storage; it does
not replay external work or evict accepted callbacks. Final JSON destruction belongs to the
canonical disposer, including after node or engine removal. The stream envelope/value pool has
its separate 256-allocation / 256 MiB limit. HTTP also uses this canonical callback reservation.
Other callback producers still need their own integration; these limits do not qualify them.

HTTP separates native outcomes from admitted owner replies. The worker prepares both the response
callback and its graph-application-failure alternative, and retains response/request/body/parsed-value
storage in the app HTTP result pool (256 allocations / 256 MiB). Nodes receive immutable handles;
callback selection/binding and receipt service do not rebuild JSON or destroy large reply containers.
Existing HTTP response/JSON complexity and 32 MiB reply limits apply before publication, including
the prepared callbacks. Callback/result capacity failure produces a bounded typed rejection carrying
request description, command identity, and attempt count. Its generation closes command admission;
the accepted suffix receives failure/rejection outcomes without sending more requests. The node
reports result rejection through a warning and log, then clears that warning when admitted results
recover. Warning set and clear operations retain individual receipts; a discarded clear is retried
before the recovery callback is published. Such rejection precedes graph/callback delivery, never fabricates a script callback outside
its budget, and never resends endpoint effects. Normal callback names, arguments, replay, and
individual publication receipts remain unchanged.

HTTP owner service is passive. The module reserves its node wake before starting workers and uses
the app's module-common `ModuleWake` adapter for notifications and coalesced retry deadlines, shared
with TCP client and UDP. Replies, request-resolution phase changes, and native producer exit signal
the owner; resolved generations notify before native cleanup can block. Bounded owner turns re-arm
while graph/callback receipts remain, and request snapshots only for configuration or graph planning.
HTTP keeps accepted completions paused while disabled; enable explicitly re-arms delivery and
admission recovery. Destroy/replacement cancels its producers, clears pending runtime results and
deadlines, and starts a fresh wake source when restored. Script argument parsing lives beside the
HTTP coordinator in `scripts.rs`; HTTP selects no periodic/compiled node kernel.

TCP server native ownership uses `golden_io::BoundedWorker`: resolve/bind/retry and socket cleanup
run off the actor, with at most 16 live or retiring native tasks. Broadcast admission checks a 1 MiB
payload/storage bound before copying; its queue allows 256 items / 8 MiB. Replies allow 64 items /
32 MiB. Input continuations share that byte limit; output/status continuations are at most 64 KiB.
A native turn performs at most four operations per
input/output/status producer. Input reply saturation leaves outgoing work runnable. The server
owns at most 128 clients and closes overflow connections; disconnect releases capacity.

Each broadcast captures the currently live native client cohort when it starts, preserving the
existing broadcast delivery boundary. It shares one payload across round-robin client cursors,
writes at most 64 KiB per attempt, and has a five-second absolute deadline. Partial write failures
close the original connection with its written-byte count; neither a suffix nor a completed prefix
is replayed on another connection. Whole broadcasts remain FIFO. Bind failure retries with bounded
backoff, and native Listening/Recovering events own module connection feedback. Applied port and
enabled state fence new script/command admission before callback dispatch.

TCP server owner service is passive and reserves its wake before starting native work. Each client
has a separate parser, bounded to a 1 MiB partial frame (128 MiB maximum across 128 clients).
Native input generations discard partial frames on grammar/processing changes; logging changes
preserve them. The shared stream input envelope prepares source-tagged callbacks, graph values,
and bounded logs on the worker. Connection/disconnection callbacks also use the canonical native
callback pool. The owner retains one bounded native drain through callback publication receipts;
discarded publication retries only that event. Snapshot-free callback service precedes bounded
graph application. Parameter/metadata edits, replies, producer exit, and retry deadlines wake service.

Ordinary reconfigure/disable closes native admission and retains one retiring owner through its
accepted native command prefix and result publication. Already-drained callbacks and graph batches
keep their separate delivery owners until their receipts settle. Disabled service
explicitly opts in for that accepted prefix, and retired client status cannot restore the live
client list. A replacement starts after the old producer resolves; destroy alone cancels producers
and discards runtime deliveries. Undo reserves a fresh wake source. The server has no periodic or
compiled node kernel. Broader producer admission/disposal audits remain P03 work.

TCP and WebSocket Server share app-owned `common/streaming/server_feedback.rs` for connection,
warning, client-count and client-node projection ownership. Protocol adapters supply target IDs,
warning IDs and client-parameter factories, retaining their existing metadata. The component owns
one in-flight projection receipt; newer desired state stays dirty until that receipt settles.
A discarded or partly applied projection reconciles against a fresh snapshot, preserving committed
client identities and avoiding optimistic parameter-handle caches. Callback/graph/native frame
delivery remains independent, so projection recovery never replays those effects. Staging a
projection explicitly wakes its owner, including disabled completion service; destroy/undo resets
the component with the owner's wake generation.

Serial, TCP Server and WebSocket Server also prepare outgoing completion logs on their native
cursors through app-owned `common/streaming/outgoing.rs`. Logging policy is captured with each
admitted command. One bounded result reports confirmed output, with at most 1024 payload bytes
formatted; the actor forwards it without inspecting the byte buffer. Server summaries count only
confirmed members of the original native cohort, using explicit pending/confirmed/failed recipient
states. Submission, partial effects and failed recipients never claim full success; WebSocket
confirmation waits for flush. Full feedback mailboxes retain that single output continuation and
pause further output while independent input remains serviceable. Ordinary retirement drains the
confirmed results even while disabled; explicit destroy may cancel them. These messages describe
native send completion, not application-level acknowledgement by a peer.

The five stream command node types capture immutable snapshot leases and their fixed parameter IDs
before native preparation. One shared app worker admits 64 queued jobs and 256 total live tickets,
with at most 16 per command owner. It blocks on notification-safe receive while idle. Invocation
overrides reserve a separate 64-allocation / 16 MiB canonical input pool; each invocation permits
256 KiB, 4096 JSON values and 16 levels. Only dispatch-target validation happens on the actor
(reference overrides have a smaller 4 KiB/128-value/eight-level bound); decoding other overrides,
creating overlays, visiting values and encoding bytes/JSON happen on the native worker.

Command preparation bounds value trees to 4096 values, 16384 visited nodes, 64 folder levels and
4 MiB of source data. JSON encoding additionally limits depth to 32; final encoded output is at
most 1 MiB, with tighter transport limits checked before native send. Incremental byte writing
prevents separator/JSON expansion from allocating an unbounded intermediate output. Source string
views and flattened values borrow the captured model. Prepared requests reserve the shared stream
payload pool, including fixed envelope/lifetime overhead, and final disposal stays off the actor.

Owners publish one typed local request through an isolated edit receipt in FIFO order. Only a
discarded publication retries; an applied request is never replayed. Producer failure resolves its
active and queued tickets explicitly, and later admissions can restart the bounded worker. Source
cancellation and target lifetime guards reject stale work across destroy/undo. Linked target and
per-invocation values are captured before admission, while transport eligibility is checked when
the module receives the request. Preparation admission does not claim a successful native send.
Property/event/log ingress disposal and broader module admission audits remain P03 work.

Stream script methods capture the original engine-owned argument vector through
`ProcessCtx::capture_script_args`. Capturing an unrelated slice or a standalone callback is rejected;
the public Golden contract exposes shared arguments without app-specific conversion policy. App
common retains at most 64 input wrappers / 16 MiB of referenced storage, with 4096 values and
4 MiB retained storage per invocation, including spare vector/string capacity. Each wrapper owns
the Golden script argument lease; independent pools never claim the same raw Arc as final owner.
Taking a lease does not clone strings. Generic method admission is described in [hosts](hosts.md).

All six stream modules enqueue that input directly into their existing native send queues, alongside
prepared command-node payloads. Current enabled/configuration/connection checks still run before
native admission. Queue weight reserves a conservative output bound derived from argument shape,
capped by each transport's retained-byte limit. String/reference lengths and fixed scalar expansion
bounds are inspected without conversion. Small calls share queue capacity; native encoding cannot
retain output beyond its reservation. Temporary encoding work is bounded by the shared codec's
4 MiB input / 1 MiB output limits, and scratch capacity is trimmed before a send cursor owns it.

Argument conversion, escape decoding, byte/hex parsing and final input disposal occur off the actor.
Scripts retain the transport's original connection generation or frozen UDP destination plan and
share its FIFO, cancellation, native completion and ordinary retirement behavior. Disable/reconfigure
does not discard queued script inputs; explicit destroy may cancel them. Conversion failure produces
native error feedback without sending a prefix, replaying, reconnecting an otherwise healthy endpoint
or blocking later commands. Script invocation success means native queue admission; content/output
conversion failures are asynchronous feedback. Argument-shape, lifetime-capacity and endpoint
admission failures remain synchronous. Method arguments enter the engine in a prepared Golden lease,
so native admission rejection also preserves background final disposal. Properties, custom events
and staged logs share the same bounded Golden input lifetime contract; event routing/replay keeps
the original charge until its consumers release it. See [host ownership](hosts.md) for exact bounds.
The remaining module audit stays P03 work; accepted domain copies and engine bound-handle costs
have separate ownership and qualification.

Encoded command delivery also retains its canonical preparation lease through native admission.
Production send inputs accept only retained command or script data; raw vector/slice constructors
are test fixtures. Full or disconnected queues release a small lease on the actor, keeping final
buffer disposal on the shared payload pool. Serial, TCP Client and UDP write directly from the
original allocation. WebSocket frames and TCP broadcasts create owned buffers on their native
worker when the downstream API consumes a vector.

Canonical storage, including spare capacity and envelope metadata, keeps its existing pool
reservation. Each native queue accounts for its own header and logical output bytes; it does not
charge the shared allocation again for every lease. The outgoing state is exclusive: prepared,
script, owned native bytes, or terminal failure. The module checks source/target eligibility before
native admission. After acceptance, the original transport generation/cursor owns the prefix;
retiring the command source does not revoke already admitted bytes.

WebSocket client uses the same passive wake, retained input and callback-receipt contracts. Its
native owner uses the locked Tungstenite public nonblocking handshake/framing/control API, with
at most 16 live or retiring workers. Commands allow 256 items / 8 MiB, with a 1 MiB retained frame
bound checked before copying; replies allow 64 items / 32 MiB and one input/output/status continuation
per source. The protocol limits frames and reassembled messages to 1 MiB and bounds its write buffer
to 2 MiB plus 1 KiB. Each native turn resets 64 KiB and four-syscall budgets per direction, preventing
library fragment or partial-flush loops from escaping the native service budget.

Frames submit once to the codec and retain a flush cursor with a five-second deadline. Failure
reports an unconfirmed effect and never replays it. Generation-tagged queued frames cannot move
onto a new connection. Native setup resolves at the I/O boundary, checks up to eight addresses with
100ms connect attempts, and polls both cancellation and admission closure during its three-second
handshake deadline. Reconnect backoff is 250ms–5s. Ping/close handling remains active even when
application receive is disabled; shutdown and graceful retirement do not wait for peer ACKs.

Applied host/port/path fence script and command admission before callback dispatch. Grammar and
processing revisions discard partial frames; log-only revisions preserve them. Worker preparation
keeps binary callbacks distinct from text callbacks. Shared stream envelope rejection and native
oversize feedback invalidate partial input rather than joining later bytes across a dropped batch.
An owner retains its accepted frame/input/callback prefix across reconfigure and disable, then
starts the replacement. Destroy alone cancels producers and drops runtime deliveries; undo uses
a fresh wake source.

WebSocket server now uses bounded native ownership for resolution, bind/retry, concurrent upgrades,
frame I/O and cleanup. It shares the client's finite codec configuration and per-direction
64 KiB/four-syscall socket budget. The server reserves at most 16 native lifetimes; each has
256 commands / 8 MiB, a 1 MiB retained command limit checked before copying, 64 replies / 32 MiB,
and one retained continuation per input/output/status producer. The combined active-peer and
pending-handshake limit is 128. Pending handshakes rotate through at most four advances per native
turn, with a three-second deadline; an idle HTTP client cannot block an unrelated upgrade or output.
Paths are exact (empty accepts any); authored and received paths are bounded before retaining copies.

Each broadcast captures the current native peer identities when its command begins. Recipients
share the message payload but retain independent submit/flush state, five-second deadlines and
round-robin progress. A vanished peer's suffix fails explicitly and cannot target a replacement
with the same public address. WouldBlock flushes the existing encoded frame without resubmission.
Receive-disabled peers still service protocol ping/close messages. Cancellation/drop never joins
on the engine; a blocked native setup retains its lifetime slot until cleanup. Bind failures retry
with 250ms–5s backoff; native Listening/Recovering events own connection feedback.
Temporary native lifetime exhaustion retries after a coarse owner deadline, without rebuilding
snapshots while that deadline is pending.

Applied port/path/enabled values fence command/script admission before configuration callbacks.
Each native peer owns a parser with a 1 MiB partial-line limit (128 MiB maximum across 128 peers).
Grammar/processing generations invalidate that peer's partial input; logging-only edits preserve it.
Workers prepare peer-tagged values, bounded logs and canonical retained callbacks, preserving
text/binary callback semantics. Input-envelope rejection invalidates only the rejected peer's parser.
Connection/disconnection callbacks use the same canonical payload pool; disconnection reports
discarded partial lines. Input settings update live without rebinding the listener.

The module is passive: replies, receipts, configuration/metadata edits, native producer exit and
retry deadlines wake focused service. One bounded native drain stays owned through callback
publication receipts, and discarded publication retries that callback without rebuilding input.
Snapshot-free callback delivery precedes bounded graph application. No periodic or compiled node
kernel remains. Ordinary disable/reconfigure closes native admission and retains one retiring
producer through its accepted frame/input/callback prefix. Disabled wake service finishes that
prefix; retiring statuses cannot restore the live client projection. Replacements wait for old
producer disconnection. Destroy cancels native producers, clears runtime deliveries and releases
the wake source; undo reserves a fresh source. Per-peer codec/parser buffers are finite under the
128-peer/16-lifetime limits; this is not a product-scale memory qualification.
Connection/warning/client projections use the shared server-feedback contract above.
Serial and broader producer admission/disposal audits also remain P03 work.

Serial's app-owned common layer separates connection, discovery and metadata responsibilities.
The existing locked serialport crate owns platform APIs. Connections reserve at most 16 native
lifetimes, including blocked open/cleanup, with 256 queued commands / 256 KiB and a 256 KiB retained
command bound checked before copying. Replies allow 64 items / 32 MiB and one retained continuation
per input/output/status producer. A native turn advances at most four command operations; each
write slice and read buffer is at most 8 KiB. Driver I/O requests use a 20ms timeout; cancellation
cannot interrupt an uncooperative driver, so that worker retains its lifetime instead of joining
the actor or allowing unlimited replacements.

Write cursors retain the confirmed prefix on the original connection and have a five-second
active-command deadline. WouldBlock/Interrupted resume only the suffix. Other errors, including
timeouts that may hide partial Windows WriteFile effects, report an unconfirmed effect without
replay. Reconnect generations reject old queued commands rather than sending them on replacement
devices; reopen backs off 250ms–5s. DTR/RTS setup errors now enter recovery before claiming connected.
Input and output reply pressure have independent retained continuations; results are not silently
dropped at a full mailbox. Native graceful admission closure drains its accepted output prefix.

Discovery reserves one live/retiring worker. Last-unsubscribe shutdown cancels without joining;
a blocked enumeration retains that slot, and cancelled/ownerless results cannot restore the catalog.
Enumeration errors retain the last good catalog and recover on later scans. Applied port, baud,
DTR, RTS and enabled values fence new script/command admission before callbacks. Temporary native
connection/discovery capacity retries use coarse owner deadlines.

Serial owner service is passive. It reserves a wake before subscribing or opening a connection;
replies, receipts, configuration edits and coarse retry deadlines drive service. Native workers
prepare parsed values, canonical byte/text callbacks and bounded incoming logs. Connection and
input-plan generations discard partial frames across reconnect, grammar and processing changes.
Only a bounded drain is retained while callback receipts complete. Ordinary disable/reconfigure
closes admission and retains the old worker until its accepted output and pending input complete;
retiring status cannot restore connected. Destroy cancels, releases catalog subscriptions and
clears deliveries; restored nodes reserve a fresh wake source.

Discovery admits at most 1024 subscribers and one live/retiring producer. Catalogs admit 128 ports,
2 MiB including prepared enum options, and bounded retained string/tag capacities; canonical
payload leases own final disposal off the actor. Snapshot reads copy a header and lease. Catalog
comparison/options preparation run on the discovery worker, and notifications run outside its
lock. Last unsubscribe invalidates the producer epoch; late enumeration cannot restore state.
Producer exit notifies subscribers, with coarse restart deadlines and no owner polling. Failed
enumeration or catalog admission retains the last good catalog.

One receipt owns each serial catalog/connected/warning projection. Only successful completion
acknowledges its captured catalog/selection generation; failed or partly applied edits reconcile
against a fresh snapshot, while newer desired state stays pending. A missing selected port remains
selectable. These bounds and deterministic fixtures do not qualify physical line timing, native
UI interaction, or product-scale main-thread latency and memory.

HTTP admission reads applied snapshot values and compares them with the authored inputs captured
for the active generation, so a configuration edit followed immediately by a command or script
call cannot send to the previous endpoint or with previous credentials. This comparison borrows
bounded values; URL/header preparation happens only when configuring a generation. Disabled auth
and inactive scheme fields do not affect identity or retain unused credentials. Source limits are
8 KiB for the base URL and each active auth field, 64 KiB for header text, and 256 default headers.
Native configuration also checks retained allocation capacity (128 KiB total, including vector and
string spare capacity) before cloning settings or reserving a worker lifetime. Invalid/pending
settings reject new admission while accepted requests and completions retain their existing order.

HTTP scripts and command nodes enqueue typed retained input; parsing, header/form preparation,
body encoding and invocation snapshot overlays run on the generation's HTTP worker. Commands
capture one immutable view and use the shared app command-input boundary with stream commands.
Overrides are limited to 256 KiB / 4096 nodes / depth 16; HTTP command parameters admit 4 MiB
total retained storage. Content conversion failure produces bounded asynchronous warning feedback
with zero endpoint attempts and does not disconnect or replay a healthy generation. Existing
`responseReceived` / `requestFailed` native callbacks and their completion receipts are preserved.

`golden_io::BoundedWorker::spawn_notified` can publish those node wakes after reply admission and
final producer exit. `DelayedNotifier` provides bounded, coalesced host deadlines without a node
polling clock. A `PayloadPool` can retain prepared data through consumer ownership and perform
its final destruction on a background disposer; graph/event-owned copies keep their own lifetimes.
`ProcessTreeSnapshot::retained_bytes` exposes a cached conservative allocation weight, computed
when the view is built; cloning the view and reading that weight are constant-time. A background
consumer can acquire `try_lease` without copying node records. Golden Engine's canonical
384-allocation / 1 GiB pool reserves the outer view and its deduplicated shared storage, including
data still held by ordinary clones or overlays. Its disposer owns final destruction. Exhaustion
returns a typed rejection before handing a snapshot to a new consumer; it never creates an
unbudgeted cleanup worker. App-specific preparation and destination policy remain in the app.

Unprocessed wakes and callback-absorption failures return to their reserved slots. Deferred-edit
receipts retain readiness until application settles; failure re-arms the node's level notification,
without replaying an edit batch or consumed command. The node still owns cursor/payload recovery.
External work is not serviced inside an edit journal. Removal and replacement close the old source;
restoring the same node ID creates a fresh one. Engine ingress closes before native node destruction.
Closed handles remain reserved while a worker or delivery owns them, bounding retirement as well
as active admission. Modules should release their own handle when destroyed.

Deferred producers can observe the current request set with `EditQueue::completion`. A snapshot
alone does not acknowledge application. The receipt remains pending through structural deferral
and atomic preparation; atomic commit publishes success, while rollback, rejection, discarded
suffixes, and superseded values fail it. Duplicate removals share the retained request's outcome.
Observed no-ops also wait for application instead of escaping an earlier rejected request.
Ordinary drains still retain their applied prefix, including mutations whose lifecycle completion
failed. A failed receipt therefore forbids assuming that nothing happened or blindly replaying
triggers. Receipts retain only counters/tokens proportional to the observed edits, not graph copies.
They cover those requests, not subsequent subscriber edits or external effects. Later undo does
not rewrite an earlier receipt. Engine ingress validation lives in `engine/ingress.rs` and transfers
the complete request ownership; it must not strip observation tokens while coercing node types.

Lifecycle materialization belongs to the engine, including detached project-load augmentation.
`LifecycleLimits` admits 64 edit/preprocessing passes and 65,536 generated work units per scope
by default. Work counts generated requests and newly inserted nodes; nested callbacks share
the active scope. Generated trees and user-item forests reserve their complete node count
before coercion/insertion, so a rejected forest cannot leave a partially inserted root.
Explicit detached input trees remain scalable inputs rather than consuming the callback
expansion allowance. Their generated follow-ups use the same limits. Hosts can lower or raise
the limits before entering a scope; an admitted scope keeps its original limits.

Budget exhaustion returns `LifecycleBudgetExceeded` (`lifecycle_budget_exceeded` through the UI)
and drops the generated suffix, failing its receipts. Ordinary application keeps its committed,
undoable prefix; an enclosing atomic operation restores its original graph and runtime activation.
These are work-count guardrails for engine-driven expansion, not preemption of arbitrary Rust
callbacks or a wall-clock deadline for a caller's explicit graph input.

Replacement retires only the outgoing payload, retains its children and identity, closes its
worker generation, and removes its subscriber registrations while preserving other listeners
to that identity. Fresh payloads run attached/init/ready through the guarded materialization path;
project-load ready callbacks remain deferred. The old payload stays stopped while history owns
it. Undo/redo stops the live payload and restores runtime readiness with a fresh worker generation,
without repeating authored initialization. Rejected history activation restores the previous
payload and reactivates it; atomic rollback also tracks replaced roots for reactivation.
Rollback deduplicates roots, so several replacements of one identity restart it once.

History replay restores the engine's effective-enabled cache and node callbacks when a metadata
toggle, cross-parent move, or node replacement changes a subtree's inherited enabled state.
Replacement also inherits the live node's cached state before reconciling descendants, including
under a disabled parent. Callbacks receive a snapshot after all changed cache values are applied.
Project load and imported-subtree insertion initialize each node's cache from its attached
parent before lifecycle callbacks. Process-tree snapshots project that cached effective state
directly; live metadata changes must therefore use engine edits rather than mutating an attached
node's metadata behind the engine's back.
Process-tree parameter projection carries only value, constraints, and control state; full UI
parameter snapshots still carry editor defaults and hints. Child-order slices remain indexed for
every parent. Declaration IDs and exact child aliases (declaration ID, short name, label) use hash
indexes only above 16 siblings. Smaller parents use a bounded ordered scan. Both paths preserve
the first matching sibling across aliases, including an earlier label before a later declaration
ID. Duplicate aliases share a key instead of allocating another string per node.
Golden retains one canonical process view across ticks. Mutable `NodeStore` access records a
bounded set of candidate IDs; sparse captures replace those records and only affected parent
order/selector indexes. Unchanged records, UUIDs and indexes share immutable storage with older
views. Maps compact after eight delta layers or dense changes. Removals, store/root replacement,
UUID rebinding and collisions use full capture. Synthetic parameter overlays remain separate.
Each shared map layer has its own deduplicated canonical lease, including retained parent layers;
the existing 384-allocation / 1 GiB budget is unchanged. Partial lease admission is released.
Snapshot statistics count changed captures and compaction copies as well as full construction.
The public `process_tree_snapshot` API still captures a fresh complete view.
Reference-cache maintenance first checks whether an immutable node exposes references. Nodes
without references need neither parameter snapshots nor mutable access that would dirty the journal.

Large tick-scoped and canonical process snapshots release through a two-slot retirement pool when uniquely owned;
small or shared snapshots use ordinary release. Saturation or worker-start failure falls back to
synchronous release, and `process_snapshot_retirement_metrics` exposes occupancy and rejection.
The pool starts no worker until a large snapshot is actually retired.

`ProcessTreeSnapshot::subtree_revision` identifies engine-published changes within an authored
subtree before callback delivery. Parameter, metadata, topology, script configuration, and opaque
authored edits advance that subtree and its ancestors. A shared change-target classifier also
feeds project dirty tracking; neither feature depends on UI replay publication. Revision namespaces
belong to one engine lifetime. Undo/rollback may advance a revision even when values return to an
earlier state; consumers compare identity rather than interpreting the counter. Immutable snapshots
retain their old identity; parameter overlays invalidate conservatively in a fresh namespace.
Disjoint subtree edits and custom traffic events preserve a prepared read model's identity.
Ancestor metadata is outside this subtree identity, so consumers must check inherited root enabled
state separately. Direct mutation of attached authored state must use engine edit APIs.
Mutation bookkeeping marks only changed nodes and explicit topology parents in constant time per
target. Initial capture folds descendants iteratively. Refresh scans compact local counters and
propagates new stamps through a bounded union of ancestor paths, sharing untouched folded entries.
Structural changes require fresh parent stamps and acyclic bounded paths; these stamps dominate
previous descendant maxima, including a moved subtree's old parent. Raw topology edits, deep/dense
work and namespace changes use the complete iterative fold. Folded counter layers share the same
canonical reservation and disposer as records and selectors. Revision lookup is bounded by the
map's layer limit and retains no engine.

Applying a subtree read precondition does not construct that whole-tree projection. Golden folds
local authored counters over the guarded live subtree and compares the exact captured stamp.
Overlapping guards share a temporary fold within one fence; following requests always start a new
fold, so an earlier applied edit cannot hide behind cached results. Work and temporary storage are
bounded by the union of guarded subtrees, independently of disjoint project payloads. Traversal is
iterative over validated engine topology; UUID, inherited enabled-state and namespace checks remain.

`RecordingModuleIo` captures versioned inputs and authoritative outputs using an injected clock.
Deterministic clocks keep protocol and hardware fixtures repeatable without putting polling or
device work on the engine loop.
