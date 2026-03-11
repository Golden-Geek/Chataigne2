# Source Layout

`golden_engine` uses the filesystem as the source of truth. A contributor should be able to locate a concept by following the folder tree without guessing about hidden `#[path]` wiring or synthetic module names.

## Current Top-Level Map

- `src/lib.rs`
  - Top-level public module surface only.
- `src/engine/`
  - Engine state, edit application, runtime scheduling, persistence helpers, UI event helpers, and engine tests.
- `src/node/`
  - Node traits, built-in node families, dashboards, animation-curve nodes, and typed handle helpers.
- `src/parameter/`
  - Parameter value model, constraints, control state, and the parameter node type.
- `src/script/`
  - Script runtime plus checked-in default templates.
- `src/*.rs`
  - Leaf top-level modules such as `app.rs`, `events.rs`, `process_ctx.rs`, `ui_sync.rs`, and `logger.rs`.

## Rules

- `src/` is the real crate tree. Do not place runtime modules beside `src/` and reattach them with `#[path]`.
- If a concept has children or tests, it is a folder with `mod.rs`.
- Do not keep `thing.rs` beside `thing/` as a steady-state layout.
- Child concepts live physically under the parent that owns them.
- Filenames should match the primary exported concept or a tight concept family.
- Folder context should remove redundant prefixes. Inside `src/engine/`, use `runtime.rs`, not `engine_runtime.rs`.
- `mod.rs` should stay thin: declare children, re-export the public surface, and keep only parent-level orchestration that genuinely belongs there.
- Folder modules keep tests in `tests.rs`.
- Single-file leaf modules may keep inline `mod tests { ... }` until they are promoted to folders for real code-ownership reasons.
- When a leaf module becomes large enough to justify a folder, move its tests out of the runtime body at the same time.

## Lookup Examples

- `DashboardNodeWidgetVec2EditorOptionsNode` -> `src/node/dashboard/widget_options/vec2_editor.rs`
- `DashboardNode` -> `src/node/dashboard/mod.rs`
- `ParameterHandle` -> `src/node/handles/parameter_handle.rs`
- `ProjectFile` -> `src/engine/persistence.rs`
- `EngineRuntimeError` -> `src/engine/runtime.rs`

## Split Guidance

- Split files by ownership and reason to change, not by macro category or by “all structs in one file”.
- When a module starts carrying multiple independent node families, move them into sibling files under the same folder.
- Favor predictable placement over barrel-heavy indirection. Re-exports can flatten the public API, but the disk layout must stay obvious.
