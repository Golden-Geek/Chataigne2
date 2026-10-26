# Built-in Mapping

Mapping is the shipped `chataigne.mapping@1` Formula recipe in
`apps/chataigne/resources/formulas/builtin/Mapping.json`. It uses the common Alchemist
compiler/runtime; a Formula name never selects a separate evaluator. The corrected ordinary-node
and explicit-compression implementation is complete at its recorded baseline. See the
[historical acceptance evidence](../archive/2026-10-18/progress/builtin-mapping-status.md),
the [one active plan](../plan/Chataigne2_Codex_Implementation_Plan.md), and
[current status](../progress/implementation-status.md) for subsequent work.

## Ownership and authoring

| Responsibility                                                    | Owner                                           |
| ----------------------------------------------------------------- | ----------------------------------------------- |
| ANode signatures, typed values, lowering, kernels                 | `apps/chataigne/systems/alchemist/src/`         |
| Bindings, managed execution, per-context state, send policy       | `apps/chataigne/systems/alchemist/processor/`   |
| Ordinary nodes, backend edits, persistence/conversion integration | `apps/chataigne/systems/alchemist/integration/` |
| Inspector composition and optional previews                       | `apps/chataigne/ui/src/lib/systems/alchemist/`  |
| Generic graph/runtime/protocol/UI extension contracts             | Public Golden packages                          |

Inputs, Filters, Outputs, settings, resources, and command bindings are ordinary authored nodes.
They support normal hierarchy, inspection, scripting/headless edits, warnings, references, and
history. The authored tree is authoritative; there is no hidden workflow mirror or required
preview subscriber. The standard inspector exposes controls; preview supplements it.

Filters and commands can be created before input resolution and inside an invalid chain. The
backend persists unresolved authoring, produces durable diagnostics, suppresses invalid execution,
and recovers when relevant sources/configuration become valid. The catalog does not hide entries
according to the current chain's shape. UI does not repair backend state to permit creation.

## Value and invocation contracts

One source supplies one typed value; multiple sources supply an ordered heterogeneous tuple.
Each filter consumes the preceding whole value and produces the next value. Explicit compatible
elementwise operations, reductions, component projections, and reshape operations handle tuples
and vectors; incompatible values produce diagnostics rather than silently choosing a subset.
Custom Formulas own branching and per-source paths. Runtime collections remain values rather
than changing authored tuple arity.

Sources have stable authored identities. Reordering cannot transfer temporal history to a
different source; incompatible provenance/type changes reset affected state explicitly. Compiled
structures may be shared, while bindings, buffers, history, and accepted-send caches belong to
each instance and processor context. Runtime coefficients and resource revisions update their
own bound data; structural changes publish only a complete current compiled revision.

Suppression is delivery state, not zero, Unit, or removal of a tuple element. A closed suppressing
gate freezes affected downstream temporal state. Hold Last has no value before its first accepted
sample unless a default was authored. Output Default delivers its authored value. Trigger
occurrences retain order and multiplicity; timestamps or labels alone do not make values dirty.

Action and Mapping use the same concrete generic/module command catalog and queued execution.
Mapping owns real command instances with ordinary typed per-argument bindings to whole values,
stable tuple elements, components, constants, or existing property/context values. Explicit Invoke
supports referenced commands where appropriate; references are not the mandatory output model.
Invocation-time overrides never rewrite authored command arguments on every sample.

Validate all local targets and required arguments before admitting a dispatch batch. Update
accepted-send caches after local acceptance; external I/O/retry belongs to module runtimes.
Multiple commands may consume one result. Shared group timing/cancellation and deterministic
context/effect ordering remain part of the common command boundary.

## Conversion, identity, and persistence

Converting Mapping to a custom Formula materializes its configured authored meaning and ordinary
commands/bindings/resources, not an executable-memory dump. Preserve the documented UUID and
external-reference ownership contracts; conversion's deliberate temporal reset is distinct from
compatible runtime edits. Full/sparse persistence, copy/duplicate/import, script/headless edits,
and undo/redo consume the same backend model. Narrow old-format readers are migration machinery,
not a second current authoring path.

## Explicit compression

Compilation and compression are independent. Compression defaults Off and is requested through
explicit Compress/Expand controls. Inspection, save, benchmarks, and release builds never enable
it automatically. Successful compression actually removes the managed Inputs/Filters/Outputs
subtrees; opening a panel does not recreate hidden nodes.

Preparation captures typed, versioned immutable authored source: the exact Formula revision,
managed instance, command definitions, and sparse documents with persistent UUIDs. It contains no
process-local NodeId, prepared device handle, or executable runtime memory. Editable and frozen
representations compile through the same managed Formula runtime and shared command executor.

Generic Set Parameter and Trigger Parameter have UUID-keyed prepared execution using the same
validation/edit implementation as live command nodes. Commands, control modes, schedules, or
known inbound references requiring live descendants produce structured preflight blockers.
Unsupported cases leave the editable representation intact; hidden nodes are not a workaround.
Dynamic script paths cannot be proven absent. Compressed descendants are deliberately unavailable
through such paths until expansion; retain that explicit limitation.

Compression/expansion is one history transaction. Preparation must still match the current source
at commit; stale, invalid, or unsupported preparation leaves representation/status unchanged with
a diagnostic. Read-only frozen data rejects public parameter/script mutation. Expansion reserves
the entire restored identity forest and checks collisions before applying it. Duplicate/import
remaps the entire frozen forest and its internal references coherently before either copy expands.

Settlement checks canonical live compression controls and Golden's public UUID index. It does
not build a whole-project process snapshot to repair archive identities. Uncompressed processors
skip archive decoding; active malformed archives still report their existing errors. Pending
engine edits are applied even when no processor requires repair. Snapshot consumers and live
settlement share one typed archive decoder in the app-owned processor adapter.

Compatible temporal memory and UUID-keyed accepted-send caches survive representation transitions;
unchanged OnChange outputs do not replay. External sources and contexts remain live. Sparse reload,
exact expansion, and undo/redo retain those contracts. Historical measurements and earlier
superseded designs live in the [archive](../archive/2026-10-18/README.md), not this current reference.
