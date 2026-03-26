# Adding A Node

## Where To Work

- Add app-owned nodes under `src/` in a cohesive feature subtree such as `src/module/`, and keep concrete module implementations grouped under their family directories like `src/module/modules/protocol/osc/`.
- Add reusable engine-level nodes only when they truly belong in `submodules/golden_core/crates/core/src/node/`.
- Keep app shell registration minimal; the node registry is generated from supported node declaration macros.
- Follow the `golden_engine` layout rules in `submodules/golden_core/crates/core/docs/source_layout.md` when adding or moving shared runtime code.

## Flow

1. Declare the node type with the standard Golden node macros.
2. Implement the runtime behavior and persisted state on the node itself.
3. If the node should appear in new projects, wire that through the app lifecycle in `src/app/bootstrap.rs`.
4. Rebuild so `build.rs` regenerates the app node enum via `golden_codegen_support`.

## Important Rules

- Do not manually edit generated node registry output.
- Do not path-import private files from `golden_core` to register nodes.
- Keep node APIs scalable for large graphs and deep hierarchies.
