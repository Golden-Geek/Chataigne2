# Adding A Node

## Where To Work

- Add app-owned nodes under `src/` in a cohesive feature subtree such as `src/module/`, and keep concrete module implementations grouped under their family directories like `src/module/modules/protocol/osc/`.
- Add reusable engine-level nodes only when they truly belong in `submodules/golden_core/crates/core/src/node/`.
- Keep app shell registration minimal; the node registry is generated from supported node declaration macros.
- Follow the `golden_engine` layout rules in `submodules/golden_core/crates/core/docs/source_layout.md` when adding or moving shared runtime code.

## Flow

1. Declare the node type with the standard Golden node macros.
2. Implement the runtime behavior and persisted state on the node itself.
3. If the node is a user-creatable module, declare `#[item("module", ...)]` next to its `impl Node`.
4. If the node should appear in new projects, wire that through the app lifecycle in `src/app/bootstrap.rs`.
5. Rebuild so `build.rs` regenerates the app node enum and declared-item catalog via `golden_codegen_support`.

## Module Catalog

- A concrete module's type id, default label, item kind, and Add menu path belong in that module's file.
- `ModuleManager` consumes the generated declared-item catalog for `module` items; do not add a parallel module descriptor list there.
- Project creation should compare against generated node constants such as `Self::NODE_TYPE`, not a second hand-written type id.

## Add Menu Paths

- Containers expose Add menu entries through `UserCreatableItem`.
- Use `menu_path = ["Generic"]` on declared items, such as `#[item(..., menu_path = ["Generic"])]`, or `menu_path: ["Generic"]` on `define_user_item_factory_methods!` entries when an item belongs under one or more submenus.
- The path describes only submenu labels; the item label remains the final clickable entry.
- Keep fallback structural items, such as `Folder`, after categorized items so the UI can present them at the bottom behind a separator.

## Important Rules

- Do not manually edit generated node registry output.
- Do not path-import private files from `golden_core` to register nodes.
- Keep node APIs scalable for large graphs and deep hierarchies.
