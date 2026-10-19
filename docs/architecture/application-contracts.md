# Application Contracts

`golden_application` is the app-agnostic contract layer. It owns public project-transaction,
graph-editing, runtime-value, observation, module-I/O, persistence, and host-lifecycle traits.
`ApplicationFacades` composes those concerns independently so applications and hosts do not depend
on engine internals.

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
timestamps, and recovery state. It reserves `ProcessCtx::node_waker` in its ready callback before
starting a producer and publishes mailbox entries before signalling; hosts can reserve
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

TCP client and UDP prepare incoming stream values, callback JSON, and bounded diagnostic logs
on their native workers using app-owned common streaming code. Input plans separate live parsing
settings from socket identity: grammar/processing changes invalidate a partial frame; logging
changes preserve it. TCP also fences partial frames by connection generation and reports abandoned
frames on recovery or graceful retirement. Prepared results keep their original interpretation.
The owner retains at most one four-event drain, publishes callbacks through individual application
receipts, then submits the prepared values to the shared graph cursor. Callback-only service needs
no tree snapshot. Destroy/undo restores authored state with fresh runtime input, and final prepared
payload disposal uses the shared stream pool. TCP transport warnings belong on its connection folder.

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
Large tick-scoped process snapshots release through a two-slot retirement pool when uniquely owned;
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
target. Snapshot creation copies that index and folds descendants iteratively in linear time
alongside its existing whole-tree projection; revision lookups are constant time and retain no engine.

`RecordingModuleIo` captures versioned inputs and authoritative outputs using an injected clock.
Deterministic clocks keep protocol and hardware fixtures repeatable without putting polling or
device work on the engine loop.
