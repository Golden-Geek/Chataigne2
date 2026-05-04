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

## Readability And Reviewability
- Optimize for readable diffs, reviewable formatting, and newcomer legibility.
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
- Start by identifying the layer that should own the change.
- Prefer moving responsibility to the right layer over adding glue.
- Do not preserve broken boundaries for convenience.
- Do not introduce compatibility shims unless the task explicitly requires them.
- Do not duplicate protocol, persistence, or host declarations across languages or layers.
- For reused Params DSL folders, do not restate inherited metadata such as `label` unless the change intentionally diverges from the base declaration.
- Do not leave path-based imports into private submodule internals in app code or build scripts.
- When the repository already violates these rules, treat that as cleanup pressure, not as precedent.
