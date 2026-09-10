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
captures an owned sparse document and its project generation/document revision in one actor turn,
accepts a monotonic destination ticket, and performs JSON encoding and disk work outside the actor.
The coordinator orders every accepted save for one normalized destination, permits bounded
cross-destination concurrency, and holds the transaction lease through path/saved-revision
publication. Later edits therefore remain dirty, and a slower earlier Save As cannot overwrite a
newer successful request's metadata.

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

`RecordingModuleIo` captures versioned inputs and authoritative outputs using an injected clock.
Deterministic clocks keep protocol and hardware fixtures repeatable without putting polling or
device work on the engine loop.
