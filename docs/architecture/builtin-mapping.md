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
| ANode declarations, signatures, typed stage layouts, graph lowering, and reusable kernels | `apps/chataigne/systems/alchemist/src/` |
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

Each stage receives an ordered typed layout and a frame of values with separate
validity, change, and delivery state. Channel identities derive from authored input
items or filter output ports, not source references, labels, or list positions.
Compound values stay in one channel until explicitly extracted. A selection is
`All compatible` or explicit channel identities; missing explicit identities are
errors. Unselected channels pass through. Pack and reduce consume declared order
and insert a replacement at the earliest consumed position; extraction replaces
the source in place. Reorder and duplicate have declared output order and identity.
Runtime-sized collections remain collection-valued.

A stage's executable capability comes from its configured ANode and resolved
signature. Runtime coefficients and condition values update bound slots; layout,
selection, and operation changes produce a revisioned structural candidate.
Resource edits update their resource revision and sampler. Compilation occurs away
from ordinary evaluation and publishes only a complete current revision. An
invalid committed revision does not dispatch through an obsolete plan.

Suppression is a delivery state, never a zero, Unit, or removed channel. A closed
suppressing gate freezes affected downstream temporal state until reopened.
`Hold last` has no value before its first accepted sample unless an explicit
default was authored. `Output default` delivers its authored value. Trigger
occurrences retain multiplicity and order. Runtime timestamps and label changes
alone do not mark semantic values dirty.

Each output is a command invocation with authored argument bindings to channels,
compatible components, constants, or existing property/context values. More than
one command may read one channel. Validate all local targets and required
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

Phase 02 introduces `ChannelLayout` and `ValueLaneKey` in the app-owned
Alchemist crate. A descriptor holds one channel's type, authored identity,
semantic output port, provenance, label, and available range/unit metadata.
Structural revisions track identity, order, type, binding, and port changes;
presentation revisions also track labels and metadata. InputSet exposes the
declared layout even when a source is disabled or unavailable. Explicit backend
source-schema events resolve dynamic source types; ordinary value samples do not
rebuild layouts. `ChannelFrame` in the processor crate stores values, validity,
change, and delivery in separate aligned slots. Compound and array values remain
single slots. A missing or wrong-typed source stays non-dispatching and diagnoses
the authored input without filling a default. Selection resolves stable IDs;
an all-compatible selection with no matches reports an identity-stage status.
Pack/reduce, extraction, duplicate, and reorder layout projections resolve their
identities and positions before runtime evaluation.

The existing homogeneous managed runner still receives `ValueSet` entries from
InputSet while the new frame is available for backend layout queries. Phase 04
will replace that runner's stage shape with the typed layout/frame contract and
remove the positional ValueSet handoff. Source-schema discovery from Golden
parameters and actual Mapping UI queries are Phase 06 and 09 integration work.

Phase 03 extends ANode role capabilities to the configured instance. The
application resolver checks primary, auxiliary, and output sockets against its
signature, resolves stable-channel selections and authored groups, and reports
the state scope. Math uses its graph kernel in both `each` and `combine` modes.
Elementwise auxiliary sockets are bound as Formula properties; they can read a
constant, a shared reference, or a channel-context reference at evaluation time.
An edit to an authored managed input socket updates a compiled binding and its
processor instance without discarding lane memory. Structural config edits still
rebuild the processor. The current managed runner does not yet execute every
resolved layout or multiple-output application; Phase 04 replaces that runner
before such applications are exposed as executable palette choices.

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

Phase 11 must extend the fixture to 1,000 and 10,000 processors, mixed layouts,
aggregation, sparse changes, multiple contexts, and bounded temporal history.
Record raw samples, allocations, cache counts, and p50/p95/p99 on matching
hardware before setting regression thresholds. The five-case reference above
must not be used as a threshold for the unmeasured scenarios.
