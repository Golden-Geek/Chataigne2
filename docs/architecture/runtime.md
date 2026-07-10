# Compiled semantic runtime

Phase 5 introduces `golden-runtime`, the semantic execution plane. It accepts a domain-neutral `GenerationSpec` at the compilation boundary and publishes an immutable `RuntimeGeneration`; it has no dependency on `golden-graph` and cannot access editable project documents.

The control plane is single-owner actor state. Its asynchronous compilation service builds candidate generations on a worker thread while the current valid generation continues running. Successful candidates are migrated and published through an atomic generation slot only at the control boundary. Failed or superseded candidates never disturb semantic execution.

Each generation contains typed dense bool, integer, and float slots; direct generation-stamped input slots; flattened dependency routes; immutable operation batches; stable state keys; and deterministic effect-order keys. Value ticks therefore perform no project snapshot, topology traversal, or binding reconstruction. Growable scheduler and effect buffers are reserved from the generation layout; tick metrics report any unexpected internal capacity growth as a semantic allocation.

The scheduler uses a preallocated min-heap for sparse routed work and switches to deterministic dense batches once the compiled threshold is reached. Effects are staged, sorted by their semantic order key, and committed only after evaluation. Generation swaps migrate matching typed slots and stable state while invalidating direct slots from the previous generation.

Start with:

- `crates/golden-runtime/src/model.rs` for the graph-free generation contract;
- `crates/golden-runtime/src/compiler.rs` for validation, dense layout, batches, and routes;
- `crates/golden-runtime/src/semantic.rs` for sparse/dense execution and atomic swap;
- `crates/golden-runtime/src/service.rs` and `control.rs` for background compilation and ownership;
- `crates/golden-runtime/src/tests.rs` for canonical workload gates.
