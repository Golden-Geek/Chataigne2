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
before the old engine object is dropped outside the actor. Duplication lifecycle failures run
destroy callbacks and restore the pre-operation graph/history/event publication boundary.

Pure comparison evaluators have no effect authority. External output requires an
`AuthoritativeOutput` issued by the composed application facade, preventing comparison or
diagnostic paths from duplicating commands, triggers, effects, or device traffic.

`RecordingModuleIo` captures versioned inputs and authoritative outputs using an injected clock.
Deterministic clocks keep protocol and hardware fixtures repeatable without putting polling or
device work on the engine loop.
