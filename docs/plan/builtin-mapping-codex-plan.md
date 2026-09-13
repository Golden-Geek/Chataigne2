# Chataigne2 — Built-in Mapping implementation plan for Codex

## 1. Mission and starting point

Implement the built-in Mapping as an Alchemist Formula with the familiar **Inputs → Filters → Outputs** workflow. Every filter must consume the typed channel layout produced by its predecessor. Packing, extraction, aggregation, conversion, and ordinary value processing must compose freely throughout the chain.

Keep exactly one Mapping entry in the processor creation menu. Multiple inputs, conditions, and multiple outputs are capabilities of that Mapping, not separate processor products. Reserve custom Formulas for genuinely graph-shaped logic, not ordinary linear chains that happen to change types or channel counts.

The reviewed Chataigne2 baseline is `5dd0be6f90d861573b3faa1dd1fe0c447dee0a5a`; `main` was rechecked against that commit on September 12, 2026. The Chataigne 1 comparison baseline is `f8634ae02c3efd1620077d5be3e387f896f5ddbc`. These are reference points, not instructions to reset a newer checkout. [R1]

The source review found existing Formula assets, managed regions, filter capabilities, and lane-aware machinery. It also found terminal-only projection, homogeneous managed filtering, positional output pairing, debug-based result retrieval, and a JSON ValueSet round-trip. Extend and correct the existing implementation; do not build a parallel Mapping engine. Reference paths are listed at the end. [R2–R5]

**Execution directive:** implement the phases in order. Update the progression document throughout the work. Validate, commit, and push after every phase. Do not stop after another design document or scaffolding-only pass. Stop only for a genuine blocker, unsafe repository operation, or failed required gate, and leave an exact recovery point.

## 2. Non-negotiable engineering rules

Read the root `AGENTS.md` and applicable nested instructions before editing. [R6] Follow their ownership, editing, formatting, test-layout, protocol-generation, and tool-use rules.

Preserve the existing product: panels, inspectors, outliner, Formula editor, module integrations, processor behavior, state machine, contexts, multiplexing, scripting, undo/redo, and existing development workflows. No phase may intentionally leave the application uncompilable or replace established UI with a placeholder.

All authored input, filter, selection, grouping, binding, and output configuration must remain accessible through normal backend nodes and parameters. States, other Mappings, scripts, remote control, and the inspector must reach the same backend behavior. Compiled plans are execution representations, not hidden sources of truth.

Implement calculations once, in reusable Alchemist operations. Do not create Mapping-specific copies of Math, Smooth, conversion, gating, or command dispatch. Any necessary Golden extension must be app-neutral and public. Do not introduce `golden_mapping`, move Alchemist into Golden, or import private files across crate boundaries.

Use Svelte 5 runes only, generated Rust-owned DTOs, ordinary backend edit intents, and existing public Golden UI extension points. The UI must not infer domain layouts, generate persistent identities, choose backend defaults, or repair invalid node structures.

Preserve project behavior through narrow, typed, tested migrations when this implementation changes persisted semantics. Do not retain a legacy evaluator or permanent compatibility layer. Do not silently reinterpret an old default-substitution gate as a suppressing gate.

Use focused modules and adjacent `tests/` directories. Keep implementation files within repository size limits. No dependency or toolchain upgrades unrelated to this feature. No desktop-control automation or synthesized input to the user's desktop.

## 3. Mandatory progression and Git protocol

### 3.1 Required documents

Save this implementation plan at:

```text
docs/plan/builtin-mapping-codex-plan.md
```

Create and maintain the single authoritative progression document at:

```text
docs/progress/builtin-mapping-status.md
```

Create it in Phase 00 before implementation. Link it from the appropriate existing documentation index. Do not scatter contradictory status summaries across several files.

Initialize a ledger row for every phase, 00 through 11, and an acceptance row for every scenario, M01 through M20. Update it when a phase starts, after meaningful milestones, when checks run, when a blocker appears, and at every commit/push checkpoint. Read it at the start of every resumed Codex session, then reconcile it against the actual tree and remote before continuing.

### 3.2 Required status structure

Use separate columns so implementation, verification, and delivery cannot be confused:

```markdown
# Built-in Mapping implementation status

## Summary
- Overall: NOT_STARTED | IN_PROGRESS | BLOCKED | COMPLETE
- Baseline commit:
- Working branch and approved remote:
- Active phase:
- Last validated implementation commit:
- Last verified remote implementation commit:
- Current blockers:
- Product checks still outstanding:
- Next concrete action:
- Last updated: ISO-8601 timestamp with timezone

## Phase ledger
| Phase | Status | Implementation | Validation | Delivery | CI | Evidence / blocker |
|---|---|---|---|---|---|---|
| 00 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | |

## Acceptance coverage
| Acceptance ID | Scenario | Owning phase | Test/evidence | Status |
|---|---|---|---|---|

## Validation evidence
| Timestamp | Phase | Tested revision | Command/scenario | Environment | Result | Evidence |
|---|---|---|---|---|---|---|

## Phase reports
### Phase NN
- Changes and affected public boundaries:
- Acceptance gates satisfied:
- Remaining work:
- Exact checks and outcomes:
- Implementation commit:
- Verified remote ref, observed OID, and timestamp:
- CI status and relevant runs:
- Decisions/deviations and rationale:

## Blockers and handoff
- What failed:
- Last known-good checkpoint:
- Reproduction:
- Next action:
```

Phase statuses: `NOT_STARTED`, `IN_PROGRESS`, `VALIDATION_PENDING`, `DELIVERY_PENDING`, `BLOCKED`, `COMPLETE`.

Implementation statuses: `NOT_STARTED`, `IN_PROGRESS`, `IMPLEMENTED`, `BLOCKED`.

Validation statuses: `NOT_RUN`, `RUNNING`, `PASSED`, `FAILED`, `ENVIRONMENT_BLOCKED`.

Delivery statuses: `NOT_COMMITTED`, `COMMITTED`, `PUSH_PENDING`, `PUSH_VERIFIED`. This column describes the implementation commit, not the commit containing the status document itself.

CI statuses: `NOT_RUN`, `PENDING`, `PASSED`, `FAILED`, `NOT_APPLICABLE_WITH_REASON`.

A phase is complete only when its implementation and acceptance checks pass, its implementation commit is verified remotely, its status checkpoint is also pushed, and any CI designated as required for that phase passes. `IMPLEMENTED`, `PENDING`, and `ENVIRONMENT_BLOCKED` do not mean complete. Do not substitute invented completion percentages for evidence.

### 3.3 Branch safety

Inspect the working tree, branch, upstream, and remotes before changing anything. Record pre-existing user modifications and preserve them. Do not stash, discard, overwrite, or stage unrelated work automatically.

Use the task's explicitly designated branch. Otherwise use or create a dedicated `codex/builtin-mapping` branch from the appropriate current baseline, preserving existing user work. Do not push to `main` unless the user explicitly designated it as the implementation branch. Verify that the configured remote is the intended repository.

Never force-push, rewrite published history, disable hooks, bypass protected-branch rules, or weaken CI. Use explicit task-owned paths when staging. If a non-fast-forward push or a dirty-tree conflict prevents safe progress, record the blocker rather than applying a destructive shortcut.

### 3.4 Close every phase with implementation and status checkpoints

**Checkpoint A — implementation:** finish the phase, run its required gates, review the diff, and update the progression document with actual results and `PUSH_PENDING`. Create a coherent phase commit:

```text
feat(mapping): phase NN — <completed capability>
```

Use `refactor`, `test`, or `docs` when more accurate. Include tests, generated changes, relevant documentation, and the progress update. Do not batch several phases into one final commit.

Push immediately to the approved branch. Verify the remote ref with `git ls-remote` or an equivalent remote read; do not rely only on local tracking refs or a successful-looking command message. The observed tip should equal the pushed commit, or demonstrably contain it if an authorized later commit has advanced the branch.

**Checkpoint B — progress publication:** after A is actually verified, record A's SHA, remote evidence, timestamps, and observed CI state. Commit this factual status update:

```text
docs(mapping): record phase NN delivery and validation
```

Push and verify B as well. The status document records the previously verified implementation commit A; it does not attempt to contain its own commit SHA or predict its own future push. Report the verified status-checkpoint SHA in the session handoff. This avoids self-referential commit bookkeeping.

When required CI is pending, show that honestly and do not mark the phase complete or start dependent work that assumes it passed. Publish subsequent CI results in a status checkpoint. If CI fails, fix forward, rerun validation, and push the correction before closure.

If a push fails, retain the local commit, mark delivery blocked, and report the exact failure. Do not claim the phase is delivered, queue several unpushed phases, or alter credentials or protections. A source-control checkpoint of incomplete work must be labeled `WIP/BLOCKED`, never complete.

## 4. Fixed architecture and behavioral contracts

### 4.1 Ownership and Formula authority

Keep ANode contracts, signatures, and compilation under `apps/chataigne/systems/alchemist/src/`; processor/context bindings and managed execution under `processor/`; backend node materialization under `integration/`; and product presentation under the app's Alchemist UI.

`Mapping.json` remains the authored built-in recipe. Its name or ID must never select a hardcoded evaluator. Managed regions are executable Formula constructs with explicit boundaries, not UI hints that authorize bypassing the rest of the Formula graph. A custom Formula with managed regions and additional operations must execute all authored semantics.

An optimized managed-stage plan is permitted only as a lowering of those same constructs. Custom Formulas and Mapping must share operations, flow semantics, state contracts, and effects. Do not clone a complete Formula graph per processor or multiplex lane.

### 4.2 Typed layouts and identity

Separate immutable layout descriptors from mutable runtime frames. Reuse existing suitable types; the conceptual names `PipelineLayout`, `ChannelId`, and `PipelineFrame` are not mandates to duplicate them.

A layout describes ordered channels, their stable identities, individual types, semantic ports, labels, provenance, and available range/unit information. A frame holds values plus separate validity, change, and delivery state. Runtime timestamps or labels must not make an otherwise unchanged value dirty.

Mixed layouts such as `[float, bool, vec3, string]` are supported. A Vec3, Color, or array remains one typed value until explicitly extracted. Zero configured inputs is an incomplete, non-dispatching Mapping with actionable diagnostics; zero configured outputs is a valid non-dispatching authoring state.

Input identity derives from the authored input item, not the source target or list index. Two inputs referring to the same source remain distinct. One-to-one filters preserve identity; aggregate/pack outputs use stable filter/group/output-port identities; extraction derives child identities from source and component; explicit duplicate outputs have authored identities. Reordering never transfers state or rebinds outputs accidentally.

Selections are either `All compatible` or explicit stable channel references. Unselected channels pass through. Explicit incompatible or missing selections diagnose rather than silently retarget. If `All compatible` selects nothing, treat the stage as identity with visible informational status, not a fabricated output.

Pack/reduce operations consume declared ordered selections and insert their replacement outputs at the earliest consumed position. Unconsumed channels retain order. Extract replaces each selected compound at its position. Reorder and duplicate use explicit declared order. Document exceptions for operations whose semantics require another rule.

Runtime-sized collections remain collection-valued channels. Unbounded collection-to-channel expansion is outside this implementation. Fixed-size, explicitly configured extraction/duplication is required; values must not trigger graph rebuilding merely because an array length changes.

### 4.3 Filter applications and live control

A managed filter application resolves an ANode declaration, application mode, selected channels/groups, primary and auxiliary socket bindings, output layout, identity rules, parameter slots, state scope, and scheduling requirements.

Support per-channel transforms, per-channel type conversion, reduction across channels, packing, extraction, reorganization, and whole-stream flow control. Resolve capabilities from the configured application and signature, not only a static cardinality enum.

Math must support both `Apply to each` and `Combine selected`, using the same numerical implementation. Auxiliary operands can use supported constants, properties, references, and context bindings. Define subtraction/division fold order explicitly.

Classify settings as runtime values, structural configuration, resource revisions, or presentation. Runtime coefficients and gate conditions update slots without recompiling. Structural changes revalidate the affected plan. Curve/gradient changes update their existing resources and derived samplers, not the entire Formula graph. A live-bound value is never constant-folded as an immutable literal.

### 4.4 Execution, flow, memory, and edits

Use explicit compiled output slots or a result sink, independent of debug capture. Keep typed data native between managed stages and output binding. Do not serialize intermediate values through JSON or introduce a Chataigne-specific type into a Golden package.

Share immutable executable specializations for compatible structure and types. Keep bindings, caches, buffers, and state instance-local. Specialization keys exclude processor UUID, UI state, labels, and runtime coefficient values; include every executable-shaping dependency. Remap authored IDs to instance-local slots without sacrificing plan sharing or preview attribution.

Infer plans from declarations and binding schemas. If a dynamic source lacks a schema, remain unresolved or resolve through an explicit schema event outside normal evaluation. Do not compile on every value update or retain only a single last-seen shape that thrashes between contexts.

Distinguish unchanged, suppressed, invalid, explicit default, and held values. Suppression never means zero, Unit, or deletion from the layout. `Suppress`, `Hold last`, `Output default`, and trigger gating are separate behaviors.

A suppressing gate blocks affected downstream delivery and freezes affected downstream temporal state by default; upstream stages may continue. Hold/default modes supply actual values and may allow downstream temporal processing. Reductions suppress when a required operand is suppressed; using old operands requires an explicit policy. On reopening, make the current valid sample eligible for delivery without recompilation. Define hold-before-first-sample behavior without manufacturing an implicit zero.

State identity includes processor, required processor-context key, authored filter identity, channel/group identity, and semantic state slot. Processor contexts and Mapping channels are distinct. Reconcile sparse lane pools at structural/context membership changes, not by rebuilding active-key collections every tick.

Preserve compatible state across rename, reorder, and unrelated additions. A type change, source replacement, or change in an upstream temporal dependency requires deliberate per-operation reset/migration rules; stable identity alone is not proof that old state remains valid. Reuse processor lifecycle policies. The statechart retains one global active-state truth.

Prepare revisioned candidate plans away from the hot evaluation path and publish at an engine boundary. Reject stale compilation completions. Do not execute half-applied edits. If the committed authored revision is invalid, suppress invalid dispatch and expose diagnostics rather than silently executing the obsolete plan indefinitely. Define pending-versus-active revision visibility while preparation is in progress.

Use shared engine scheduling and event contracts, not a thread or timer per Mapping. Process dirty dependencies and genuinely due temporal work. Changes to settings, conditions, bindings, resources, or routing are also dependencies. Preserve trigger multiplicity/order and existing cycle handling. Writes back into Mapping controls use normal queued engine transactions, never recursive evaluation.

### 4.5 Inputs and outputs

Read a coherent source snapshot with explicit Golden projections and context resolution. Missing/disabled sources retain declared identity and produce appropriate validity; do not shift subsequent channels. Explicit removal changes layout. Never fabricate a default source value unless the user selected a fallback policy.

Each output is a normal command invocation with stable argument bindings, not a positional partner of one ValueSet entry. Support channels, compatible component projections, constants, and existing property/context bindings. Multiple commands may read the same final channel without a Duplicate filter.

Keep compound values typed unless the command binding explicitly projects or expands them. Verify target and argument compatibility before enqueueing the local dispatch batch. Structural/binding errors prevent malformed partial batches; intentional gate suppression is normal flow, not an error. Define which commands are eligible from their required argument deliveries.

Update output-change caches only after local dispatch acceptance, keyed by output identity, processor context, binding revision, and semantic argument values. Target/routing edits and lifecycle force-send controls invalidate them appropriately. Trigger occurrences must not be collapsed by value equality.

Module runtimes continue to own IO, reconnect, retry, and external delivery. Local validation atomicity is not a promise of atomic execution across external devices.

## 5. Implementation phases

Every phase includes the Git and progression protocol in Section 3. The implementation remains buildable after every phase. Fine-grained internal commits are allowed, but they do not replace the phase checkpoint and push.

### Phase 00 — Verify baseline and establish evidence

**Work:** inspect repository state, instructions, manifests, toolchain pins, CI, and applicable source paths. Record the actual baseline and approved branch. Compare the current code to the reviewed limitations instead of assuming they still exist.

Create the plan, progression document, acceptance ledger, and a concise Mapping architecture document at `docs/architecture/builtin-mapping.md`. Record decisions from Section 4, existing behavior to preserve, planned persisted changes, and the ownership map.

Discover actual package names, test commands, code generators, watch workflow, headless fixtures, and product smoke checks. Capture baseline results and benchmark fixtures before changing execution. Inspect the Chataigne 1 mapping/filter/output behavior as a product reference, not a porting template.

**Exit gates:** the baseline is reproducible; all required checks are identified; pre-existing failures are distinguished from changes; every later acceptance scenario has an owning phase; baseline documentation is committed and pushed. Do not commit deliberately failing tests to simulate progress.

### Phase 01 — Direct evaluation results and native value handoff

**Work:** add/reuse explicit compiled result access that remains valid when unchanged nodes are skipped. Remove managed execution's dependence on `DebugCaptureSink`, mandatory preview samples, and forced reevaluation solely to retrieve results.

Replace the managed ValueSet JSON encode/decode handoff with native typed data through existing public contracts. Preserve required persistence codecs at actual boundaries. Make diagnostics and result availability explicit; no missing-result fallback values.

**Exit gates:** existing Remap, Smooth, aggregate, Pack Vec3, and Action tests remain valid. Preview-off execution produces correct outputs; skipped unchanged nodes expose their current results. Instrumentation verifies zero intermediate JSON encoding and zero debug samples when capture is off. Characterize remaining allocations rather than claiming they are already eliminated.

### Phase 02 — Typed channel layouts and stable identity

**Work:** implement heterogeneous descriptors/frames, stable channel identity, selection/grouping contracts, compound preservation, range/provenance metadata, and structural-versus-value revisions.

Validate identity uniqueness and type/frame consistency at authoring/materialization boundaries. Resolve output identities for pack/extract/duplicate before evaluation. Preserve declaration identity through temporary source unavailability and input disabling. Add backend-owned layout queries needed by later UI work.

**Exit gates:** tests cover mixed types, repeated references to one source, stable reorder, source rename, disable/reenable, removal diagnostics, extraction identity, grouping order, metadata-only updates, and empty/incomplete authoring states. Value changes do not rebuild layouts.

### Phase 03 — Declarative filter applications and runtime bindings

**Work:** extend the existing ANode capability API to describe instance-aware managed applications, multiple outputs, auxiliary bindings, selections, grouping, and state scope. Replace static-only assumptions without introducing a second registry of operation behavior.

Implement Math's per-channel and combine modes. Bind runtime settings through existing parameter/property/context contracts. Add declaration validation so the palette cannot advertise an application that cannot lower or execute.

**Exit gates:** a parameter change made through a backend edit updates filter behavior without recreating the plan or resetting unrelated memory. The same Math kernel serves graph and Mapping use. Signatures diagnose unsupported selection/type/socket combinations. Role/capability queries return executable choices, not optimistic placeholders.

### Phase 04 — Arbitrary composable managed compilation

**Work:** replace the terminal-projection restriction with generic composition of typed stages. Support repeated pack/extract/reduce/convert operations before, between, and after per-channel operations. Remove hardcoded Pack Vec3 projection dispatch in favor of declared socket/output bindings.

Lower managed regions through actual Formula boundaries and preserve all surrounding authored operations. Support compatible plan sharing, specialization caching, reusable result buffers, dependency tables, instance binding maps, and authored preview attribution. Keep compilation out of steady-state evaluation.

**Exit gates:** execute `Remap → Sum → Smooth`, `Pack Vec3 → Extract → Math → Pack Vec3`, and heterogeneous selected-channel chains. A custom Formula with managed regions plus an extra operation runs that operation. Equivalent instances share executable plans but not mutable state. Unsupported graphs diagnose instead of silently switching to a reduced evaluator.

### Phase 05 — Flow control, temporal scheduling, and state correctness

**Work:** implement suppression/hold/default/trigger flow semantics and their propagation through all stage applications. Complete dirty tracking for data and control dependencies, per-stage temporal wakeups, and separate send eligibility.

Integrate processor contexts, sparse state, compatible migration, gate reopening, lifecycle reset/freeze/resume, and bounded retention. Publish revised plans atomically and reject stale builds. Reuse engine cycle/transaction handling for mappings controlling other mappings or their own settings.

**Exit gates:** tests prove closed suppressing gates send no zero, defaults send only when selected, repeated triggers remain distinct, a changed gate condition reacts to an unchanged source, and smoothing progresses while its source is steady when appropriate. Context/channel histories remain isolated; rename/reorder do not exchange them. Add/remove/edit operations retain only compatible state, release removed state, and never execute partially updated chains.

### Phase 06 — Complete input resolution and command output binding

**Work:** connect declared inputs to coherent engine snapshots, projections, supported bindings, and context axes. Implement per-command argument bindings, normal fan-out, local validation, output change caches, and processor send/lifecycle policy integration.

Preserve existing generic parameter commands and module-provided commands. Do not introduce another device dispatch path. Expose binding diagnostics through the standard backend surfaces and include routing dependencies in execution planning.

**Exit gates:** test one result driving several commands; multiple channels driving several arguments of one command; constants and compound projections; unavailable sources; removed selections; invalid targets; disabled outputs; changed destinations with unchanged data; suppression; enqueue rejection; and correct event order. Disabling or reordering an output must not remap unrelated bindings.

### Phase 07 — Complete the required filter catalog

**Work:** inventory existing kernels first, then complete managed applications and add only missing operations. The following matrix is required unless a documented product decision explicitly changes scope; existing source files alone are not evidence of completeness.

| Family | Required Mapping capability |
|---|---|
| Numeric | Remap, Clamp, Math per-channel/across selected channels, negation/inverse, existing applicable numeric functions |
| Reduction | Sum, product, min, max, average, ordered difference, explicitly defined two-value distance |
| Layout | Select/reorder/duplicate, Pack/Extract Vec2 and Vec3, Pack/Extract Color |
| Conversion | Explicit integer/float/boolean/string conversion, component projections, declared vector/color conversions |
| Curves | Curve remapping through existing Golden curve data and sampling facilities |
| Temporal | Existing smoothing methods, speed, hold/freeze, one-tick delay, bounded timed delay |
| Conditions | ConditionGate and appropriate comparison/threshold applications |
| Color/string | Gradient sampling, supported color modes, formatting, concatenation |

For timed delay, define clock, bounded capacity, overflow diagnostics, memory limits, event ordering, and disable/reset policy. Do not expose unbounded queues. Define integer overflow, division by zero, invalid parsing, non-finite values, empty reduction, incompatible components, and missing operands for every affected operation.

Curve keys and gradient stops remain editable/addressable through existing node contracts. Reuse Golden inspectors and compiled samplers. Do not add Mapping-only curve or gradient models. Collection split may remain collection-valued; unbounded channel explosion and arbitrary Script/subformula filters are not required for this release.

**Exit gates:** every advertised filter has declaration, layout, numerical/flow, state, error, persistence, and graph/Mapping equivalence tests where applicable. Live external edits to coefficients, curve keys, and gradient stops update processing. No unavailable filter is advertised as implemented.

### Phase 08 — Backend authoring, asset, and persistence integration

**Work:** finish manager materialization and edit intents for inputs, filters, selections, grouping, and command argument bindings. Batch known node trees and transactions; avoid sequential child-creation storms and full snapshot rebuilds.

Update/export `Mapping.json` through supported tooling, retaining stable built-in catalog identity and hidden-from-project-library behavior. Keep one Mapping entry and normal read-only built-in inspection. Add precise migrations for changed processor/filter/output records and changed gate semantics. Remove superseded wrappers only after their necessary migration or explicit diagnostic boundary exists.

Regenerate affected DTOs and update consumers in the same phase. Test create/edit/delete/reorder/copy/duplicate/undo/redo/save/load through public backend APIs. Keep custom Formula and Action paths working throughout.

**Exit gates:** a Mapping can be fully configured through backend intents without UI repair. Persistence retains stable IDs and bindings; duplicate creates appropriately distinct ownership identities. Reloaded projects preserve explicit historical behavior. Missing/unknown assets or schemas diagnose instead of silently resetting data. Bulk creation emits bounded transactions/events.

### Phase 09 — Complete the Svelte 5 Mapping inspector

**Work:** preserve the Inputs → Filters → Outputs interaction. Add compact filter rows, standard parameter editors, backend-derived compatible palettes, channel/group selectors, before/after layout summaries, command argument binding editors, and actionable diagnostics.

Provide opt-in, bounded value previews for selected stages and one processor context; virtualize large lists where appropriate. Distinguish authored, pending, and active runtime revisions. Preview focus must not mutate runtime state or Formula defaults. Unmount/session changes unsubscribe and release capture/history.

Use Golden inspector hooks and controls, runes, keyed stable identities, relative units, and generated DTOs. Do not reimplement type solving, label allocation, binding defaults, or structure repair in TypeScript.

**Exit gates:** frontend tests cover ordinary creation/editing and repeated layout changes, keyboard/focus behavior where supported, undo/redo, diagnostics, preview context changes, and teardown. Verify the actual product surface rather than only mounting an isolated component. No mandatory preview traffic when the inspector is closed. Existing panels and Formula editor remain usable.

### Phase 10 — Convert a configured Mapping to a custom Formula

**Work:** implement an atomic backend operation that materializes the current configured Mapping, including selections, settings, auxiliary bindings, resources, output commands, and relevant exposed properties. Duplicating the empty built-in asset does not satisfy this phase.

Use the same reusable operations and managed constructs so the result can be edited as a normal Formula. Preserve compatible state with a tested identity map, or perform an explicit documented reset where migration is unsupported. Keep processor identity and external references stable where possible.

Replace execution atomically so old and new processors cannot both dispatch during conversion. Implement undo/redo and persistence. Do not promote a disposable compiled plan into the authored source of truth.

**Exit gates:** deterministic Mapping-versus-converted-Formula tests cover values, suppression, events, commands, context bindings, properties, and fresh-state temporal behavior. Conversion respects its state policy, preserves configured resources and edits, and emits no duplicate side effects. The converted Formula is editable and survives reload.

### Phase 11 — Qualification, performance, and final cleanup

**Work:** run the complete acceptance matrix, relevant workspace/product gates, packaging checks, and benchmark suite. Remove obsolete terminal-projection paths, debug-result dependencies, intermediate serialization, legacy evaluators, unused wrappers, and temporary rollout scaffolding.

Document the completed architecture, authoring contract, filter extension path, state/flow semantics, diagnostics, and measured performance. Reconcile every ledger entry with real evidence. Validate existing Action, custom Formulas, statechart, context/multiplex, module commands, and launch workflows—not only Mapping tests.

**Exit gates:** all mandatory acceptance cases pass; relevant CI passes; no unresolved feature-owned regressions remain; final progress documentation is committed, pushed, and verified. Any unavailable mandatory desktop/platform check keeps overall status `IN_PROGRESS` or `BLOCKED`, not `COMPLETE`. Finish with an evidence-based handoff listing implementation and status commit SHAs, branch/remote, tests, measurements, and any explicitly excluded scope.

## 6. Required acceptance matrix

Track these IDs from Phase 00 and add more where implementation reveals additional risks.

| ID | Scenario | Required result |
|---|---|---|
| M01 | Three floats → Remap → Sum → Smooth → two commands | Mid-chain aggregation, downstream state, ordinary fan-out |
| M02 | Three floats → Pack Vec3 → Extract → reorder → Pack Vec3 | Repeated layout transitions and stable component bindings |
| M03 | Float + bool + string; select only the float | Mixed types work; unselected channels remain intact |
| M04 | Color → Extract RGB → Smooth selected components → rebuild | Per-component state; declared alpha handling |
| M05 | Closed numeric suppressing gate | No unintended zero/default command |
| M06 | Gate condition changes while source is steady | Output eligibility changes without source-change dependence |
| M07 | Hold/default before first accepted sample and after reopening | Explicit, deterministic behavior without fabricated values |
| M08 | Rename/reorder/add unrelated channels during smoothing | Compatible state preserved; no cross-channel transfer |
| M09 | Identical filters across processors and multiplex contexts | Shared executable structure; isolated state and settings |
| M10 | State/script/Mapping changes coefficient, key, stop, or binding | Same backend behavior as inspector edits |
| M11 | Several outputs bind one channel; one output binds several | Typed argument routing without positional pairing |
| M12 | Source/target unavailable, type changed, selection removed | Stable identities and useful diagnostics; no malformed partial batch |
| M13 | Same-valued trigger events repeat | Occurrences/order preserved under event contracts |
| M14 | Save/load, copy/duplicate, undo/redo, semantic migration | Identities and authored meaning retained |
| M15 | Custom Formula contains managed regions and extra operations | All authored graph semantics execute |
| M16 | Convert configured Mapping to custom Formula | Equivalent behavior and no duplicate effects |
| M17 | All processing with previews disabled | Results independent of debug machinery |
| M18 | Structural edit arrives during compilation/evaluation | Revision-safe activation; stale plans rejected |
| M19 | Mapping output changes its own or another Mapping's controls | Normal queued transaction/cycle rules; no recursive runaway |
| M20 | Inspector/context switching and large lists | Bounded capture, cleanup, and preserved product responsiveness |

Add property-based layout tests and deterministic differential tests using existing test tooling where suitable. Numeric comparisons use declared tolerances; identities, ordering, delivery flags, and intent counts use exact assertions. Avoid tautological tests that merely compare a function with itself through two wrappers.

## 7. Validation and performance requirements

### 7.1 Commands and product checks

Confirm these against the checkout during Phase 00. The reviewed workspace provides these package names and root scripts; extend with targeted tests and existing CI commands rather than inventing package names. [R7]

```sh
cargo metadata --no-deps --format-version 1
cargo fmt --all --check
cargo test --locked -p chataigne_alchemist -p chataigne_processor -p chataigne_condition -p chataigne_state_machine
cargo check --locked --workspace
npm run check
npm test
npm run lint
npm run build
```

For changed protocol boundaries, run the actual generator explicitly and fail on its errors. Do not rely on a lifecycle script that can mask preparation failures. Relevant existing app-workspace commands include:

```sh
npm run codegen:state-machine-protocol --workspace chataigne-ui
npm run codegen:golden-ui-protocol --workspace chataigne-ui
npm run codegen:golden-graph-ui --workspace chataigne-ui
```

Run only the generators affected by the changes, then verify regenerated output and consumers together. Add Golden crate tests, clippy, supported platform/feature combinations, and the repository's product/qualification gates whenever their boundaries are touched. Do not indiscriminately enable incompatible platform-specific features.

Before phase commits, run the required formatter, including Golden Core's applicable formatting scope, then recheck. Avoid formatting unrelated files solely to enlarge the diff. A successful `cargo check` is not proof of linking, launch, or interactive correctness.

Preserve and verify `cargo run`, `cargo run -- --dev`, the existing watch workflow, bundled UI operation, supported headless operation, and normal packaging/resource lookup. Use permitted non-interactive test infrastructure; record desktop checks that require a suitable environment rather than claiming they occurred. Keep the supported toolchain and existing modules working.

A pre-existing unrelated failure may be recorded separately; it must not conceal a new failure or excuse skipping validation of a changed boundary. Mandatory environment-blocked checks remain outstanding until actually performed.

### 7.2 Benchmark workloads and hard invariants

Benchmark recorded hardware/build profiles with representative processor counts such as 1,000 and 10,000, channel counts such as 1/8/32, and short/long filter chains. Use a documented representative subset, not an impractical Cartesian product. Include mixed types, aggregation, multiple contexts, structural edits, and bounded temporal history.

Measure idle work, sparse source changes, runtime-setting changes, continuous filters, dispatch load, and previews separately. Record allocations, compile/cache counts, stages/channels evaluated, retained state, p50/p95/p99 latency, event/preview volume, and memory cleanup.

Required execution invariants:

- No compilation, authored graph materialization, intermediate JSON serialization, or debug capture in ordinary steady-state Mapping evaluation.
- No plan recompilation for changing a runtime coefficient, value, label, or preview selection.
- For fixed-size numeric paths after warmup, no avoidable per-channel/per-stage allocation; close gaps in kernel output allocation through shared reusable buffers/sinks. Variable-size strings/collections have explicit measured bounds.
- Idle and sparse work scales with dirty dependencies and due temporal work, not a repeated whole-project scan. Large-context memory reclamation must not require rebuilding all keys every tick.
- Compatible processors share executable specializations while keeping mutable state isolated. Expansion/history limits and specialization caches are bounded.

Set wall-clock regression thresholds from the baseline and recorded target hardware. Do not weaken thresholds, remove difficult scenarios, or disable behavior to make a benchmark pass. Report correctness and performance together.

## 8. Definition of done and final handoff

The feature is complete only when one file-authored built-in Mapping supports the required mixed layouts, selections, mid-chain shape changes, live controls, temporal/gating semantics, and command bindings; its UI remains usable; and conversion to custom Formula works through shared semantics.

Every required filter must actually execute from the normal product palette. All authored controls remain backend-addressable. Existing Action/custom Formula/module/state-machine behavior and supported launch workflows remain intact. Hot paths no longer depend on debug capture or intermediate JSON.

Every phase must have its validation evidence, verified pushed implementation commit, and published progress checkpoint. All mandatory acceptance/product checks and required CI must pass. The progression document must show both completed work and any explicitly excluded scope without disguising untested work as complete.

The final Codex handoff must identify the branch and remote, phase implementation commits, final verified status commit, commands and outcomes, benchmark evidence, product checks, and any remaining blockers. Do not merely say “implemented” or “tests pass.”

## Reference source paths

These paths were reviewed at the baseline SHA above. Read the corresponding current files before implementation and follow moved public boundaries rather than resurrecting obsolete locations.

```text
AGENTS.md
Cargo.toml
package.json
apps/chataigne/ui/package.json
docs/architecture/alchemist-runtime.md
apps/chataigne/resources/formulas/builtin/Mapping.json
apps/chataigne/systems/alchemist/src/node.rs
apps/chataigne/systems/alchemist/src/pipeline.rs
apps/chataigne/systems/alchemist/src/formula.rs
apps/chataigne/systems/alchemist/src/library/anodes/mod.rs
apps/chataigne/systems/alchemist/src/library/anodes/condition_gate.rs
apps/chataigne/systems/alchemist/processor/src/managed_formula.rs
apps/chataigne/systems/alchemist/processor/src/value_set.rs
apps/chataigne/systems/alchemist/processor/src/value_set_pipeline.rs
apps/chataigne/systems/alchemist/processor/src/input_set.rs
apps/chataigne/systems/alchemist/processor/src/output_set.rs
apps/chataigne/systems/alchemist/processor/src/tests/
apps/chataigne/systems/alchemist/integration/processor/
apps/chataigne/systems/alchemist/integration/filters.rs
apps/chataigne/ui/src/lib/systems/alchemist/
apps/chataigne/systems/state_machine/runtime/src/protocol.rs
```

Chataigne 1 behavior references, in `benkuper/Chataigne` at the stated comparison commit:

```text
Source/Common/Processor/Mapping/Mapping.cpp
Source/Common/Processor/Mapping/Filter/MappingFilter.h
Source/Common/Processor/Mapping/Filter/MappingFilterManager.cpp
Source/Common/Processor/Mapping/Filter/filters/conversion/MergeFilter.cpp
Source/Common/Processor/Mapping/Output/MappingOutput.cpp
Source/Common/Processor/Mapping/Output/MappingOutputManager.cpp
```

### Source attribution

- [R1] GitHub branch metadata for `Golden-Geek/Chataigne2`, branch `main`, rechecked September 12, 2026: `5dd0be6f90d861573b3faa1dd1fe0c447dee0a5a`. Chataigne 1 branch metadata: `benkuper/Chataigne`, `master`, `f8634ae02c3efd1620077d5be3e387f896f5ddbc`.
- [R2] `docs/architecture/alchemist-runtime.md` and `apps/chataigne/resources/formulas/builtin/Mapping.json` at the Chataigne2 baseline: existing catalog, Formula, and managed-region design.
- [R3] `apps/chataigne/systems/alchemist/processor/src/managed_formula.rs` at the baseline: homogeneous managed filtering and terminal-only projection restrictions.
- [R4] `apps/chataigne/systems/alchemist/processor/src/value_set_pipeline.rs` and `value_set.rs` at the baseline: debug-based result retrieval and JSON-backed ValueSet conversion.
- [R5] `apps/chataigne/systems/alchemist/processor/src/output_set.rs` at the baseline: positional output pairing and single-output scalar requirement.
- [R6] Root `AGENTS.md` at the baseline: Golden/Alchemist ownership, Svelte 5, backend authority, generated protocol, test layout, formatting, and repository-operation rules.
- [R7] Root `Cargo.toml`, root `package.json`, and `apps/chataigne/ui/package.json` at the baseline: package identities, frontend scripts, and code-generation entry points.

All requested behavior, phase requirements, and acceptance gates in this plan are implementation directives, not claims that the feature is already implemented or tested.
