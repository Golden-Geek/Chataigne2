# Chataigne2 — Built-in Mapping corrective implementation plan for Codex

**Revision:** 2026-09-15 — node-first authoring, shared commands, ordinary inspectors, explicit compression
**Inspected branch:** `codex/builtin-mapping`
**Inspected commit:** `a94b9a7e20c9c240c7f29ab1674c90d304289987`
**Repository destination:** `docs/plan/builtin-mapping-codex-plan.md`
**Authoritative progression document:** `docs/progress/builtin-mapping-status.md`

## 1. Directive and precedence

Correct the existing implementation on `codex/builtin-mapping`. Do not restart from the original `main` baseline, discard the branch, or reimplement the execution engine.

This revision supersedes previous instructions and acceptance gates that permit:
- hidden managed-region authoring as the default Mapping experience;
- mandatory Output Command reference adapters instead of owned app command nodes;
- filter creation restricted by the current input shape or chain validity;
- a complete Mapping-specific replacement for ordinary node inspection;
- node-free or non-editable authoring as an implicit/default optimization.

Preserve the useful compiled runtime, typed values/tuples, composable filters, state isolation, direct result slots, command argument validation, and optional preview machinery. Adapt their authoring and invocation boundaries to the corrected contract.

Execute the corrective phases R00–R08 in order. Update progression continuously. Validate, commit, push, and verify remote delivery after every phase. Do not stop after another plan or scaffolding pass. A blocked check or unavailable product environment must be recorded honestly, not treated as a pass.

Read the actual checkout and its applicable `AGENTS.md` files before editing. The inspected commit is a review anchor, not permission to reset newer work. Preserve unrelated modifications and published history.

### 1.1 What the review established

The following are source-review observations, not desktop test results:

| Source | Observation | Consequence for this correction |
|---|---|---|
| `integration/processor/mod.rs` | `processor_managed_regions_tree` creates real `StateProcessorManagedRegions`/`StateProcessorManagedRegion` nodes, but marks the managed root `show_in_inspector_content = false`. The region accepts ANode items. | Expose and integrate existing authored nodes; do not add a second visible tree that mirrors hidden authoritative data. |
| `integration/processor/managed_regions.rs` | Extraction locates the hidden root and ignores children whose type is not `ANODE_NODE_TYPE`. | Merely making managers visible or admitting command nodes is insufficient. Update extraction, ownership, persistence, and lowering together. |
| `integration/processor/palette.rs` | Palette generation compiles/reconciles the current managed workflow and removes applications that fail shape validation. Failure to resolve the layout can return an empty palette. | Decouple creatability from executability in both the menu and backend acceptance rules. |
| `processor/src/alchemist/mod.rs` | `OutputTarget`, displayed as “Output Command,” holds a command reference and a serialized bindings document. | Replace the normal authoring path with actual app command instances and their normal parameters. |
| `integration/managed_nodes/mod.rs` | `OutputsManager` already accepts generic commands, module commands, and output groups. Its contextual catalog initializes a selected module target. `ConsequencesManager` also exists as a separate older item-kind container. | Reuse and consolidate the command path; do not assume a name change alone unifies the actual systems. |
| `ProcessorFormulaInspector.svelte` / `MappingInspector.svelte` | Ordinary children are a fallback; tuple managed workflows use a dedicated inspector and custom region lists driven partly by a preview catalog. | Restore normal child rendering and make preview/diagnostics optional supplements. |
| Progression document | Phases 00–10 are recorded complete under the former contract, while Phase 11 and interactive/product checks remain outstanding. | Preserve historical evidence, reopen affected acceptance, and begin a corrective ledger. Do not report the revised feature as nearly finished solely from old phase counts. |

Paths above are relative to `apps/chataigne/systems/alchemist/`, except UI components under `apps/chataigne/ui/src/lib/systems/alchemist/components/`. The source index at the end gives full paths.

### 1.2 Keep current value semantics in scope

The branch’s current plan uses one linear typed value path: one input supplies one value; multiple inputs supply an ordered heterogeneous tuple. Keep that contract, including supported elementwise operations, reductions, packing, extraction, and conversion.

This correction does not introduce independent user-facing routing lanes, channel-selection matrices, or a second graph editor inside Mapping. Arbitrary branching still belongs to a custom Formula. Stable tuple-element identities and processor context identities remain internal requirements for correct binding and temporal state.

## 2. Non-negotiable product contract

### 2.1 Normal nodes are the default

A newly created Mapping must expose its authored structure directly through the normal engine hierarchy:

```text
Mapping
├── ordinary processor controls
├── Inputs
│   ├── Input Source
│   │   ├── Source
│   │   └── Projection
│   └── Input Source …
├── Filters
│   ├── Remap
│   │   ├── In Min / In Max
│   │   └── Out Min / Out Max
│   ├── Math
│   │   ├── Operation / Application
│   │   └── ordinary operand/binding controls
│   └── Smooth …
├── Outputs
│   ├── an actual app-provided command
│   │   ├── its normal target/configuration
│   │   └── its normal argument parameters and bindings
│   └── another command or supported command group
└── Compression: Off
```

Names illustrate the hierarchy; use the existing declaration conventions and backend-generated labels.

Inputs, Filters, and Outputs are real, visible manager nodes. Each item is a normal node with engine identity, parentage, metadata, enabled state, user permissions, persistence, and ordinary parameter children. Nested conditions, curve keys, gradient stops, and binding settings must be normally addressable too.

The outliner, inspector, reference browser, scripts, remote clients, copy/paste, and undo/redo must agree on the same structure. Do not flatten it only in Svelte while leaving a different hidden editing hierarchy authoritative.

Normal nodes do not require bespoke handwritten Rust evaluators for every filter. A registry-driven ANode-backed node is acceptable when it genuinely participates in the ordinary engine tree and materializes its settings through normal parameter contracts.

Manager labels and ownership come from Formula surface declarations and stable role/region identities, not comparisons against `"Mapping"`, `"Inputs"`, or display labels.

### 2.2 One authoritative authored representation

In editable mode, the live authored subtree is the only mutable source of truth. Compiled plans, DTOs, inspector summaries, and value previews are derived views.

Do not maintain a hidden workflow document and a second editable node tree synchronized by change listeners. A serialization snapshot is permitted at persistence/export boundaries; it is not a parallel editable model.

Preserve stable authored IDs when moving existing items into their proper managers. Use batched detached subtree construction and transactional edits, not repeated add-child/retry loops. Materialization must not depend on opening a panel or requesting preview data.

Reference selection must find actual input/filter/command parameters by ordinary hierarchy traversal. A JSON string containing many individually meaningful settings does not satisfy “everything is controllable.”

### 2.3 Compilation is not compression

Both editable and compressed Mappings use the same optimized compiled semantics.

Editable mode already benefits from shared executable plans, runtime slots, sparse state, and dependency-driven execution. Normal parameter nodes must not each poll or run their own copy of a filter calculation.

Compression additionally removes the instantiated workflow subtree and locks its authoring. It is an explicit production tradeoff, never a prerequisite for good ordinary performance.

Do not auto-compress because of release builds, save/load, processor count, idle state, hidden panels, or benchmark mode. Fresh Mappings default to editable nodes; missing persisted representation metadata defaults to editable nodes.

## 3. Shared command system for consequences and outputs

### 3.1 Command instances, not mandatory command references

“Consequence” and “Output” describe where/how a command is invoked; they are not separate command implementations.

Use one app-owned command catalog/factory, concrete command-node types, argument schema, target-resolution contract, invocation preparation, and dispatch path. Reuse the existing generic/module command infrastructure and output-group scheduling where applicable.

The visible Mapping Outputs manager must offer the same applicable app/module commands as the Action/consequence path. Selecting a command creates that command under Outputs, with its normal settings and module initialization.

A module target, device target, or parameter target required by the selected command is legitimate. A mandatory extra reference to a command node the user must first create elsewhere is not the default Mapping workflow.

Do not “fix” this by renaming `OutputTarget` to Command, relabeling its reference field, embedding a command picker in the old adapter, or displaying a concrete command while still treating a hidden reference wrapper as the editable source of truth.

Trace the actual Action asset and its materialized command manager before consolidating. `ConsequencesManager` and `OutputsManager` currently have different acceptance paths; unify their shared command responsibilities without accidentally preserving a separate legacy command registry as the implementation of one of them.

### 3.2 Invocation policy is separate from command definition

An Action invokes commands on an event/condition transition according to its lifecycle and scheduling policy. A Mapping invokes commands when its filtered value is eligible for delivery according to Mapping’s send policy.

Both prepare a shared invocation with:
- command identity/type and version;
- resolved target and context;
- typed argument values/overrides;
- occurrence identity and logical time;
- scheduling, cancellation, and dispatch-acceptance information where applicable.

Reuse the same execution behavior for a given command. Do not duplicate OSC/MIDI/generic parameter-write implementations in Mapping. Do not bypass the queued engine command boundary for performance.

Preserve output groups, delay/stagger/cancellation, enabled states, ordering, diagnostics, reconnect ownership, and batch behavior. Do not indiscriminately impose Action timing defaults on continuous Mapping delivery; keep the caller’s policy explicit.

### 3.3 Bind ordinary command arguments to invocation values

Use normal command parameter/control surfaces to express:
- the command’s authored value or existing control binding;
- the whole incoming Mapping value;
- an explicit stable tuple element or supported component projection;
- supported context/property/reference sources.

The processed value is invocation data. It must not be written into authored command parameters on every sample. Prepare typed overrides/slots for the invocation so state, undo history, other invocations, and context lanes are not polluted.

Store binding choices as typed, individually addressable backend parameters or established parameter control specifications. Reuse a suitable existing binding contract; extend a public domain-neutral Golden contract only where genuinely reusable. Keep Chataigne-specific invocation policy in Chataigne.

Retire the Mapping-only opaque `bindings` JSON parameter as the normal editing interface. Reuse useful parsing/validation logic through the shared typed model, with a narrow persisted-data migration.

When a command explicitly declares one primary value argument, the backend may initialize it to “Incoming value,” including while input shape is unresolved. Never select the first argument merely because it happens to have a compatible type. Ambiguous choices stay visible and configurable.

Creating an output command must not require a valid Mapping input first. Incomplete bindings are an authoring state with warnings, not a reason to hide commands or reject creation.

### 3.4 Preserve correctness

Multiple outputs can consume the same value. One command can bind several tuple elements. Keep vectors/colors typed unless a binding explicitly projects them.

Validate targets and required arguments before admitting the appropriate local batch. Suppression is normal flow; malformed binding is a diagnostic. Update change-only send state only on local acceptance. Preserve trigger occurrence multiplicity even when payloads compare equal.

Module IO/reconnect remains outside pure formula evaluation. Do not claim atomic external-device delivery from local batch validation.

Copying a command between Action and Mapping preserves its concrete command schema and authored settings. A copied incoming-value binding that has no invocation value in its new context remains visible with a warning; do not silently rewrite it to zero.

## 4. Creation, validation, and ordinary inspection

### 4.1 All registered filters are creatable at any time

Expose every installed, registered filter-capable declaration/application in the standard Filters creation menu, independent of:
- zero or unresolved inputs;
- current type/tuple arity;
- an incompatible preceding filter;
- invalid output bindings;
- disabled/unavailable sources;
- creation before an input has ever been assigned.

This means all supported filters, not arbitrary non-filter graph nodes. Registry membership and genuine permissions remain legitimate creation checks.

Split the contracts:

```text
Creatability:
    registered filter capability + valid authored request + permissions

Applicability/executability:
    current configuration + upstream schema + binding/type validation
```

Remove current-layout membership from `user_container_accepts_item`, not just from the UI menu. Headless creation, paste, duplicate, undo, and scripting must accept the same incomplete authoring operations.

Do not compile the Mapping merely to discover its creation palette. Cache static declaration metadata by registry revision; compute compatibility diagnostics separately.

All legitimate entries stay enabled. Optional compatibility text/ranking must not hide or disable an incompatible filter.

### 4.2 Unresolved authoring is valid persisted data

Allow unresolved type variables, unresolved argument references, and insufficient tuple arity in the authored model. Such a document is structurally valid and saveable even when it has no executable plan.

Create each filter with declaration-owned defaults and normal controls. Do not bake a source-count-dependent `/inputs/N` choice into its identity or silently rewrite authored arity after a later input edit.

Where a filter already supports “all tuple operands,” keep that as a declared semantic mode. Fixed-arity operations such as Pack Vec3 can be created with zero sources and display a “requires three numeric operands” warning.

Revalidate after relevant source/schema, filter, binding, resource, or structural changes. Adding compatible inputs later must resolve the warning and activate a valid plan without deleting/recreating the filter.

No incompatible operation may run with fabricated defaults, silently skip itself, or continue sending through an obsolete valid plan. An invalid required stage blocks affected downstream dispatch. An explicitly disabled stage follows its documented bypass semantics, with downstream revalidation.

### 4.3 Use normal node warnings

Publish durable, backend-owned warning state on the exact offending input/filter/command node and a concise aggregate on its manager/processor.

Distinguish:
- awaiting source/schema;
- incompatible type or tuple arity;
- unresolved command target/argument;
- invalid configuration;
- upstream-blocked execution;
- ordinary gate suppression;
- compressed/locked status.

Compatibility problems are visible warnings in the authoring UI even when their execution effect is to block dispatch. Do not discard severity/context by turning everything into one generic runtime error.

Diagnostics need stable identity, node attribution, a useful explanation, and clear resolution conditions. Avoid duplicating the same upstream cause on every downstream node and do not publish unchanged diagnostics every tick.

Warnings must exist without preview capture or an open Mapping inspector.

### 4.4 Restore standard inspector composition

The processor inspector must render ordinary children by default. Inputs, Filters, Outputs, their items, and parameters use the existing Golden inspector/outliner/creation/editing infrastructure.

Remove the full editing replacement in `MappingInspector.svelte`/`MappingRegionList.svelte` once their useful pieces have been moved to appropriate public supplements. Do not keep the same bespoke editing experience under a different filename.

Allowed additions are small and compositional: value chips, before/after type summaries, a validity badge, a context selector, an ordinary command-argument binding control, a resource editor, or the compression control/status. Preserve normal headers, default child content, context menus, and selection.

Extract genuinely generic improvements into public Golden UI extension points. Keep app-specific registrations in Chataigne. Use Svelte 5 runes only and Rust-generated contracts.

Do not require the state-machine preview catalog to render, select, or edit the tree. Preview feeds provide runtime samples, not authoritative item discovery. The hierarchy must remain usable when preview is off, a processor is invalid/disabled, or runtime preview data has not arrived.

## 5. Explicit production compression

### 5.1 Representation and scope

Implement a visible per-Mapping “Compression” control, default Off, backed by a typed representation state equivalent to:

```text
EditableNodes
CompressedRuntime
```

A transition may expose pending/failure status, but “Compressed” must mean the transition actually succeeded.

Editable mode has the full ordinary workflow subtree. Compressed mode has no instantiated Inputs/Filters/Outputs workflow subtree, including no hidden duplicate managers, command nodes, curve-key nodes, or sentinel per-item proxies.

The processor identity and a small documented set of public controls/feedback remain normal nodes: lifecycle enable/disable, representation, validity, last error, bounded last output/summary, and useful activity/dispatch counters. Keep this public surface explicit and small.

Compressed workflow content is read-only through every interface, not only disabled in Svelte. Do not allow hidden JSON edits, individual filter changes, or mutation via a stale reference. Editing the workflow requires an explicit return to EditableNodes; never auto-expand on a write.

### 5.2 A reversible authored snapshot, not serialized executable memory

Retain a versioned, immutable canonical authored snapshot sufficient to restore the workflow and rebuild execution after loading:
- formula source/version and managed-region identities;
- input/filter/command definitions, settings, binding specifications, and ordering;
- owned resources and stable semantic identities;
- relevant metadata and schema versions.

This snapshot is archival authored data, not an instantiated second tree. While compressed, it is the sole workflow source. In editable mode, any executable plan/snapshot cache is derived and invalidated normally.

Do not persist native pointers, engine `NodeId`s, locks, prepared device handles, or process-specific compiled objects. Recompile/rebind from the frozen source at the appropriate load/registry boundary.

Decompression restores the saved UUIDs in the same project, reserves/reconciles identities safely, and preserves internal references. Duplicating/importing a compressed Mapping generates a consistent new owned identity map; never expand two copies with colliding child UUIDs.

### 5.3 Dependency preflight is mandatory

Before compression, inventory references and behaviors that rely on the descendant nodes.

Known inbound references from States, other Mappings, scripts’ declared bindings, dashboards, formulas, and remote-exposed controls must not silently break. Reject compression with exact blockers by default when they require internal node identity/addressability.

Live outbound source/property/context/resource bindings must remain live in the compressed plan. Compression freezes authoring, not incoming runtime values or the rest of the application.

Internal references among the removed descendants must lower to stable local slots/resources where semantically supported. Reject a dependency requiring a real node callback/address rather than secretly keeping its target alive.

Arbitrary scripts or remote clients can construct paths dynamically, so static inspection cannot prove the absence of every future lookup. State this limitation in the preflight result and document the explicit loss of internal addressability. Do not claim perfect dependency detection. Known blockers still cannot be ignored silently.

Do not auto-promote controls to a new top-level interface or auto-freeze live external values. A future explicit promoted-property feature is outside this correction unless already supported by the normal Formula model.

### 5.4 Shared command lowering is a prerequisite

Current command dispatch includes engine-node targets. Removing command nodes without changing that boundary can leave dangling IDs or no executable target.

Introduce/reuse a shared command preparation/lowering capability that can prepare the same invocation from either:
1. a live normal command node; or
2. a frozen command definition with runtime bindings.

Both paths must use the same command execution implementation, target resolution, validation, scheduling, cancellation, and IO ownership.

Do not create a Mapping-only command evaluator, synthesize temporary engine nodes on each evaluation, or retain hidden command nodes solely to satisfy node-target dispatch.

Commands or resource behaviors that cannot run without an authored node must explicitly report that compression is unsupported. Keep them fully usable in editable mode. Required built-in command scenarios must have tested compression support before this feature is declared complete; unsupported extension types must fail compression cleanly.

Pending delayed/staggered invocations must be migrated without replay/loss through stable invocation descriptors, or compression must be rejected until they drain. Never remove their target nodes blindly.

### 5.5 Atomic transitions and state continuity

Compression is one backend-owned operation available consistently to UI, scripting, and headless callers. A plain boolean write must not independently hide/delete nodes before preparation succeeds.

Prepare a candidate from an exact authored revision; validate dependencies, bindings, and command lowering; prepare compatible memory/scheduler transfer; then commit the representation and subtree change at one engine boundary.

Reject stale prepared candidates. If any step fails, keep the original editable tree and execution intact, expose the failure, and leave the actual representation Off.

Use the same compiled plan semantics. Preserve compatible temporal state, processor contexts, trigger occurrence progress, and output-send caches; do not replay outputs solely because representation changed. If a specific state/schedule cannot be transferred safely, reject the transition rather than silently reset it.

Decompression follows the inverse prepared transaction. Do not run node-backed and compressed instances simultaneously.

Mode changes participate in ordinary persistence and undo/redo. Undo/redo restores the authored representation at the current runtime boundary; it must not rewind historical external effects, replay old triggers, or resurrect obsolete pending invocations.

### 5.6 Feedback without resurrecting the subtree

Keep useful status, current/last output, errors, and bounded preview by stable archived stage identity where supported. Do not recreate child nodes when opening an inspector.

Runtime diagnostics can retain archived item labels/type/provenance without claiming those items are live referenceable nodes. “Expand to edit” should be an ordinary explicit action.

External source loss, module reconnect, context membership changes, and schema/registry changes still receive normal handling. A newly incompatible frozen definition becomes invalid with feedback; it does not silently change the frozen workflow or auto-expand.

Measure the actual benefit. Compression is expected to change authored-node memory/lifecycle/snapshot costs; it does not justify claiming an evaluation-speed improvement without measurements.

## 6. Ownership and implementation boundaries

| Area | Required responsibility |
|---|---|
| Alchemist core and processor crates | Keep shared ANode operations, typed tuples/layouts, compiler, result slots, state, preview contracts, and caller-independent command preparation boundaries. |
| `integration/processor/` | Materialize visible managers; resolve Formula role ownership; derive runtime models from normal nodes or frozen snapshots; publish representation transitions. |
| `integration/managed_nodes/` | Consolidate ordinary Inputs/Filters/command-container capabilities with existing Action behavior; avoid parallel legacy factories. |
| App command modules and command dispatch | One command registry/schema/execution path and explicit prepared-invocation support for both representations. |
| App-owned Alchemist UI | Register small inspector supplements; remove Mapping-specific hierarchy/editing replacement. |
| Golden packages | Only app-neutral node/edit/protocol/binding/inspector capabilities, through public interfaces. |
| Persistence | Versioned authored forms, narrow migrations, identity remapping, atomic snapshot restore. |

Start from the existing public APIs. Do not add another crate or abstraction merely to rename concepts. Do not put Alchemist, Mapping-specific command rules, or product labels in Golden.

Keep `Mapping.json` as a shipped Formula asset, not a Rust branch by product ID. A custom Formula containing managed regions plus extra graph operations must still execute all authored semantics.

## 7. Mandatory progression, commit, and push protocol

### 7.1 Update the existing progression document; do not erase history

Keep `docs/progress/builtin-mapping-status.md` authoritative. Add:
- correction revision/date and review baseline;
- overall corrected status, initially IN_PROGRESS;
- active corrective phase;
- the five product requirements and compression safety requirements;
- a corrective phase ledger R00–R08;
- acceptance ledger N01–N24 below;
- actual validation environment, tested revision, evidence, and outstanding checks;
- last verified implementation commit, remote, observed ref/OID, and timestamp;
- blockers and next concrete action.

Retain original phases 00–11 and M01–M20 as historical evidence. Mark affected conclusions as superseded/requiring requalification under the new contract. In particular, old shape-filtered palette and custom-inspector gates are not new-contract passes.

Original phases 03, 06, 08, 09, 10, and 11 need explicit reassessment. Useful earlier runtime tests remain regression evidence, not proof that the revised hierarchy, commands, or compression are complete.

Use separate implementation, validation, delivery, and CI states. Do not turn “implemented,” SSR-only, pending CI, or environment-blocked into complete.

```text
Phase: NOT_STARTED | IN_PROGRESS | VALIDATION_PENDING |
       DELIVERY_PENDING | BLOCKED | COMPLETE

Implementation: NOT_STARTED | IN_PROGRESS | IMPLEMENTED | BLOCKED
Validation: NOT_RUN | RUNNING | PASSED | FAILED | ENVIRONMENT_BLOCKED
Delivery: NOT_COMMITTED | COMMITTED | PUSH_PENDING | PUSH_VERIFIED
CI: NOT_RUN | PENDING | PASSED | FAILED | NOT_APPLICABLE_WITH_REASON
```

For each phase, record changes, rejected approaches, exact commands/results, product checks, tests superseded and why, implementation SHA, and remaining limitations. No unsupported completion percentages.

### 7.2 Commit and push after every phase

Continue on the approved `codex/builtin-mapping` branch. Verify the actual branch/upstream/remote and preserve user changes. Do not reset, force-push, rewrite published commits, stage unrelated files, alter protections, or weaken checks.

After implementing a phase:
1. Run its acceptance and required regression checks.
2. Update the progression document with actual outcomes and pending delivery.
3. Commit a coherent implementation checkpoint, including tests, generated files, and applicable docs.
4. Push it immediately to the approved remote/branch.
5. Verify remote delivery with a real remote read, not only a local tracking ref.
6. Record that implementation SHA, observed remote evidence, and CI status in a documentation checkpoint; commit and push that checkpoint too.

Suggested subjects:

```text
refactor(mapping): R01 restore node-first hierarchy and inspection
feat(commands): R02 unify Mapping outputs and Action commands
fix(mapping): R03 allow filter creation before input resolution
docs(mapping): record R03 validation and verified delivery
```

Use suitable commit types for other phases.

The status document records the previously verified implementation commit; do not try to predict its own commit hash or future successful push. Report the status-checkpoint SHA after verifying it.

A failed push is a delivery blocker. Keep the local commit and document the failure; do not accumulate several unpushed “complete” phases. Required CI must pass before closure. Preserve pre-existing CI failures as separate evidence, not a blanket excuse for new failures.

## 8. Corrective implementation phases

### R00 — Reopen acceptance and establish the correction baseline

**Work:** Read the actual branch and instructions. Replace the repository plan with this revision, reconcile the progression document, and inventory where the current nodes, authoring data, command factories, inspectors, validation, and runtime IDs actually live.

Trace a real built-in Action from asset to command creation and dispatch. Capture the current Mapping subtree and its hidden flags through backend snapshots. Identify existing persisted fixtures and references requiring migration.

Classify existing work as keep, adapt, or remove. Add characterization tests for preserved runtime behavior and establish the new tests’ ownership. Do not commit an intentionally failing mainline test suite; introduce new executable tests alongside each correction.

**Exit:** The revised plan/progression are committed and pushed; old acceptance is not misrepresented; the agent has an exact implementation map and runnable baseline checks.

### R01 — Restore normal hierarchy and default child inspection

**Work:** Materialize Inputs/Filters/Outputs as visible, ordinary managers directly in the processor’s authored surface. Reuse existing nodes where suitable and preserve their UUIDs. Remove the hidden managed-root dependency from factories, reconciliation, extraction, conversion, and persistence for this representation.

Create inputs and filters as ordinary engine items with typed child controls. Ensure curves/gradients/conditions keep their normal descendant structure.

Restore standard processor child rendering and normal selection/context-menu/editing behavior. Move essential feedback into small supplements so removing the bespoke editor does not remove diagnostics.

Introduce the narrow structural migration in this phase, not at final cleanup. Do not make a visible shadow copy. Keep old output adapters only as explicitly tracked transitional data until R02, not as an accepted final command design.

**Exit:** A new Mapping and migrated existing Mapping expose real manager/item/parameter nodes to outliner, inspector, reference lookup, scripting, and headless APIs. Editing does not require preview/catalog data. UUID and persistence tests pass. Commit and push.

### R02 — Unify command authoring and invocation

**Work:** Consolidate the command catalog/factory, schema, context initialization, and invocation preparation used by Action/consequences and Mapping.

Outputs creates real generic/module command nodes and supported groups. Remove the default OutputTarget reference-adapter palette. Update managed extraction so it deliberately lowers command nodes instead of silently skipping every non-ANode child.

Represent argument bindings through ordinary typed command controls. Keep invocation values separate from authored parameters. Preserve common target validation, command scheduling, repeated events, grouping, and accepted-send caches.

Migrate old adapters where semantics can be proved equivalent. Do not steal or delete a command referenced from another Action/module. Do not silently snapshot a shared command and thereby lose intended live shared configuration. For ambiguous cases, preserve the source data with an explicit migration diagnostic and resolution path; no hidden compatibility evaluator or silent destructive conversion.

An existing standard “invoke another command” capability may remain an explicit advanced command where appropriate, but must not become the required creation path again.

**Exit:** The same selected command is a concrete owned child in Action and Mapping; a Mapping can create/configure outputs before inputs; a meaningful target and incoming-value binding produce the expected effect through the shared runtime. New-registry command tests work in both contexts without Mapping-specific registration. Commit and push.

### R03 — Decouple creation from current type validity

**Work:** Replace compile-dependent palette construction with registry-driven filter enumeration. Remove shape-based rejection in container acceptance, paste, duplicate, and public creation intents.

Allow incomplete but structurally valid filter instances. Separate stable creation identity from current input count. Publish ordinary per-node validation warnings and revalidate the affected chain when prerequisites change.

Remove/rewrite tests that require incompatible filters to be absent; replace them with creation-success plus warning/dispatch-suppression assertions.

**Exit:** Remap, Smooth, Math, packing/extraction, and every supported filter application can be created with zero inputs and after an invalid stage. They survive reload/undo and become executable when compatible inputs/configuration appear. Wrong data never dispatches. Commit and push.

### R04 — Complete normal controls, warnings, and preview supplements

**Work:** Audit every authored setting and binding for ordinary addressability, including nested resources. Remove leftover JSON-only control islands and Mapping-specific edit protocols where ordinary node intents suffice.

Reuse normal parameter editors, binding controls, warning presentation, and header/row extension points. Keep preview demand bounded and optional; subscriptions must release on panel/node/context changes. Do not use a preview DTO as an item inventory.

Ensure live external edits invalidate only relevant runtime settings/resources/structure. Node existence must not introduce per-parameter polling or repeated plan compilation.

Delete the obsolete full inspector/list/edit-state components after migrating useful generic parts and consumers. Update generated contracts and tests in the same change.

**Exit:** UI, State, Mapping, script, and headless edits reach the same controls; standard node creation/editing works with preview fully disabled; warnings persist without subscribers; no parallel Mapping selection/undo system remains. Commit and push.

### R05 — Requalify persistence, conversion, and product behavior

**Work:** Validate new hierarchy and command migrations together. Update configured-Mapping-to-custom-Formula conversion to preserve actual command nodes, live binding semantics, authored identity relationships, resources, and extra graph operations.

Test sparse/full persistence, duplicate, cross-context command copy, undo/redo, and structural edits during execution. Audit external references before and after migration. Preserve unrelated modules, panels, Action behavior, contexts, and the global statechart.

Use the ordinary inspector for the resulting custom Formula’s managed nodes; do not reinstate the removed Mapping editor through the conversion path.

**Exit:** Normal editable Mapping is a complete product checkpoint, not a dependency on future compression. Required node/command/creation/inspection scenarios pass, and unavailable interactive gates are explicitly pending. Commit and push before starting compression.

### R06 — Prepare representation-independent command execution and frozen source

**Work:** Implement the shared command lowering/preparation capability needed when command nodes are absent. Keep execution behavior common to the live-node path. Define versioned immutable frozen source, identity reservation/remapping, and native dependency preflight.

Inventory commands/resources requiring live authored nodes. Implement tested support for required built-in cases and structured unsupported reasons for other capabilities. Cover delayed/staggered pending invocation transfer or explicit preflight rejection.

Add an internal test path evaluating equivalent live-derived and frozen-derived plans, without exposing the compression switch prematurely.

**Exit:** Frozen source can rebuild the required computations/commands without any workflow node IDs; equivalence and dependency-blocker tests pass; no per-tick temporary nodes or separate Mapping command executor exist. Commit and push.

### R07 — Implement the explicit compression switch and atomic transitions

**Work:** Add the visible default-Off control and backend transition service. Prepare and atomically publish compression/decompression, with read-only enforcement, state continuity, failure rollback, identity restoration, and small feedback surface.

Persist compressed mode and frozen source correctly. Test duplicate/import and repeated mode toggling. Ensure inspector preview does not rematerialize the tree.

Compression fails explicitly on invalid workflows, known unsafe inbound references, unsupported node-bound behavior, stale revisions, or untransferable pending schedules. Keep the editable Mapping intact after failure.

**Exit:** The workflow subtree is truly absent only when the user explicitly enabled compression; execution/output behavior remains equivalent; edits are rejected consistently while compressed; expansion restores the authored nodes and references; transitions do not replay effects. Commit and push.

### R08 — Final qualification, cleanup, and evidence

**Work:** Run all N01–N24 scenarios plus preserved runtime regressions. Compare full-node and compressed modes on identical workloads. Check new default node counts, snapshot traffic, startup/edit latency, memory, allocations, recompiles, sparse work, dispatch counts, and bounded preview traffic.

Remove obsolete runtime branches/adapters/UI registrations and contradictory architecture documentation. Retain only deliberate, typed persisted-data migration code. Confirm the shipped asset and fresh-project factory default to editable mode.

Validate supported root launch, dev, watch, headless, packaging, and CI workflows. Use approved non-interactive tests and repository tooling; do not synthesize input or take control of the user’s desktop. Record any unavailable mandatory manual/product gate as pending/environment-blocked.

**Exit:** All required new-contract acceptance and delivery gates are satisfied, or the progression document explicitly remains incomplete with exact blockers. Final code and status checkpoints are committed, pushed, and remotely verified. Do not equate benchmark success with product acceptance.

## 9. Acceptance matrix

Initialize all rows as NOT_RUN for the corrected contract. Preserve prior M01–M20 results separately.

| ID | Scenario | Required evidence |
|---|---|---|
| N01 | Fresh Mapping | Real visible Inputs/Filters/Outputs managers exist immediately; compression Off; no preview request required. |
| N02 | Every authored item is a node | Snapshot/outliner/reference lookup finds input, filter, command, binding, and nested resource controls with real identities. |
| N03 | No shadow authoring tree | One edit changes one authoritative node model; no hidden editable workflow copy or duplicate manager content. |
| N04 | Ordinary inspector | Default child renderer and standard context menus/editing work; custom Mapping editor is not required. |
| N05 | Create filters before inputs | All registered supported filter applications create, serialize, and undo with zero inputs. |
| N06 | Create inside an invalid chain | An incompatible upstream stage does not empty/disable the palette or reject valid authoring intents. |
| N07 | Warnings, then recovery | Incompatible input produces node-local warning and no bad dispatch; fixing input clears it without recreating the filter. |
| N08 | Identical headless creation | UI and ordinary headless/script/paste paths enforce the same creatability contract. |
| N09 | Shared command catalog | A test command registered once appears and creates concrete nodes in both Action and Mapping. |
| N10 | Real module command | Selecting a module command initializes its meaningful target, owns its ordinary parameters, and executes without a pre-created command reference. |
| N11 | Shared typed argument binding | Whole value, tuple element, compound projection, and constants use normal controls; invocation does not mutate authored parameter values. |
| N12 | Fan-out, groups, timing | Several commands share a result; group/delay/stagger/cancel and accepted-send rules are preserved; no duplicate effects. |
| N13 | Controllable from everywhere | State, another Mapping, script, and headless client change settings/bindings/key/stop nodes and affect the expected runtime. |
| N14 | Stable identity and state | Reorder/rename/compatible additions preserve references and suitable per-context/element state; incompatible changes reset or diagnose deliberately. |
| N15 | Persistence/migration | Existing data is preserved or explicitly diagnosed; new save/load/duplicate/undo works; external command ownership is not stolen. |
| N16 | Custom Formula conversion | Current configured commands/settings/bindings/resources and graph semantics survive conversion without replay or editor regression. |
| N17 | Compression default and scope | New/defaulted data stays expanded; explicit success removes actual workflow nodes, not merely their presentation. |
| N18 | Dependency preflight | Known incoming references and unsupported live-node behavior block compression with exact reasons; no data loss. |
| N19 | Compressed equivalence | Required filter/command scenarios match expanded execution, including typed arguments, contexts, suppression, and repeated triggers. |
| N20 | Compressed lock and feedback | UI/script/headless edits to frozen content fail; lifecycle controls, validity/output/diagnostics remain useful; preview creates no nodes. |
| N21 | Atomic transition and failure | Failed/stale preparation leaves mode/tree/runtime intact; successful mode change does not double-run, reset compatible history, or replay effects. |
| N22 | Restore and duplicate compressed data | Reload, expand, undo/redo, and duplicate/import preserve correct identity maps and avoid collisions or historic effect replay. |
| N23 | Product regression | Action, ordinary module commands, Formula editor, node inspector/outliner, panels, context/multiplex, and supported launch workflows still work. |
| N24 | Separate performance qualification | Expanded-node and compressed workloads report real counts/costs; hot execution has no mandatory debug/JSON/tree creation or hidden alternative evaluator. |

Negative cases must include no inputs, unresolved references, incompatible tuple arity, missing modules, source-type change while compressed, delayed invocations during transitions, stage errors, runtime writes back into Mapping controls, and incoming references restored after decompression.

The default reference-addressability tests are hard gates. A UI screenshot that looks like a tree is not a substitute for actual engine-node evidence.

## 10. Validation and completion

Discover exact commands from the current manifests, aliases, and CI rather than copying stale command lines blindly. At minimum run the relevant Alchemist/processor/condition/state-machine/app tests, workspace checking, strict Clippy for affected crates/targets, generated-protocol drift checks, Svelte/type checks, UI tests, formatting, and bundled UI build.

Representative commands, subject to the checkout’s supported toolchain/platform setup:

```sh
cargo metadata --no-deps --format-version 1
cargo fmt --all --check
cargo test --locked -p chataigne_alchemist -p chataigne_processor \
  -p chataigne_condition -p chataigne_state_machine
cargo check --locked --workspace
npm run check
npm test
npm run lint
npm run build
git diff --check
```

Also run the app tests, affected Golden crate tests, code generation, and platform build prerequisites through the repository’s supported tooling. Follow `AGENTS.md` formatting requirements for root and Golden Core. A valid backend test run is not proof of a working desktop inspector.

Preserve `cargo run`, `cargo run -- --dev`, the repository’s watch task, and supported headless workflows. Record actual execution results. Do not mark manual/interactive checks complete from SSR, a helper-model test, or successful compilation.

For benchmarks, record hardware/toolchain/revision, workload, node count, contexts, channel/tuple size, filter depth, enabled output commands, preview mode, raw samples, and baseline qualification. Measure cold creation/load separately from steady execution and sparse edits. Include real normal command nodes in the expanded baseline.

Required steady-state invariants remain: no compilation from ordinary value changes; no authored graph reconstruction; no intermediate JSON handoff; no mandatory debug capture; no temporary workflow nodes; no new per-Mapping threads/timers; no avoidable fixed-size numeric per-channel allocation. New full-node authoring must not regress these.

Final completion requires the corrected normal-node product, shared commands, unrestricted filter authoring with warnings, standard inspector integration, explicit safe compression, successful required validation, and verified commit/push checkpoints.

The final handoff must state what changed, exact tested/untested scopes, migrations or unsupported compression capabilities, product evidence, performance evidence, and the last verified remote commits. Never report completion solely because the original phase ledger reached Phase 11.

## Source index for the inspected branch

These are repository source references at `a94b9a7e20c9c240c7f29ab1674c90d304289987`, not claims that this review executed their tests.

- `AGENTS.md`
- `docs/plan/builtin-mapping-codex-plan.md`
- `docs/progress/builtin-mapping-status.md`
- `apps/chataigne/systems/alchemist/integration/processor/mod.rs` — `processor_managed_regions_tree`, `StateProcessorManagedRegion`, container acceptance.
- `apps/chataigne/systems/alchemist/integration/processor/managed_regions.rs` — snapshot extraction and ANode-only item assumption.
- `apps/chataigne/systems/alchemist/integration/processor/palette.rs` — compile/reconcile-dependent palette filtering.
- `apps/chataigne/systems/alchemist/integration/processor/surface.rs` — ordinary property-manager materialization.
- `apps/chataigne/systems/alchemist/integration/managed_nodes/mod.rs` — Conditions, Consequences, Inputs, Filters, Outputs, and contextual command creation.
- `apps/chataigne/systems/alchemist/processor/src/alchemist/mod.rs` — OutputTarget command reference and serialized binding config.
- `apps/chataigne/systems/state_machine/integration/manager/command_dispatch.rs` — node-target dispatch, argument overrides, acceptance caches, and budgets.
- `apps/chataigne/ui/src/lib/systems/alchemist/components/ProcessorFormulaInspector.svelte` — Mapping-specific replacement and default-children fallback.
- `apps/chataigne/ui/src/lib/systems/alchemist/components/MappingInspector.svelte` — bespoke hierarchy/editor and preview-catalog dependency.
- `apps/chataigne/ui/src/lib/systems/alchemist/components/MappingRegionList.svelte` and sibling Mapping editors — registered component family to audit, extract, and retire where superseded.
