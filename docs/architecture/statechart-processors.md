# Statechart, condition, context, and processor foundation

Phase 4 separates authored control models from their executable forms.

`golden-statechart` owns the state graph domain and compiles it into immutable state and transition tables. Runtime state is one non-multiplexed active configuration; context lanes never duplicate statechart truth. Transitions emit entry and exit processor references without depending on the processor crate.

`golden-condition` compiles authored predicate trees into postfix stack operations. Runtime condition checks consume only this program and a keyed input set, so no condition-node walk occurs during processing.

`golden-context` composes explicit replace/accumulate layers before `LaneLayout` performs cardinality checks or materializes stable lane keys. `golden-processor` then creates one formula instance per lane while every instance shares the same immutable Alchemist kernel. Mapping creation has one definition path regardless of input count or optional condition.

The built-in Action and Mapping assets are real formulas. Mapping uses the reusable `condition_gate` ANode; the processor layer obtains these assets through Alchemist's public single-node formula builder and does not import graph internals.

`apps/chataigne/backend` contains only the product composition boundary that resolves statechart processor references and evaluates registered processor runtimes. `golden-statechart-ui` composes `golden-graph-ui` and applies keyed active-configuration deltas.

Start with:

- `crates/golden-statechart/src/compiler.rs` and `runtime.rs`;
- `crates/golden-condition/src/lib.rs`;
- `crates/golden-context/src/lib.rs`;
- `crates/golden-processor/src/compiler.rs` and `runtime.rs`;
- `apps/chataigne/backend/src/composition.rs`.
