# Source Layout

`golden_engine` is the engine implementation inside `crates/golden_core/`. A contributor should
be able to locate a concept by following the folder tree without guessing about hidden `#[path]`
wiring or synthetic module names. Foundation types, hosts, services, and build support are sibling
areas under the same Golden Core parent; see the root
[repository layout](../../../../docs/reference/repository-layout.md).

## Current Top-Level Map

- `src/lib.rs`
  - Top-level public module surface only.
- `src/engine/`
  - Engine state, edit application, runtime scheduling, persistence helpers, UI event helpers, and engine tests.
- `src/node/`
  - Node traits, built-in node families, dashboards, curve nodes, and typed handle helpers.
- `src/parameter/`
  - Parameter value model, constraints, control state, animation control nodes, and the parameter node type.
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
- Normalized folders keep tests in sibling `tests.rs` or `*_tests.rs` files.
- Runtime files inside normalized folders do not keep inline `mod tests { ... }` blocks.
- If a module needs sibling runtime files or sibling tests, promote it to a folder instead of keeping a large leaf file.

## Lookup Examples

- `DashboardNodeWidgetVec2EditorOptionsNode` -> `src/node/dashboard/widget_options/vec2_editor.rs`
- `DashboardNode` -> `src/node/dashboard/mod.rs`
- `CurveEasingNode` -> `src/node/curve/easing.rs`
- `curve_from_snapshot` -> `src/node/curve/snapshot.rs`
- `ParameterAnimationControlNode` -> `src/parameter/animation_control.rs`
- `ParameterHandle` -> `src/node/handles/parameter_handle.rs`
- `ProjectFile` -> `src/engine/persistence.rs`
- `EngineRuntimeError` -> `src/engine/runtime.rs`

## Split Guidance

- Split files by ownership and reason to change, not by macro category or by “all structs in one file”.
- When a module starts carrying multiple independent node families, move them into sibling files under the same folder.
- Favor predictable placement over barrel-heavy indirection. Re-exports can flatten the public API, but the disk layout must stay obvious.
