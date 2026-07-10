# Alchemist foundation

Phase 3 rebuilds formula authoring and evaluation as a domain layer over `golden-graph`.

`golden-alchemist` owns formula metadata, typed surface ports, managed regions, ANode registration, the versioned formula codec, compilation, kernel caching, and formula instances. The authored `GraphDocument<AlchemistGraphDomain>` is compiled once per formula/graph/registry revision into dense value slots and ordered operations. Runtime evaluation reads only that immutable kernel; it does not traverse authored nodes or edges.

Functional outputs are direct slot reads. Observation is an explicit evaluation option, so previews and inspectors do not impose work on headless or unobserved instances. Pure instances retain dirty-slot state and return immediately when their inputs have not changed. `evaluate_batch` executes multiple instances that may share the same `Arc<CompiledFormulaKernel>`.

`golden-alchemist-ui` adapts formula nodes to `golden-graph-ui`, composes the reusable graph canvas, and keeps catalog, surface, and runtime-output revisions separate. It presents runtime values but does not decide formula semantics.

Start with:

- `crates/golden-alchemist/src/formula.rs` for the authored contract;
- `crates/golden-alchemist/src/compiler.rs` for graph-to-kernel lowering;
- `crates/golden-alchemist/src/runtime.rs` for dense evaluation;
- `crates/golden-alchemist/src/codec.rs` for the versioned file boundary;
- `packages/golden-alchemist-ui/src/formula-store.ts` for keyed UI state.
