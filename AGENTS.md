# AGENTS.md

## Mission

This repository is building the long-term architecture for `Chataigne2`, `MapGyver`, `golden_core`, and the UI stack.

The goal is a clean, correct, scalable foundation.
Backward compatibility is not a goal unless a task explicitly asks for it.

## Active Implementation Plan

- When the user says “implement the plan” or asks to continue implementation, use
  `docs/plan/Chataigne2_Codex_Implementation_Plan.md` as the single execution entrypoint.
- Read its current ledger at `docs/progress/implementation-status.md` and reconcile the checkout
  before continuing the next dependency-ready task. Do not restart completed historical work.
- For an explicit request to implement **MapGyver**, use
  `docs/plan/MapGyver_Implementation_Plan.md` and `docs/progress/mapgyver-status.md` instead.
  MapGyver is a sibling app sharing Golden, not a restart of Chataigne's plan. The unqualified
  phrase “implement the plan” continues to select Chataigne's entrypoint above.
- Documents under `docs/archive/` preserve evidence only. Their pauses, branch/push instructions,
  old status tables, and task lists are non-operative.

## Implementation Plan Delivery

- For every implementation plan, commit and push at each meaningful phase or coherent,
  reviewable implementation milestone. Do not wait for the entire plan or a large phase to finish.
- This is standing authorization to make milestone commits and push them without asking again.
  Use the plan's selected branch and its remote; establish the upstream when needed. Do not
  switch products or branches merely to deliver a milestone.
- Before each milestone commit, run the relevant checks and required formatting, and update
  the owning status ledger/dashboard with evidence, remaining scope and blockers. Include only
  that milestone's changes; preserve unrelated work.
- Checkpoint useful partial work and diagnostic evidence when a larger acceptance gate remains
  open, stating those limits explicitly. A commit or push never closes an unverified gate.
- Push the milestone before starting the next implementation batch. Reconcile remote changes,
  preserve other contributors' commits, and never force-push or rewrite published history.
  If validation or delivery fails, repair it where possible, retain local work and report the
  actual blocker; report a successful push only after verifying it.
- This rule applies to existing and future implementation plans. It does not authorize releases
  or deployments, and an explicit user instruction can override it for a particular task.

## Autonomous Progress And Anti-Drift

- At the start of each implementation batch, record one concrete product deliverable, its
  owning layer, the check that proves it works, and a stopping time in the active ledger.
  Default to a reviewable feature slice within 45 minutes, including investigation and checks.
  Larger features may span batches; keep advancing the same product behavior rather than
  replacing it with an expanding infrastructure investigation. Splitting delivery must preserve
  the full planned feature and its acceptance requirements.
- Time-box an investigation to 20 minutes and at most two experiments, including the baseline.
  State the hypothesis and what result would change the implementation before running them.
  Reaching the limit means preserve the findings and choose the next action; do not invent
  another probe, profiler, benchmark variant or measurement prerequisite to extend the chain.
  A live check may finish or be handed off with its verified handle; do not restart it to fit a box.
- Spend at least four product-delivery batches for each diagnostic-only qualification batch.
  Never run consecutive diagnostic-only batches unless the user explicitly requests that work
  or a reproduced correctness/safety defect blocks the selected feature. A new report, archive,
  test count, refactor or commit alone is not a delivered product capability.
- When the user explicitly prioritizes completing an already implemented plan, switch to
  acceptance closure: keep one primary phase/gate active, finish dependency-ready acceptance
  before opening more scope, and maintain an explicit return queue for blocked gates. Product/
  diagnostic quotas must not cause unnecessary features or phase hopping in this mode;
  consecutive qualification batches are allowed, but investigation/time limits still apply.
  Every original requirement remains required until verified or explicitly revised by the user.
  For a minor optimization consuming roughly a day, propose a precise deferral/lower gate with
  evidence and consequences; never apply that revision silently or classify correctness,
  ownership, data integrity or essential behavior as optional optimization.
- Gate native/hardware experiments on terminal success of prerequisite compile/software checks.
  Inspect returned results before issuing dependent validation; do not spend the experiment
  allowance on a known failing build or continue a validation chain after its prerequisite fails.
- After two batches without a usable capability or acceptance closure, reduce the current
  feature slice or select the next functionally ready product task. Explain the concrete
  obstacle once; do not keep returning with the same phase percentage and another research plan.
- Distinguish implemented, functionally verified and production-qualified work. Follow the
  selected plan's explicit functional dependencies to build subsequent features while retained
  qualification work proceeds in its bounded lane. Never count an unverified gate as passed,
  relax budgets, hide failures, or claim release readiness to show progress. Record any execution
  ordering change in the plan and ledger; do not silently reinterpret their acceptance scope.
- Reopen a parked investigation only for a reproduced blocker on the current product path,
  a scheduled qualification batch within the allocation above, or an explicit user request.
  Preserve unfinished agent work in a named stash or separate checkout before switching focus;
  record its exact reference and limits, and do not automatically restore it on continuation.
- Prefer existing evidence and the smallest useful reproducer. Do not add diagnostic frameworks,
  full source snapshots or large evidence archives unless a named acceptance requirement needs
  them and the current batch has a specific decision they will resolve.
- End each handoff with the new app capability, relevant checks, actual gate changes and the
  next product deliverable. Keep qualification debt visible without making it the default work
  queue. Explicit user priorities override these default allocations.
- Check the recorded stopping time before each new experiment or scope expansion. At the
  cutoff, stop adding work, finish or report the running check, and checkpoint the verified
  partial result. A continuation, context reset or renamed batch does not reset an investigation
  limit. Reopening the same question requires new evidence that changes a named product decision.
- Make stalled progress visible in the dashboard: name the current usable feature and the last
  delivered capability, alongside open acceptance gates. A phase percentage alone must never
  become the work queue. Do not grow evidence reports when the next action is already known.

## Core Stance

- Prefer the cleanest architecture over the smallest diff.
- Break APIs when that removes flawed structure or duplicated concepts.
- Remove obsolete patterns instead of layering compatibility glue on top.
- Fix problems at the correct boundary, not at the nearest call site.
- Keep the app viable for very large graphs and tens of thousands of nodes.
- Only edit files with the standard file editing tool, not shell-based file mutation.

## Target Architecture

### Chataigne2 App Shell

- `Chataigne2` stays a thin app shell.
- App code should focus on app node registration, lifecycle initialization, composition, and product-level wiring.
- By default, apps should launch through the reusable ready-to-run runtime provided by `golden_core` rather than reimplement desktop, headless, or transport bootstrap locally.
- Reusable engine, default host runtime, protocol, persistence, and UI logic belongs in reusable workspace crates or packages, not in the shell.
- Avoid touching `src/main.rs` unless the task is specifically about app-shell startup.

### Core And Host Separation

- `golden_core` should provide the default ready-to-launch runtime stack: desktop/Tauri startup, headless startup, built-in transport server, and native dialogs.
- Pure engine logic inside `golden_core` must still remain usable without going through desktop-only code paths.
- Desktop concerns, browser/headless concerns, native dialogs, and transport servers belong in explicit host or transport modules in `golden_core` or sibling workspace crates, not in `src/app`.
- Do not move the default Tauri/headless/file-dialog host path back into the app shell.
- Persistence must not be hidden inside host/bootstrap code.

### Golden Package Boundary

- `golden_core` and `golden_ui` must stay app-agnostic reusable packages.
- Do not put `Chataigne2`-specific nodes, module behavior, or product policy inside any `golden_*` package.
- App-owned script templates for Chataigne module nodes must live in the app layer under `src/module`, not in `golden_core`; `golden_core` may expose only generic reusable scripting primitives and snippet expansion helpers.
- When app-specific UI needs inspector, outliner, context-menu, dashboard, or similar customization points, add a public hook or registry in `golden_*` and register the app behavior from the app layer.
- No app-layer code inside `golden_*`.
- MapGyver media, composition, Matrix, screen/surface policy and specialized editors belong
  under `apps/mapgyver`. Planned reusable GPU/runtime and native-preview mechanisms belong in
  Golden packages and the shared desktop host; do not copy Chataigne's private modules or host.

### Alchemist Ownership

- `golden_graph` and the final `golden_graph_ui` are the complete app-agnostic graph document,
  editing, presentation, and canvas system. They must never import Alchemist or Chataigne types.
- Alchemist is Chataigne-specific. Its formula model, ANode registry, compiler/runtime, assets,
  catalog policy, graph-domain adapter, and formula UI belong under `apps/chataigne` and consume
  only public Golden contracts.
- The Rust implementation lives at `apps/chataigne/systems/alchemist` as `chataigne_alchemist`.
- Reusable Golden runtime, protocol, persistence, processor, condition, and UI layers must use
  domain-neutral contracts and must not depend on the app-owned Alchemist implementation.

### Public Boundaries Only

- App crates must not import private submodule files by filesystem path.
- Do not use `#[path = "..."]` to reach into another crate or submodule's internals from app code.
- Build scripts must consume a supported public API, crate, or module intended for that purpose.
- If a boundary is worth depending on, make it public and stable instead of path-importing internals.

## Protocol Rules

- Rust and TypeScript must not hand-maintain duplicate protocol declarations.
- Request, response, event, snapshot, and protocol-version types must have one source of truth.
- Prefer generating TypeScript from Rust DTOs or from a shared schema.
- Any protocol change must update the generator, generated output, and consumers in the same change.
- Do not accept drift between Rust DTOs and UI types.

## UI Architecture Rules

- Use Svelte 5 and runes only.
- Do not use legacy `on:` event syntax; use `onclick`, `onfocus`, `onblur`, and similar direct event props.
- Use relative units for layout and spacing: `em`, `rem`, `%`, `vh`, `vw`.
- UI must not own backend/domain behavior. It may collect user input, compute viewport-dependent presentation hints, and send explicit intents, but it must not allocate unique node labels, choose domain defaults, infer control modes, encode module policy, decide graph mutation semantics, or write internal node parameters to make a higher-level operation valid. Put that behavior in `golden_core`, the app/module backend, or a backend intent so every UI, transport, script, and headless caller gets identical behavior. Do not add UI-side compatibility shims for backend behavior drift.
- Avoid fixed pixel sizing unless a task deliberately establishes and documents an exception.
- Keep UI state split into small focused stores with one thin facade where needed.
- Do not let orchestration files grow into god objects.
- Session and state logic must depend on transport interfaces, not directly on a websocket implementation.
- Prefer composition of focused stores over utility files that know everything.

## Persistence Rules

- Serialization contracts, project schema, codecs, and migrations belong in persistence or protocol layers.
- Desktop file dialogs and host workflow do not belong in persistence modules.
- Host code should call persistence APIs, not own persistence formats.

## Rust Engineering Standards

- Follow idiomatic ownership, explicit invariants, and type-driven design.
- Favor compile-time guarantees over runtime patching.
- Keep modules cohesive and APIs minimal.
- Avoid hidden side effects, accidental global state, and unnecessary indirection.
- Treat warnings, clippy findings, and awkward APIs as signs to improve the design.
- Prefer conventional crate layouts where module paths match the filesystem.
- Avoid `#[path]` module wiring unless there is a strong generation-related reason.
- When asked to implement a feature or functionality needing a full protocol implementation or API interfacing, check if there are reliable crates that are already providing this (like midi, osc, serial, websocket, hid...) instead of recreating the protocol.
- Before implementing structural graph changes, reason about worst-case node counts, lifecycle callbacks, snapshot rebuilds, and event fan-out. When a known subtree or missing path can be materialized in one operation, prefer `NodeTree` / `AddNodeTree` over sequential child edits and retry loops.

## Readability And Reviewability

- Optimize for readable diffs, reviewable formatting, and newcomer legibility.
- Keep source files under 1000 lines by splitting cohesive modules before they become hard to review.
  Generated files, lockfiles, long-form docs, and intentionally centralized registries may exceed this only when the tradeoff is documented.
- Do not keep ultra-wide formatting or dense one-line source files.
- Use standard formatters with sane line widths.
- Reformat touched code consistently with the repository formatter instead of preserving unreadable layout.
- Keep runtime and tests in separate files. Do not leave inline `mod tests { ... }` blocks in implementation files.
- Every test source belongs under a `tests/` directory. Keep that directory directly beside the module or feature it tests; use `tests/mod.rs` as the Rust unit-test module and split larger suites into focused files below it.
- Keep crate-level integration tests and their fixtures under the crate's top-level `tests/` directory. Do not scatter `tests.rs`, `*_tests.rs`, `*.test.ts`, or `*.spec.ts` beside runtime sources.
- Keep comments for intent and tradeoffs, not narration.
- New top-level docs should explain where responsibilities live before pointing people into implementation details.

## Documentation Rules

- Implementation status ledgers must start with a compact progress dashboard immediately below
  the title: last-updated date, overall progress bar and percentage, completed/total phases,
  current phase/topic, next action and any blocker. Include one row per phase with its state,
  progress bar, percentage and completed/total acceptance items; link to the detailed evidence.
- Keep that dashboard synchronized with the ledger in every implementation handoff and milestone
  update. Define item scope before awarding progress, count only verified acceptance, and state
  the calculation. Use fixed acceptance gates rather than commits, sessions or elapsed time;
  partial work and unavailable checks stay open. Explain any denominator change. Apply this
  format when next updating an existing ledger; do not rewrite inactive product ledgers solely
  to adopt it.
- When architecture changes, update the docs in the same change.
- Prefer short, high-signal docs that explain boundaries, ownership, and where to start.
- Link to deeper existing design docs instead of duplicating them.
- Root documentation should let a new contributor understand the top-level layers in minutes.

## Decision Rules

- If two approaches work, choose the simpler and more defensible design.
- If a proposal adds complexity without strong value, reject it.
- If a refactor yields a cleaner core, prefer it even when migration work is required.
- Document non-obvious tradeoffs where the design could otherwise be misread.
- Clean obsolete code as you go; do not leave dead aliases, legacy branches, or unused compatibility helpers behind.

## Quality Bar

- Production-oriented quality is expected even during refactor-heavy phases.
- Keep the tree cleaner after each change.
- Unify concepts instead of duplicating them across layers.
- Temporary hacks are acceptable only when they are explicitly scoped and scheduled for removal.

## Preferred Refactor Order

When a task spans multiple architectural areas, prefer this order:

1. Readability guardrails, formatting, and top-level docs.
2. Public build and codegen boundaries.
3. Crate and package boundary cleanup.
4. Protocol single-source generation.
5. UI store and session decomposition.
6. Persistence boundary cleanup.
7. Filesystem and module layout normalization.
8. Onboarding docs and repo map improvements.
9. Metadata, license, and version-policy cleanup.

## Working Rules For Agents

- Desktop automation is allowed for authorized development and verification tasks, including
  keyboard/mouse input and native window inspection. Prefer the available computer-use skill
  and its supported APIs; keep actions focused on the task's app and owned test windows.
- Use ordinary non-interactive processes for terminal commands. Do not drive a terminal UI
  merely to run a command or show its output.
- Start by identifying the layer that should own the change.
- Prefer moving responsibility to the right layer over adding glue.
- Move parsing, timestamping, and transport-side preprocessing out of the engine loop whenever an IO/runtime boundary can do that work first; keep the engine loop focused on applying state and graph mutations.
- Treat recurring millisecond-range compute or polling on the main thread as a hard no for node implementations. App Control idle polling already pushed `scheduled_ms` above 15ms, and the OS adds its own recurring ~20ms tick, so node work at that cadence must move to an IO/runtime boundary, a background worker, or a coarser event-driven path.
- For large or data-driven structure creation, design the edit shape before coding: batch detached subtrees, avoid repeated full-tree snapshot rebuilds, and add timing or tests when the expected graph size can grow significantly.
- Do not preserve broken boundaries for convenience.
- Do not introduce compatibility shims unless a task explicitly requires a persisted-data
  migration. Keep any such migration narrow, typed, tested, and removable.
- Do not duplicate protocol, persistence, or host declarations across languages or layers.
- When asked to create a new module, treat the module as incomplete unless its command nodes, script-callable functions, script callbacks, and app-owned script template snippets are designed and wired in at the same module boundary.
- Any implementation involving connection to an endpoint (hardware or software) needs to have an autoreconnect / device recovery strategy
- For reused Params DSL folders, do not restate inherited metadata such as `label` unless the change intentionally diverges from the base declaration.
- Do not leave path-based imports into private submodule internals in app code or build scripts.
- When the repository already violates these rules, treat that as cleanup pressure, not as precedent.
- Always finish with cargo fmt on both root and golden_core to avoid CI failures

## Code Exploration Policy

Use native repository and language tools. Prefer `rg` for text search and `rg --files` for file
discovery, then read only the relevant files or ranges. Use Cargo metadata, compiler output, and
language-server features when they answer dependency or symbol questions more accurately than text
search.

Choose the owning source path before exploring and scope searches accordingly:

| Layer                           | Source path                            |
| ------------------------------- | -------------------------------------- |
| App shell and app-owned modules | `apps/chataigne`                       |
| Golden Core                     | `crates/golden_core`                   |
| Generic graph system            | `crates/golden_graph`                  |
| Chataigne Alchemist             | `apps/chataigne/systems/alchemist`     |
| Chataigne state machine         | `apps/chataigne/systems/state_machine` |
| App-owned UI                    | `apps/chataigne/ui`                    |
| MapGyver shell and systems      | `apps/mapgyver` (planned)              |
| MapGyver UI                     | `apps/mapgyver/ui` (planned)           |
| Generic video runtime           | `crates/golden_video` (planned)        |
| Generic video preview UI        | `packages/golden-video-ui` (planned)   |
| `golden_ui`                     | `packages/golden-ui`                   |
| Generic graph UI                | `packages/golden-graph-ui`             |

Avoid reading generated output, dependency trees, or entire large files when a focused search or
line range is sufficient. For cross-layer changes, search each owning path separately and verify
the public boundary between them.
