# Statecharts, Conditions, And Processors

Phase 5 makes one runtime path responsible for each concern while keeping the existing Chataigne
authoring and inspection experience.

## Ownership

- `golden_statechart` stores state identity, hierarchy, transitions, active configuration,
  presentation, and revision in one `StatechartGraphDocument`. All edits use graph transactions.
- `golden_statechart_ui` projects that document into `golden_graph_ui`; the Chataigne panel adds
  product policy and edit intents without rebuilding a second graph model.
- `golden_condition` defines app-agnostic Input Value, Input Node, Group, and Script conditions.
  Compilation produces flat instructions, stable observation IDs, a kernel key, and a dense state
  layout. Hosts provide input values and script execution through explicit interfaces.
- `chataigne_processor` owns Processor instances, shared compiled formulas, property overrides,
  context bindings, lane planning, lifecycle, lane memory, `ValueSet`, and managed pipelines.
- `chataigne_state_machine` composes statecharts, processors, arbitration, and protocol DTOs.

## Runtime Path

The Chataigne condition tree is authoring state only. A cache rebuild lowers it once into a compiled
program and direct parameter bindings. Each default or multiplexed lane evaluates that program with
its own migratable `ConditionRuntime`. The current program emits transient edges; a companion
settled program supplies the stable value used after a pulse. Inspector DTOs come from compiled
observations, so preview capture cannot invoke a second condition implementation.

Processor formulas compile once per semantic formula key and are shared by every identical
instance. The lane compiler combines inherited context axes, context-linked properties, and output
axes into a stable execution plan. Stateful memory is keyed by `ContextKey`, preserved for retained
lanes, removed for deleted lanes, and initialized only for new lanes.

Action and Mapping remain the only shipped user-facing formula choices. Both use the same Processor,
condition, context, `ValueSet`, and output-intent path; Mapping is not duplicated into specialized
single-input and multi-input runtimes.

## Scale And Safety

Compiled conditions do not walk editable condition nodes in steady state. Formula kernels are
shared through `Arc`, stateless Processors allocate one process cache, and stateful Processors
allocate memory only for active lanes. The Phase 5 regression fixtures cover P50-L1 and P5-L127 in
the backend and the UI lane projection, alongside the stronger 10,000-stateless-Processor and
1,000-stateful-Processor stress tests.

Condition shadow tests compare pure compiled results with reference comparator outcomes. They have
no command, trigger, device, or output host, so they cannot duplicate product effects.
