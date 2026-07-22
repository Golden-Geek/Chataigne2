# AGENTS.md

## Mission

This repository is building the long-term architecture for `Chataigne2`, `golden_core`, and the UI stack.

The goal is a clean, correct, scalable foundation.
Backward compatibility is not a goal unless a task explicitly asks for it.

## Core Stance

- Prefer the cleanest architecture over the smallest diff.
- Break APIs when that removes flawed structure or duplicated concepts.
- Remove obsolete patterns instead of layering compatibility glue on top.
- Fix problems at the correct boundary, not at the nearest call site.
- Keep the app viable for very large graphs and tens of thousands of nodes.
- Only edit files with the standard file editing tool, not shell-based file mutation.

## Product-Preserving Migration Policy

The active architecture migration is governed by
[`docs/Golden_Architecture_Final_Plan.md`](docs/Golden_Architecture_Final_Plan.md).
Until that migration is complete, the following rules take precedence over wording such as
"thin app shell," "no legacy," or "no compatibility shims":

- The recorded working Chataigne product is the behavioral and experiential oracle. Architectural
  cleanup is not permission to remove its UI, modules, assets, formulas, scripts, fixtures,
  hosting modes, or workflows.
- "Thin app shell" describes final ownership. It does not mean replacing Chataigne with an empty
  shell, registry-only demo, headless harness, or disconnected frontend during migration.
- "No legacy" and "no compatibility shims" describe the final production state. Typed temporary
  adapters, converters, dual reads, and shadow execution are allowed only when they make a named
  runnable checkpoint or persisted-data migration safer; they are not required merely to keep an
  intermediate construction commit launchable.
- Every temporary adapter must be recorded in the parity ledger with an owner, exact scope, expiry
  phase, deletion criteria, deletion issue, and executable tests. Shadow paths must be incapable of
  duplicating commands, triggers, effects, or device traffic.
- A declared `CONSTRUCTION` interval may replace and delete an old in-scope implementation before
  the replacement passes the full application gate. The last runnable checkpoint and baseline refs
  must remain immutable, the affected parity rows and expected breakages must be recorded, and the
  cutover cannot be marked complete until the next named checkpoint passes in the real application.
- The canonical migration branch may be temporarily non-runnable between named checkpoints. Keep
  focused compile, unit, contract, serialization, and performance checks running wherever their
  dependencies are available; do not claim full-product parity from those checks.
- Named checkpoints at the end of Phases 4, 5, 6, 7, 8, and 9 must build, launch, and pass the
  complete applicable Chataigne product gate. Phases 6, 8, and 9 also require their declared
  cross-platform qualification. A checkpoint failure returns the phase to construction state.
- The failed rewrite is a donor, not a migration base. Import or reimplement donor work one reviewed
  unit at a time; never merge or cherry-pick the donor branch wholesale.

The recorded refs, evidence state, and phase status live under
[`docs/product/`](docs/product/README.md). Unknown or unexecuted parity evidence is a blocker, not a
pass.

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

### Alchemist Ownership

- `golden_graph` and the final `golden_graph_ui` are the complete app-agnostic graph document,
  editing, presentation, and canvas system. They must never import Alchemist or Chataigne types.
- Alchemist is Chataigne-specific. Its formula model, ANode registry, compiler/runtime, assets,
  catalog policy, graph-domain adapter, and formula UI belong under `apps/chataigne` in the final
  architecture and consume only public Golden contracts.
- The Rust implementation now lives at `apps/chataigne/alchemist` as `chataigne_alchemist`. The
  empty imported `packages/golden-alchemist-ui` placeholder has been removed.
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
- When tests belong to a module, place them in a sibling test module file (for example `tests.rs` or `*_tests.rs`).
- Keep comments for intent and tradeoffs, not narration.
- New top-level docs should explain where responsibilities live before pointing people into implementation details.

## Documentation Rules

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

- Never invoke or spawn `run-visible-command`, computer-use, desktop-control, SendKeys,
  AutoHotkey, or any mechanism that synthesizes keyboard or mouse input or takes control of the
  user's desktop. A request to show output in a terminal does not authorize GUI automation; use
  ordinary non-interactive processes or provide a command for the user to run.
- Start by identifying the layer that should own the change.
- Prefer moving responsibility to the right layer over adding glue.
- Move parsing, timestamping, and transport-side preprocessing out of the engine loop whenever an IO/runtime boundary can do that work first; keep the engine loop focused on applying state and graph mutations.
- Treat recurring millisecond-range compute or polling on the main thread as a hard no for node implementations. App Control idle polling already pushed `scheduled_ms` above 15ms, and the OS adds its own recurring ~20ms tick, so node work at that cadence must move to an IO/runtime boundary, a background worker, or a coarser event-driven path.
- For large or data-driven structure creation, design the edit shape before coding: batch detached subtrees, avoid repeated full-tree snapshot rebuilds, and add timing or tests when the expected graph size can grow significantly.
- Do not preserve broken boundaries for convenience.
- Do not introduce permanent compatibility shims. During the active product-preserving migration,
  use only the governed temporary adapters authorized above and delete them at their recorded exit
  criteria.
- Do not duplicate protocol, persistence, or host declarations across languages or layers.
- When asked to create a new module, treat the module as incomplete unless its command nodes, script-callable functions, script callbacks, and app-owned script template snippets are designed and wired in at the same module boundary.
- Any implementation involving connection to an endpoint (hardware or software) needs to have an autoreconnect / device recovery strategy
- For reused Params DSL folders, do not restate inherited metadata such as `label` unless the change intentionally diverges from the base declaration.
- Do not leave path-based imports into private submodule internals in app code or build scripts.
- When the repository already violates these rules, treat that as cleanup pressure, not as precedent.
- After a migration phase is fully validated, commit the completed phase before beginning the next
  phase so subsequent sessions start from a clean Git state.
- Always finish with cargo fmt on both root and golden_core to avoid CI failures

## Code Exploration Policy

Use native repository and language tools. Prefer `rg` for text search and `rg --files` for file
discovery, then read only the relevant files or ranges. Use Cargo metadata, compiler output, and
language-server features when they answer dependency or symbol questions more accurately than text
search.

Choose the owning source path before exploring and scope searches accordingly:

| Layer | Source path |
| --- | --- |
| App shell and app-owned modules | `apps/chataigne` |
| Shared Rust crates | `crates/` |
| Chataigne Alchemist | `apps/chataigne/alchemist` |
| Reusable statechart | `crates/golden_statechart` |
| App-owned UI | `apps/chataigne/ui` |
| `golden_ui` | `packages/golden-ui` |
| Generic graph UI | `packages/golden-graph-ui` |

Avoid reading generated output, dependency trees, or entire large files when a focused search or
line range is sufficient. For cross-layer changes, search each owning path separately and verify
the public boundary between them.
