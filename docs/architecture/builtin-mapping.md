# Built-in Mapping

The built-in Mapping is the file-authored `chataigne.mapping@1` Formula in
`apps/chataigne/resources/formulas/builtin/Mapping.json`. It is the sole ordinary
Inputs → Filters → Outputs processor product. Its authored managed regions and
surrounding Formula graph are both executable; a formula name must not choose a
separate evaluator. [The implementation plan](../plan/builtin-mapping-codex-plan.md)
defines the delivery gates, and [the status ledger](../progress/builtin-mapping-status.md)
records evidence.

## Ownership

| Responsibility | Owner |
| --- | --- |
| ANode declarations, signatures, typed value shapes, graph lowering, and reusable kernels | `apps/chataigne/systems/alchemist/src/` |
| Input and command binding, per-processor/context memory, managed execution, and send policy | `apps/chataigne/systems/alchemist/processor/` |
| Backend node materialization and edit transactions | `apps/chataigne/systems/alchemist/integration/` |
| Mapping inspector and preview presentation | `apps/chataigne/ui/src/lib/systems/alchemist/` |
| App-neutral graph, runtime, protocol, and inspector extension contracts | Public Golden crates and packages |

The Formula asset and ordinary backend nodes are the authored source of truth.
Compiled structures may be shared among compatible instances; bindings, buffers,
history, and output-change caches belong to an instance and its processor context.
Converting a Mapping to a custom Formula must materialize the configured authored
semantics, not export a transient compiled plan.

## Stage and binding contract

One source supplies one typed value; multiple sources supply one ordered typed
tuple. Every filter consumes the preceding value and produces the next value.
An elementwise filter can process compatible tuple elements in parallel; a
reduction merges tuple elements into one value; Pack Vec3 can turn X/Y/Z inputs
into one typed Vec3 command argument. Compound values remain whole until an
explicit operation extracts or converts them. Mixed tuples require a declared
compatible operation or a diagnostic; the Mapping does not silently select a
subset. It has no authored channels, channel groups, or independent routing
lanes. Custom Formulas own branching and per-source paths.

Each authored source still has a stable identity for persistence, editing,
diagnostics, and compatible elementwise state. The tuple shape records source
order and types; runtime frames carry separate validity, change, and delivery
state. Reorder changes tuple shape but cannot silently transfer state between
sources. Internal `ChannelLayout` and frame machinery can represent tuple
elements while the implementation is migrated. Runtime-sized collections remain
values and do not change authored tuple arity with sample length.

A stage's executable capability comes from its configured ANode and resolved
signature. Runtime coefficients and condition values update bound slots; tuple
shape and operation changes produce a revisioned structural candidate.
Resource edits update their resource revision and sampler. Compilation occurs away
from ordinary evaluation and publishes only a complete current revision. An
invalid committed revision does not dispatch through an obsolete plan.

Suppression is a delivery state, never a zero, Unit, or removed tuple element. A closed
suppressing gate freezes affected downstream temporal state until reopened.
`Hold last` has no value before its first accepted sample unless an explicit
default was authored. `Output default` delivers its authored value. Trigger
occurrences retain multiplicity and order. Runtime timestamps and label changes
alone do not mark semantic values dirty.

Each output is a command invocation with authored argument bindings to the final
whole value, explicit tuple elements, compatible components, constants, or
existing property/context values. More than one command may read one result.
Validate all local targets and required
arguments before accepting the dispatch batch. Update per-output change caches
only after local acceptance; external IO and retry remain with module runtimes.

## Current baseline and migration boundary

At `b6ac86eb702d703560593c108b599f95545a417c`, the managed ValueSet runner
supports homogeneous elementwise chains and a terminal aggregate/reshape
projection. It retrieves graph outputs through selected debug samples and forces
unchanged evaluation; `ValueSet` crosses a JSON extension boundary. OutputSet
pairs entries and enabled outputs by position. InputSet already derives a stable
key from the authored item, but unresolved sources are omitted from the frame.
These are implementation gaps, not contracts to preserve.

Phase 01 resolves managed graph outputs to compiled slots and reads only initialized
values after evaluation, including when an unchanged node is skipped. Stateless
projection runs reuse an Alchemist scratch frame. Managed ValueSets now pass to
OutputSet as native typed data; the JSON extension codec remains for actual
Formula graph and persistence boundaries. Ordinary managed evaluation uses no
debug samples. It still allocates an active-lane key set, result entries, context
keys, property frames, per-node input/output vectors, and enabled-output lists;
these are measured and reduced in later performance work rather than described
as allocation-free.

Phase 02 introduced `ChannelLayout` and `ValueLaneKey` in the app-owned
Alchemist crate. Its descriptors and `ChannelFrame` can carry the types, authored
source identities, metadata, and runtime validity of a tuple. InputSet retains
declared positions when a source is disabled or unavailable, and explicit backend
schema events resolve dynamic source types without rebuilding on value samples.
This internal representation also supports selections and groups for custom
Formula work. The revised standard Mapping contract still needs a scalar/tuple
shape boundary and whole-value filter semantics; the old Phase 02 validation
alone does not prove them. `MappingValueShape` now reports incomplete, scalar,
or ordered tuple source shapes without introducing authored channels. Input
reconciliation retains a resolved type across reorder when both source identity
and reference are unchanged, and resets it when the source is replaced. Filters
and outputs still need to consume this boundary end to end.

The current value pipeline executes typed frames through a composable stage chain.
Golden parameter declarations resolve input schemas during processor rebuilds;
the host reads live values from its parameter snapshot and marks dependent
processors dirty on source changes. The final frame still passes through the
older positional OutputSet adapter. Phase 06 will replace that adapter with
explicit command argument bindings. Ordinary value samples do not alter stage
layouts.

Phase 03 resolves each Mapping filter against the entire ordered typed value.
The configured ANode capability and signature determine its primary, auxiliary,
and output sockets and state scope. A standard Mapping rejects explicit channel
selection and grouping, and does not silently pass incompatible tuple elements
through a filter. Custom Formula regions retain routed behavior. Their persisted
filter mode defaults to routed for older projects; the built-in Mapping asset
will declare tuple mode in Phase 08. Math's elementwise and tuple-combine
applications and the Sum/Average reductions use the same arithmetic kernel as
their graph nodes.
Elementwise auxiliary sockets are bound as Formula properties; they can read a
constant, a shared reference, or an element-context reference at evaluation time.
An edit to an authored managed input socket updates a compiled binding and its
processor instance without discarding compatible memory. Structural config edits still
rebuild the processor. The backend Mapping palette validates each configured
variant against the whole tuple and the typed stage compiler. Its creation token
carries the validated input count for variable-arity reductions, so three
operands materialize three input sockets. Trigger and custom Formula routed
choices continue using their respective compiler checks. A
structural change to managed regions refreshes the affected palette and emits
one reusable creatable-items event; ordinary input samples and runtime socket
edits do not recompute it. The UI applies that event to its graph store.

Phase 04 compiles every enabled Mapping filter against the entire typed scalar
or ordered tuple. Each stage's declared input/output sockets determine its next
shape, so Remap → Sum → Smooth and elementwise Math → Pack Vec3 use the same
linear stage runner. Tuple mode rejects empty input and implicit subset
pass-through. Routed mode remains available inside custom Formulas. The app
runtime keeps a bounded stage specialization cache: equivalent instances share
compiled Alchemist graphs, while output frames, result buffers, socket bindings,
and lane memory remain instance-owned. Authored filter previews map cached graph
nodes back to each instance's node identity.

For a Formula with an authored graph, the managed InputSet, FilterPipeline, and
OutputSet must declare typed ValueSet sockets on graph nodes. The processor
reuses the catalog's compiled graph, substitutes the managed region evaluator at
those nodes, and runs all surrounding authored operations in graph order. It
rejects missing or disconnected boundaries instead of evaluating a reduced
sidecar. The ValueSet extension codec is still used at this actual graph
boundary; the graph-free Mapping path keeps native typed frames. Trigger
pipelines with authored graph nodes currently diagnose an unsupported boundary
rather than silently skipping that graph. Phase 05 completes typed flow and
temporal behavior; Phase 06 replaces the positional OutputSet adapter with
explicit command argument bindings.

Existing projects, Action and custom Formulas, processor contexts, state-machine
truth, module commands, script control, and undo/redo remain product contracts.
Changed persisted filter or output semantics need narrow typed migrations. In
particular, a historical default-substituting gate must not silently become a
suppressing gate. The built-in catalog identity stays stable while its file and
processor records evolve.

## Measurement fixture

The pre-change functional reference is the locked Rust test set for
`chataigne_alchemist`, `chataigne_processor`, `chataigne_condition`, and
`chataigne_state_machine`, plus the root UI checks. The existing
`crates/golden_core/engine/benches/baseline.json` is explicitly unqualified for
wall-clock comparison and does not measure Mapping. The pre-change managed-runner
fixture is `apps/chataigne/systems/alchemist/processor/benches/mapping_baseline.rs`.
It measures 1/8/32 float channels with one Remap and 8/32 channels with eight
alternating Remap/Clamp stages after warmup. On an Intel Core Ultra 9 275HX,
Windows x64, Rust 1.97.0 `bench` profile, Criterion 0.8.2 (10 samples,
1-second warmup, 2-second measurement), its center estimates were 1.681,
13.499, 51.411, 33.988, and 141.72 µs respectively. This measures the current
managed runner, including debug-result capture; it is not a full processor or
dispatch benchmark.

With Phase 01's direct slots and capture-free managed path, the same five
fixtures measured 0.713, 5.803, 22.653, 13.116, and 52.838 µs respectively
on that host and profile. These are Criterion center estimates, not p95 or
whole-product dispatch times. The comparison shows shorter managed-runner
latency in the measured numeric cases; it says nothing yet about mixed layouts,
temporal behavior, or large processor counts.

Phase 11 must extend the fixture to 1,000 and 10,000 processors, scalar and mixed
tuples, elementwise work, aggregation, compound construction, sparse changes,
multiple contexts, and bounded temporal history.
Record raw samples, allocations, cache counts, and p50/p95/p99 on matching
hardware before setting regression thresholds. The five-case reference above
must not be used as a threshold for the unmeasured scenarios.
