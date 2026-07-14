# Application Seams and Phase 2 Shadowing

Phase 2 introduces `golden_application` as the app-agnostic contract layer. It owns the public
project-transaction, graph-editing, runtime-value, observation, module-I/O, persistence, and host-
lifecycle traits. `ApplicationFacades` composes those concerns independently so later phases can
replace one subsystem without changing Chataigne UI or node registration.

`golden_engine::application::ProductionRuntime` is the governed production adapter. The current
engine remains authoritative, but desktop and transport code no longer own or lock its mutex.
Transactions, ticks, project replacement, persistence capture, and observation publication cross
typed operations on the adapter. Its deletion criteria and expiry are recorded in the
[Phase 2 seam dashboard](../product/manifests/phase2-seams.v1.json).

Shadow execution is restricted to `PureEvaluator`. It returns the authoritative output plus a
semantic-digest comparison and has no effect authority. External output requires an
`AuthoritativeOutput` issued by the composed application facade, so a shadow hook cannot duplicate
commands, triggers, effects, or device traffic through the typed I/O boundary.

`RecordingModuleIo` captures versioned input and authoritative-output recordings using an injected
clock. Deterministic clocks make protocol and hardware fixtures repeatable without adding polling or
device work to the engine loop.

Run `python tools/migration/check_phase2_contracts.py` to validate the seam dashboard, adapter
governance, reusable-package direction, and the ban on shared engine mutexes in host/transport code.

