# Adding A Node

## Where To Work

- Add app-specific nodes under `src/nodes/`.
- Add reusable engine-level nodes only when they truly belong in the shared runtime workspace.
- Keep app shell registration minimal; the node registry is generated from supported node declaration macros.

## Flow

1. Declare the node type with the standard Golden node macros.
2. Implement the runtime behavior and persisted state on the node itself.
3. If the node should appear in new projects, wire that through the app lifecycle in `src/app/bootstrap.rs`.
4. Rebuild so `build.rs` regenerates the app node enum via `golden_codegen_support`.

## Important Rules

- Do not manually edit generated node registry output.
- Do not path-import private files from `golden_core` to register nodes.
- Keep node APIs scalable for large graphs and deep hierarchies.