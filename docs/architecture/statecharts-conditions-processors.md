# State Machine And Alchemist Processing

Chataigne has one state-machine system and one formula-processing system. Both are app-owned.

## Ownership

- `apps/chataigne/systems/alchemist/` owns Formula and ANode models, compilation, execution, and
  the graph-domain adapter.
- `apps/chataigne/systems/alchemist/condition/` owns authored condition definitions, compilation,
  dense runtime state, and input/script evaluation contracts.
- `apps/chataigne/systems/alchemist/processor/` owns processors, Inputs, Filters, Outputs,
  context lanes, `ValueSet`, lifecycle, and managed pipelines.
- `apps/chataigne/systems/state_machine/model/` owns Chataigne state identity, hierarchy,
  transitions, active configuration, and graph-backed edits.
- `apps/chataigne/systems/state_machine/runtime/` composes the model with Alchemist processors,
  arbitration, and protocol DTOs.
- Each system's `integration/` folder adapts its kernels to Golden Core nodes.
- `apps/chataigne/ui/src/lib/systems/alchemist/` owns Alchemist presentation, while
  `apps/chataigne/ui/src/lib/systems/state_machine/` owns the product panel and its local
  state-machine document projection. Both render graph surfaces through `golden_graph_ui`.

There are deliberately no generic Golden condition or statechart packages. These concepts encode
Chataigne behavior and remain with the product.

## Runtime path

The editable condition tree is authoring state. Alchemist lowers it into a flat compiled program
and direct parameter bindings. Each default or multiplexed processor lane evaluates that program
with its own migratable condition runtime. Inspector DTOs are projected from compiled observations,
so preview capture cannot invoke a second condition implementation.

Processor formulas compile once per semantic formula key and are shared by identical instances.
The lane compiler combines inherited context axes, context-linked properties, and output axes into
a stable execution plan. Stateful memory is keyed by `ContextKey`, retained only for active lanes,
and initialized only for new lanes.

The state-machine runtime selects lifecycle and transition work, then dispatches through the same
Alchemist processor and output-intent path used elsewhere. It does not own alternate Formula,
condition, input, filter, or output implementations.

## Scale and safety

Compiled conditions do not walk editable condition nodes in steady state. Formula kernels are
shared through `Arc`; stateless processors share process caches; stateful processors allocate
memory only for active lanes. Shadow tests are pure and have no command, trigger, device, or output
host, so they cannot duplicate effects.
