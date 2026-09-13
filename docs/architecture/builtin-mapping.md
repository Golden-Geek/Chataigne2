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
OutputSet as native typed data; the extension codec remains at actual Formula
graph sockets and command intents carrying argument overrides. Ordinary managed evaluation uses no
debug samples. It still allocates an active-lane key set, result entries, context
keys, property frames, per-node input/output vectors, and output payloads;
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
and outputs now consume this boundary through the managed tuple pipeline.

The current value pipeline executes typed frames through a composable stage chain.
Golden parameter declarations resolve input schemas during processor rebuilds;
the host reads live values from its parameter snapshot and marks dependent
processors dirty on source changes. The final frame reaches OutputSet through
stable result selectors and explicit per-command argument bindings. Ordinary
value samples do not alter stage layouts. An input may also declare a component
projection; the host still reads the whole source from one coherent snapshot,
and a changed projection changes its provenance so incompatible temporal
history resets.

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
rather than silently skipping that graph.

Phase 05 carries delivery flow separately from typed values. A closed
ConditionGate suppresses its value output, including in an authored Formula
graph, while its `passed` and `blocked` outputs remain available as controls.
The managed stage chain propagates suppressed tuple elements without inventing
a fallback value or advancing downstream temporal memory. `HoldLast` starts
without delivery until it has accepted a value or the user supplied a default;
an explicit Formula connection to the default input also counts. Trigger
regions use the same typed stage chain, so separate trigger occurrences remain
separate command intents.

Each elementwise stage keys history by authored tuple-element identity and
processor context. Rebuilds transfer a compatible stage's retained histories
across source rename or reorder; a changed operation, source provenance, or
upstream temporal dependency resets the affected stage and its successors.
Removed elements and context keys release their histories on structural or
membership changes. Processor lifecycle reset policy clears managed stage and
authored-graph memory; inactivity freezes it. The manager schedules a processor
when any typed stage has due temporal work, even if the outer Formula graph has
none. A stage with only suppressed or missing inputs sleeps until a source or
control changes. Normal stage evaluation still uses Alchemist's data/control
dirty tracking, so an unchanged source can react to a changed gate condition.

The state-machine manager compiles synchronously from a coherent engine
snapshot. It prepares a complete runtime before replacing the processor cache,
keys shared Formula graphs by authored revision and property schema, and
invalidates a failed managed specialization instead of dispatching through an
old chain. No asynchronous compilation completion can race that publication
path. Output commands continue through the engine's queued intent/transaction
path, including commands aimed at another processor's controls.

Phase 06 makes each OutputSet item select the whole scalar result, one stable
tuple element, a component of a compound value, or a constant. A scalar result
fans out to any number of enabled outputs. A multi-element tuple needs explicit
selectors or command argument bindings; output order and enabled state never
assign tuple positions. Each command argument names a stable target parameter
and its own selector. The processor validates selectors against the typed result
layout before dispatch. The host resolves argument target UUIDs against the
current command subtree, coerces each value to the declared parameter kind,
and submits overrides through the existing module/generic command event path.
Unknown targets, duplicate arguments, incompatible types, and changed tuple
shapes diagnose at their owning boundary. An invalid enabled selector prevents
the OutputSet from enqueueing a partial result batch. Standard Mapping has no authored
channels; these selectors describe output bindings only.

An output may request `OnChange` delivery. The host keeps accepted values per
processor, context, output node, and resolved destination. It updates that
cache only after the command event is enqueued, so a rejected event can retry.
Fired trigger arguments always enqueue, including repeated occurrences with
identical values under `OnChange`.
Destination edits, context membership changes, and runtime rebuilds invalidate
the relevant cache. Processor command suppression still blocks dispatch, and
normal state/lifecycle routing remains in the state-machine manager. Parameter
targets use the existing `set_param` path. The host keeps command event order
when change-aware sends meet batched ordinary sends. The output-binding config
and command arguments use app-owned typed extension payloads.

Phase 07 extends the app-owned ANode catalog through the same graph and managed
stage compiler. Numeric reductions consume the full ordered tuple: Sum,
Product, Minimum, Maximum, and ordered Difference retain the common declared
numeric shape; Average and two-input Distance produce Float. An empty reduction
is invalid. Integer arithmetic checks overflow, integer division truncates
toward zero, and division by zero diagnoses. Scalar conversions are explicit;
mixed tuples can use Convert Tuple before a numeric reduction. Pack/Extract
Vec2 and Vec3, Pack/Extract Color, and explicit Vec2/Vec3/Color conversion
handle compound shape changes without routing a subset implicitly. Unsupported
components and non-finite arithmetic diagnose rather than being coerced to
zero or sent to a command.

Curve Remap hosts Golden's editable Curve node under an ANode config; Gradient
Sampler continues to host Golden's Gradient node. The app materializes their
keys or stops into typed Alchemist resource values on each affected snapshot.
Golden's Curve segment cache is thread-safe so compiled samplers can be shared
across processor contexts. Resource edits invalidate the owning ANode's
materialization and publish a newly compiled sampler; no Mapping-specific key
or stop model exists.
Golden creates repeated keys and stops as user items so sparse persistence
retains every authored entry, including multiple entries of the same node type.

Threshold compares a Float against an authored boundary with optional
hysteresis. Timed Delay advances on the evaluation context's elapsed duration
and emits at most one due value per evaluation, in arrival order. A zero delay
passes through immediately. Each stage/context queue is limited to 128 items,
1 MiB total estimated value storage, and 64 KiB for one value; configured
capacity can lower the item limit. Overflow reports a diagnostic and clears
that queue and clock. An inactive processor does not advance its evaluation
clock; disabling a stage removes its memory, and re-enabling it starts fresh.
The existing one-tick delay remains a separate operation.

The ANode declaration registry remains centralized because one trait binds
type identity, signatures, roles, and kernel compilation exhaustively. This
temporarily exceeds the usual source-file length target; Phase 11 owns a
cohesive split after catalog and authoring behavior settle.

Phase 08 gives the bundled Mapping recipe authored InputSet, tuple FilterPipeline,
and OutputSet definitions at its existing graph sockets. Its stable catalog UUID
and read-only built-in status do not depend on the recipe contents. Creating a
processor materializes the three editable regions. The ordinary `CreateUserItem`,
`SetParam`, move, remove, duplicate, and history intents author real ANode trees;
the Input Source and Output Command declarations provide complete default config
trees, while the compatible filter palette provides configured variants. An Input
Source stores a parameter reference and optional component projection. An Output
Command stores a command reference plus a validated JSON encoding of the typed
`OutputBindingConfig`; unknown binding fields and duplicate argument identities
are rejected by the backend parser. The standard Formula exporter round-trips
the built-in's managed definitions. Tree creation and built-in sync use one
batched tree transaction rather than separately adding every declared child.

Previous bundled Mapping versions had no authored managed regions, so no
persisted Mapping input/filter/output item records exist to reinterpret. The
older `sm_*` input/filter/output nodes remain in the state-machine/Action paths;
they are not accepted as managed Mapping items. Custom Formulas with older
managed-region records retain their historical routed filter mode. Gate modes
did have changed semantics: on project open, an unmarked project translates
historical pass, inverse-pass, hold, and trigger-block modes to explicit
default-preserving modes in one transaction, then marks the project root with
`chataigne.condition_gate.semantics.v2`. New projects and newly exported Formulas
carry that marker, so reopening them never reinterprets suppressing gates.
Unknown historical gate modes fail migration with a diagnostic. A shared file
with an unmarked historical gate is rejected with an actionable import error
because loading it cannot rewrite its authored source file safely. Missing or
empty built-in asset directories and a catalog that omits an existing built-in
report errors before removing any project nodes.

Phase 09 exposes the same backend-authored regions in the processor inspector
and Alchemist panel. Input, filter, and output rows retain managed item IDs;
Golden controls edit standard fields and the backend supplies the compatible
item palette. The inspector displays the compiled input/each-filter/result
shape and diagnostics. A typed output binding document lists whole-result,
stable tuple-element, component, and constant selectors, and binds explicit
command parameter IDs. Historical Output Command JSON is translated once on
project open, then the root records `chataigne.output_bindings.authoring.v2`.
Unknown schemas fail without marking the project as migrated; unmarked shared
files diagnose instead of being silently reinterpreted.

The backend identifies the exact built-in Mapping recipe before choosing this
surface, so a custom Formula or Action can still use its graph editor. An open
Mapping inspector leases catalog inspection without value capture or a forced
evaluation. Selecting one filter requests samples for that authored filter and
one processor context only when normal runtime work occurs; preview focus does
not advance temporal state or dispatch commands. A sample has at most 64 tuple
elements and 16 KiB of estimated values. Changing context replaces the lease;
unmount and session changes release capture and history. Runtime capture stays
in the state-machine manager, independent of Formula defaults. Large item lists
render a short visible window while edits use stable backend node IDs.

Phase 10 converts one configured processor through its backend trigger. The
operation validates the exact built-in Mapping identity and its managed items,
copies the authored Formula recipe into the project library with fresh graph
identities, and switches the existing processor reference in one edit group.
The processor keeps its ordered source, filter, and output ANode items,
auxiliary socket settings, command bindings, exposed surface values, and
command nodes. These are processor instance state by design; the custom
Formula owns the reusable graph, managed-region definitions, and properties.
The configured processor and its new Formula reference form the converted
authored document. The operation keeps the existing item tree as its live source
of truth, including edits made after conversion and after project reload.
The processor inspector continues to edit those managed items after conversion,
with backend stage shapes and command targets, while opening the custom Formula
shows its normal graph editor for branching.
No compiled plan is exported as authoring data, and the old and new recipes
never run as two processors.

The copy remaps graph references and managed boundary IDs, removes built-in
and external-file ownership tags, and retains embedded presentation resources.
Copied property nodes carry their original surface identity in an app-owned
tag, so existing processor managers and their nested command identities remain
stable even though the Formula nodes get new UUIDs. Detached trees preserve
nested authored item roles for sparse persistence; read-only managed-region
metadata is explicitly persisted because it defines Formula behavior. During
project load, a missing or temporarily empty Formula definition cannot delete
authored processor regions. Conversion starts managed temporal stages with
fresh history at the next runtime compilation; compatible state migration
between distinct Formula identities is intentionally unsupported.

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
