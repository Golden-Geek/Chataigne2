# Application Contracts

`golden_application` is the app-agnostic contract layer. It owns public project-transaction,
graph-editing, runtime-value, observation, module-I/O, persistence, and host-lifecycle traits.
`ApplicationFacades` composes those concerns independently so applications and hosts do not depend
on engine internals.

`golden_engine::application::ProductionRuntime` is the production implementation. Desktop and
transport code call it through typed operations for transactions, ticks, project replacement,
persistence capture, and observation publication; they do not own or lock the engine directly.

Pure comparison evaluators have no effect authority. External output requires an
`AuthoritativeOutput` issued by the composed application facade, preventing comparison or
diagnostic paths from duplicating commands, triggers, effects, or device traffic.

`RecordingModuleIo` captures versioned inputs and authoritative outputs using an injected clock.
Deterministic clocks keep protocol and hardware fixtures repeatable without putting polling or
device work on the engine loop.
