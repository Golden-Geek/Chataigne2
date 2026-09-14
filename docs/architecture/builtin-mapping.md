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

## Baseline and completed migration

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
graph sockets and command intents carrying argument overrides. Ordinary managed
evaluation uses no debug samples. The shipped typed stage runner reuses its
property frame and result slots; final result entries, context keys, and
output payloads still allocate. These paths require allocation measurements
before any allocation-free claim.

Phase 02 introduced `ChannelLayout` and `ValueLaneKey` in the app-owned
Alchemist crate. Its descriptors and `ChannelFrame` can carry the types, authored
source identities, metadata, and runtime validity of a tuple. InputSet retains
declared positions when a source is disabled or unavailable, and explicit backend
schema events resolve dynamic source types without rebuilding on value samples.
This internal representation also supports selections and groups for custom
Formula work. `MappingValueShape` reports incomplete, scalar,
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
declares tuple mode. Math's elementwise and tuple-combine
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

On ordinary ticks the manager visits only active processors selected by dirty
source listeners, changed Formula values or overrides, pending temporal work,
or explicit preview/overview demand. Topology and context rebuilds visit all
active processors. The active-order index keeps sparse evaluations in document
order, and the Formula reverse index and temporal set are refreshed as runtime
plans change. An idle tick does not scan the full active processor list merely
to discover that no Mapping needs evaluation.

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

The ANode declaration registry keeps one exhaustive trait implementation for
type identity, roles, and kernel compilation. Configuration fields and type
signatures live in adjacent `config_fields` and `signature` modules so the
registry remains reviewable. The runtime similarly separates context and
preview contracts from numeric operation helpers in `runtime/context` and
`runtime/operations`.

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
wall-clock comparison and does not measure Mapping. The historical pre-change
managed-runner fixture occupied
`apps/chataigne/systems/alchemist/processor/benches/mapping_baseline.rs`.
It measured 1/8/32 float lanes with one Remap and 8/32 lanes with eight
alternating Remap/Clamp stages after warmup. On an Intel Core Ultra 9 275HX,
Windows x64, Rust 1.97.0 `bench` profile, Criterion 0.8.2 (10 samples,
1-second warmup, 2-second measurement), its center estimates were 1.681,
13.499, 51.411, 33.988, and 141.72 µs respectively. That fixture measured an
older lane-oriented evaluator, including debug-result capture. The shipped
Mapping no longer uses it; Phase 11 removed that evaluator.

With Phase 01's direct slots and capture-free managed path, the same five
fixtures measured 0.713, 5.803, 22.653, 13.116, and 52.838 µs respectively
on that host and profile. These are Criterion center estimates, not p95 or
whole-product dispatch times. The comparison shows shorter managed-runner
latency in the measured numeric cases; it says nothing yet about mixed layouts,
temporal behavior, or large processor counts.

The Phase 11 fixture now compiles and evaluates the actual
`ManagedFormulaRuntime` used by Mapping, with capture disabled. The first
numeric run after removing per-stage property-map and temporary result-vector
construction measured 1.262, 8.682, 34.206, 43.403, and 170.50 µs for
1/8/32 sources at one stage and 8/32 sources at eight stages. Batch center estimates
were 1.533 ms for 1,000 single-source single-stage processors, 33.874 ms for
10,000 of those processors, and 74.242 ms for 1,000 processors with eight
sources and eight stages. These are warmed, full-batch evaluation timings on
the same host and profile, not p95 or product dispatch latency. The benchmark
also contains mixed-tuple, Sum, Pack Vec3, and multi-context cases.

The expanded 10-sample run on the same host measured 1.557 ms for 1,000
three-input Sum processors, 1.517 ms for 1,000 three-input Pack Vec3
processors, and 0.894 ms for 1,000 mixed float/bool/string passthrough
processors. Eight contexts through one eight-source, eight-stage processor
took 380.58 µs per complete context batch. The expanded run's numeric
batch center estimates were 1.505 ms, 33.389 ms, and 73.133 ms for the same three
processor-count cases above. The mixed case measures tuple transport without
a numeric stage; invalid mixed-type numeric application is covered by
correctness tests rather than timed as successful work.

An opt-in per-evaluation run collected 500 consecutive warmed batch timings
for each workload, after 16 warmup evaluations, using `Instant` around one
complete `ManagedFormulaRuntime` batch. The observed nearest-rank percentiles
on the same Windows host and `bench` profile were:

| Workload | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| 1,000 processors × one Float × one stage | 0.929 ms | 0.942 ms | 1.146 ms |
| 10,000 processors × one Float × one stage | 24.714 ms | 27.732 ms | 29.254 ms |
| 1,000 processors × eight Floats × eight stages | 58.018 ms | 60.905 ms | 64.035 ms |
| 1,000 processors × three Floats → Sum | 1.254 ms | 1.280 ms | 1.300 ms |
| 1,000 processors × three Floats → Pack Vec3 | 1.259 ms | 1.361 ms | 1.662 ms |
| 1,000 processors × Float/Bool/String passthrough | 0.462 ms | 0.558 ms | 0.566 ms |
| Eight contexts × eight Floats × eight stages | 0.281 ms | 0.302 ms | 0.307 ms |

These observations use the compact-output runtime, direct frame-to-command
path, and one shared stage-specialization cache within each fixture. The
cache held one distinct compiled stage plan for 10,000 equivalent scalar
processors, two for 1,000 eight-stage alternating Remap/Clamp processors,
and none for mixed passthrough with no stage. The benchmark asserts those
cache-entry counts. Earlier values above document intermediate code and
different measurement durations. A separate 10-sample Criterion reference
for the shared-cache scalar batch had a 1.197 ms center estimate, so the
single-evaluation distribution should not be read as Criterion's aggregate
timing.
The individual timings report observed batch tails, including OS scheduling
jitter, rather than percentiles inferred from Criterion's aggregate samples.
To repeat them, set `CHATAIGNE_MAPPING_LATENCY_SAMPLES=500` and run the
`mapping_baseline` bench with the `mapping_runtime_latency_distribution`
filter. The ordinary Criterion cases remain available when that variable is
unset.

The optional `CHATAIGNE_MAPPING_ENFORCE_275HX_BASELINE` guard compares these
seven observed p95 values against the first recorded 500-sample runtime p95
values on the same 275HX host (1.546, 41.880, 89.707, 2.065, 2.110, 1.162,
and 0.440 ms, in table order). These are host-specific managed-runtime upper
bounds, not end-to-end application thresholds or a claim that the cache alone
caused the later timing difference.

The guarded repeat passed with p50/p95/p99 values of 0.940/0.954/1.141 ms
for 1,000 scalar processors, 24.611/28.249/29.345 ms for 10,000 scalar
processors, and 59.262/60.850/63.638 ms for 1,000 eight-source/eight-stage
processors. The three-source Sum, Pack Vec3, and mixed tuple runs had p95
values of 1.290, 1.295, and 0.558 ms; eight contexts had a 0.296 ms p95.

An opt-in allocation report measures one complete warmed batch with the
workspace's `allocation-counter` tool after 16 warmups. Before the reusable
input/change buffers, compact node outputs, and direct frame materialization,
the report counted 16 allocations for one scalar Mapping, 95 for eight sources
with one stage, 363 for 32 sources with one stage, and 1,419 for 32 sources with
eight stages. On the final measured code, every one-processor numeric shape
(one/eight/32 sources and one/eight stages), three-source Sum, Pack Vec3, and
mixed passthrough counted **three allocations, 1,320 bytes, and zero net
retained allocations** per complete evaluation. Their 1,000-processor batches
counted 3,000 allocations and 1,320,000 bytes. The opt-in benchmark asserts
that allocation count scales only with processor count across those measured
shapes; tuple elements and stages add no warmed per-evaluation heap allocation
in this fixture. The remaining constant allocations include output-level work;
this measurement does not attribute each one or establish state-cache bounds.
Run it with `CHATAIGNE_MAPPING_ALLOCATION_REPORT=1` and the
`mapping_runtime_allocation_report` benchmark filter.

An opt-in activity run separately measures a changing source, a live Remap
setting edit, continuous Smooth evaluation after one source change, full
preview capture for one changing element in an eight-source/eight-stage
chain, and eviction of stateful context memories. It collects 500 consecutive
warmed samples on the same host and build profile:

| Managed-runtime activity | p50 | p95 | p99 | Observed volume or state |
| --- | ---: | ---: | ---: | --- |
| One changing source | 1.2 µs | 1.2 µs | 1.3 µs | 500 intents, no previews |
| One live Remap bound edit | 1.2 µs | 1.2 µs | 1.2 µs | 500 intents, no previews; command values alternate |
| One continuous Smooth stage | 0.7 µs | 0.7 µs | 0.7 µs | 500 intents after one source change |
| Preview of eight sources × eight stages | 35.3 µs | 37.7 µs | 86.2 µs | 500 intents, 4,000 preview samples |
| Evict 128 Smooth contexts to eight | 23.9 µs | 26.3 µs | 62.7 µs | Retained state lanes: 128 before, eight after |

The source-change and setting-edit timers include the in-memory snapshot or
binding update plus managed evaluation. The cleanup timer covers eviction
only; context repopulation occurs outside it. The preview case changes one
tuple element each sample and uses full capture. Run this report with
`CHATAIGNE_MAPPING_ACTIVITY_SAMPLES=500` and the
`mapping_runtime_activity_distribution` filter; set
`CHATAIGNE_MAPPING_CACHE_REPORT=1` to print distinct stage-plan counts.

The same optional 275HX guard also bounds activity p95 at 3 µs for each
single-processor source, setting, and Smooth case, and 100 µs for preview
and context cleanup. A guarded repeat passed: measured p95 values were
1.2, 1.2, 0.7, 39.7, and 25.9 µs in table order. The state metric counts
retained lanes, not heap bytes.

A long-horizon variant runs one continuously changing Smooth Mapping for
100,000 logical ticks. It produced 100,000 intents, captured no previews,
reported no diagnostics, and retained one state lane at the end. The last
500 ticks measured 0.8/0.9/0.9 µs p50/p95/p99 in the bench profile. The
optional host guard bounds this late-history p95 at 3 µs. This checks that
the retained lane count and per-tick cost do not grow with elapsed ticks;
it does not measure the lane's heap bytes.
The guarded repeat passed with the same 0.9 µs late-history p95.

An active, full-engine built-in Mapping with one Float source, an SMA Smooth
stage, and one value sink also ran for 100,000 engine ticks after a source
change. All 100,000 lanes evaluated, the sink converged to the source value,
and no Formula catalog/compile, manager-cache rebuild, or preview capture
occurred. Its early 500 ticks measured 6.3/6.7/9.2 µs p50/p95/p99; the final
500 measured 5.9/6.2/6.4 µs. Run the ignored
`mapping_full_engine_temporal_history_distribution` test with standard
`cargo test` to repeat this host-bound path.

The managed-runtime Smooth fixture measures heap ownership while materializing 128
independent context lanes, then retaining eight. The measured insertion
retained 272,416 bytes across 1,923 allocations; pruning released 97,200
bytes across 1,800 allocations and left exactly eight state lanes. These
are net allocator deltas around the two operations, so the remainder also
includes reusable container capacity and unrelated runtime allocations.
They do not describe a per-lane fixed size or a process-wide memory limit.

The activity fixture also alternates prebuilt string and Float-array values
through one graph-free Mapping with previews off. Each sample includes the
source snapshot replacement and complete managed evaluation. A separate
warmed allocation probe covers one replacement and evaluation:

| Value | p50 | p95 | p99 | Allocations / transient bytes | Net retained |
| --- | ---: | ---: | ---: | ---: | ---: |
| 64-byte string | 0.2 µs | 0.3 µs | 0.3 µs | 3 / 1,320 | 0 |
| 64 KiB string | 0.3 µs | 0.3 µs | 0.3 µs | 3 / 1,320 | 0 |
| Eight-Float array | 0.4 µs | 0.4 µs | 0.4 µs | 6 / 2,472 | 0 |
| 1,024-Float array | 11.5 µs | 11.6 µs | 11.8 µs | 6 / 148,776 | 0 |

String storage is allocated before timing; shared `Arc<str>` payloads make
the warmed Mapping cost independent of the measured string length. Array
source replacement and output ownership clone its elements, so transient
bytes and latency grow with array length even though the number of
allocation sites stays fixed. The fixture asserts exact passthrough values,
no debug capture or retained allocations, length-independent string
allocation totals, and optional 275HX p95 guards of 2 µs for strings,
3 µs for the eight-element array, and 30 µs for the 1,024-element array.
These are measured shapes, not a global maximum collection size. Timed
Delay separately enforces a 64 KiB per-value and 1 MiB queue budget.

An opt-in full-engine fixture additionally runs an active built-in Mapping with
a Float source, Remap, one value target, and one queued generic Trigger
command. Each measured sample includes the engine edit or UI intent, two
complete engine ticks, dirty scheduling, Mapping evaluation, and command
delivery. It collects 500 warmed samples per workload on the same Windows
275HX host with the optimized Rust `test` profile:

| Full-engine workload | p50 | p95 | p99 | Observed volume |
| --- | ---: | ---: | ---: | --- |
| Idle tick | 1.1 µs | 1.3 µs | 1.5 µs | No processor candidate visit or debug capture |
| Source change and queued command | 0.701 ms | 0.921 ms | 1.132 ms | 500 evaluated lanes, batches, and command executions |
| Live Remap setting and queued command | 0.420 ms | 0.554 ms | 0.637 ms | 500 evaluated lanes, batches, and command executions |
| Filter reorder and queued command | 2.738 ms | 2.986 ms | 3.122 ms | 500 local processor rematerializations; zero full manager rebuilds and Formula recompiles |
| Source change, queued command, and leased stage preview | 0.458 ms | 0.491 ms | 0.710 ms | 500 lanes, batches, and executions; 2,000 preview samples |

These are observed distributions for one processor and sequential workloads,
not a paired comparison showing that preview reduces latency. The fixture
asserts the reordered chain's output, command volume, unchanged shared
Formula compile count, and no full manager-cache rebuilds. Releasing the UI
preview lease stops subsequent debug capture. Run the ignored test
`mapping_full_engine_latency_distribution` with standard `cargo test`; set
`CHATAIGNE_MAPPING_ENGINE_SAMPLES=500` and optionally
`CHATAIGNE_MAPPING_ENFORCE_275HX_ENGINE_BASELINE=1`. The latter checks p95
upper bounds of 2 µs idle, 1.5 ms source/setting/preview, and 5 ms structural
edit on this recorded host. The guard passed for the distribution above.
On a guarded repeat after the no-full-rebuild assertion was added, p95 was
1.2 µs idle and 0.831/0.539/2.871/0.475 ms for source, setting, structural,
and leased-preview activity respectively; all 500 structural edits still
caused zero full manager rebuilds and Formula recompiles.

The managed-runtime processor-count benchmark above is a separate
execution-level result and does not establish full-engine scaling or UI
render latency. The opt-in `mapping_full_engine_processor_scale_distribution`
fixture constructs active built-in Mappings with a shared Float source and
generic Trigger command, using one detached processor-folder subtree plus
InputSet and OutputSet item trees queued as one forest edit. Each sample
changes the source and runs two full engine ticks. The optimized Rust `test`
profile on the same 275HX host produced:

| Active processors | Idle p95 | Source + command p50/p95/p99 | Snapshot work per sample | Command batches |
| ---: | ---: | ---: | ---: | ---: |
| 128 | 15.7 µs | 1.725 / 2.106 / 2.435 ms | Zero builds | One for 128 executions |
| 256 | 31.9 µs | 4.452 / 5.374 / 5.546 ms | Zero builds | One for 256 executions |
| 1,000 | 172.7 µs | 41.661 / 43.467 / 44.093 ms | Zero builds | Two for 1,000 executions |

Each row covers 100 warmed shared-source samples with previews off. The
fixture asserts one evaluated lane and one command execution per processor
per sample, correct output, no Formula catalog or manager-cache rebuild,
and bounded command batches. The elapsed distribution covers the complete
two-tick delivery path. The previous command implementation requested one
full-tree snapshot per active sample; its p95 was 10.66/24.48/140.53 ms at
128/256/1,000 processors, with snapshot construction averaging
5.42/12.20/54.36 ms respectively. An earlier direct-sibling fixture before the
processor palette manager's snapshot gate built three snapshots per sample
and recorded 27.49 ms p95 at 128 processors. The grouped fixture before
the folder gate also built three snapshots and recorded 26.12 ms p95.
The snapshot-free run still exceeds a smooth frame budget at 1,000 active
processors, so this is a measured scaling limit rather than a passed
large-graph latency gate. The latest 1,000-processor opt-in test took 208
seconds overall, versus 451 seconds before forest insertion and 552 seconds
before combined region insertion. Most of that time remains in processor-group
construction outside the timed source-change samples. This is not a
project-load benchmark.
The focused command lookup asks the engine only for UUIDs referenced by the
current inbox. The engine resolves each UUID against its live index, copies
the current parameter value, and supplies it to the callback. The command
still validates the Trigger type, handles missing/deleted targets as an
operation error, preserves ordered trigger edges, and supports per-execution
target overrides. A full snapshot remains available when another callback
requires one. Before this change, `GOLDEN_PERF_TRACE=1` identified this
command as the sole full-tree snapshot requester in the 128-processor active
path; at 256 processors, a representative 23 ms tick spent about 12 ms
building a 9,909-node snapshot and about 8 ms in other stabilization work.
The trace is diagnostic; the uninstrumented table above is authoritative.

Construction remains a separate bottleneck. A fresh processor with a resolved
project Formula now inserts its Managed Regions root and all Formula-defined
region children as one detached `NodeTree`. The root keeps its declaration ID,
type, presentation, and persistence position; existing roots still reconcile
in place. Before this change, the 128-processor `test-fast` fixture spent
2,891 ms applying the detached processor group and built 2,670 lifecycle
snapshots (2,221 ms; 2,953,036 cloned node records). With the combined
region tree, that stage took 1,851 ms and 1,527 snapshots (1,393 ms;
1,728,756 cloned records). Detached group assembly remained below 1 ms.
Golden now exposes an explicit `AddUserItemTrees` forest edit for independent
item roots under existing parents. It checks every parent and item type before
insertion, appends in request order, runs one shared lifecycle batch, records
one undo transaction, and publishes one atomic UI graph transaction. The
generated node-enum implementation forwards `create_user_item_tree`, so the
app-owned ANode factory's complete config and socket descendants reach this
edit instead of falling back to single-node creation. UI catalog lookup uses
one tree snapshot for the forest. Ordinary individual item edits keep their
existing behavior.

The 128-processor `test-fast` item stage now takes 35 ms and three lifecycle
snapshots (15 ms of snapshot work), versus 2,912 ms and 514 snapshots with
separate edits, or 1,914 ms and 258 snapshots when only the roots were batched.
The optimized 256-processor item stage takes 77 ms and three snapshots versus
12,922 ms and 1,026 snapshots before the forest. At 1,000 processors, it takes
393 ms and three snapshots (154 ms of snapshot work), versus 250.40 seconds
and 4,002 snapshots (144.47 seconds of snapshot work). The 1,000-processor
processor-group stage still takes 157.31 seconds and builds 11,991 lifecycle
snapshots (112.68 seconds of snapshot work). It still creates declared
processor children through callbacks; further construction scaling belongs at
the app-owned detached processor tree and Golden lifecycle boundary. These
are setup measurements, separate from steady-state latency and project load.

The optional sparse-project cycle uses the same authored graph after its 100
warmed source/command samples. It saves through Golden persistence, decodes the
sparse document, runs Chataigne's shipped-Formula sync hook, and prepares the
runtime in the same order as the product host. The 275HX optimized `test`
profile measured:

| Active processors | Live nodes | Sparse JSON | Save | Decode | Formula sync | Runtime prepare | Resumed source + command |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 128 | 5,045 | 4.66 MB | 33 ms | 83 ms | 36 ms | 246 ms | 1 ms / 128 executions |
| 1,000 | 38,181 | 36.35 MB | 226 ms | 759 ms | 326 ms | 2,042 ms | 34 ms / 1,000 executions |

All 388/3,004 authored user-item root UUIDs remain stable at 128/1,000
processors. Sparse reconstruction assigns new IDs to 1,288/10,008 derived
children; the startup hook adds four file/reference/control nodes and 111
bytes to the next sparse save. The fixture checks each authored root and the
source/command/sink identities, then changes the source and requires exactly
one command execution per reloaded processor. The repeated 1,000-processor
run completed new-processor setup in 125.86 seconds and dense delivery at
38.746 ms p95; those timings vary from the earlier row. Project open takes
about 3.13 seconds across decode, Formula sync, and runtime preparation,
so the remaining 125-second construction cost concerns bulk creation of new
processors rather than opening an existing project.

For new processor creation, the app's manager and folder factories now capture
Formula-managed region definitions alongside their palette entries and put
the region tree into each detached processor item. A change to the Formula's
region metadata refreshes that factory cache on the normal engine event path;
invalid or incomplete metadata falls back to live reconciliation. Golden's
existing project decoder still reconstructs persisted sparse items, so this
factory preparation does not change authored identity on reload. At 128
processors, group construction built 1,146 rather than 1,527 lifecycle
snapshots and took 1,614 rather than 1,851 ms in the `test-fast` profile.
The 1,000-processor optimized repeat built 8,994 rather than 11,991
snapshots (93.03 seconds of snapshot work). Group setup took 126.4 seconds
versus 125.9 seconds on the previous repeat. The snapshot reduction is clear;
setup wall time has not yet improved reliably. The same repeat delivered
100,000 exact commands at 35.400 ms p95 with zero steady snapshots and
reopened its sparse project in 3.22 seconds across decode, Formula sync, and
runtime preparation.

Processor palette managers and folders now pass through parameter events
outside their watched Formula region metadata. They still receive structural
events and refresh their detached-tree templates when the metadata changes.
In the shared-source/command fixture, this reduces 100-sample recipient
deliveries from 65,900 to 26,600 at 128 processors and from 502,300 to
201,400 at 1,000 processors. The guarded optimized 1,000-processor p95 fell
from 35.400 to 18.319 ms, with exactly 100,000 command executions and no
steady snapshots. The 128-processor `test-fast` p95 stayed near 1.6 ms; a
guarded optimized 256-processor run measured 3.294 ms p95. At that checkpoint,
the 1,000 case remained above a 16.7 ms frame and still created new processors
in about 125 seconds. These are separate responsiveness and authoring-scale
limits.

The detached processor factory now also captures Formula property surfaces as
small descriptions. It materializes fresh parameter, manager, and nested folder
nodes with the existing declaration IDs, labels, colors, values, and constraints
before insertion. The live property reconciler uses the same description path;
Formula property parameter and structural edits refresh the factory cache. A
32-processor construction diagnostic fell from 282 lifecycle snapshots to 3.
On two guarded optimized 1,000-processor repeats, constructing the processor
group took 2.606 and 2.552 seconds with 3 snapshots, versus roughly 126 seconds
and 8,994 snapshots before property preparation. Exact 100,000-command delivery
and zero steady snapshots still pass. Steady p95 measured 33.685 and 29.087 ms
on those repeats, above the prior 18.319 ms checkpoint; recipient count stayed
at 201,400, so this change has no demonstrated steady routing gain. The sparse
1,000-processor reload still preserved all 3,004 authored item roots and resumed
1,000 command executions after a source change.

The state-machine manager maintains an exact command-plan index and skips
listener reconciliation on idle ticks; the fixture asserts that an idle
or steady dense interval adds no listener reconciliations. Set
`CHATAIGNE_MAPPING_ENFORCE_275HX_SCALE_BASELINE=1` to apply the recorded
host's opt-in p95 regression limits to the 128, 256, or 1,000-processor
fixtures. The guard preserves the measured cost ceiling; it does not
represent a real-time responsiveness target.
