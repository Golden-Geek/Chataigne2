# Adding A Node

## Where To Work

- Add app-owned nodes under `apps/chataigne/src/` in a cohesive feature subtree such as
  `apps/chataigne/src/module/`, and group concrete modules by family.
- Add reusable engine-level nodes only when they truly belong in `crates/core/src/node/`.
- Keep app shell registration minimal; the node registry is generated from supported node declaration macros.
- Follow the `golden_engine` layout rules in `crates/core/docs/source_layout.md` when adding or moving shared runtime code.

## Flow

1. Declare the node type with the standard Golden node macros.
2. Implement the runtime behavior and persisted state on the node itself.
3. If the node is a user-creatable module, declare `#[item("module", ...)]` next to its `impl Node`.
4. If the node should appear in new projects, wire that through the app lifecycle in
   `apps/chataigne/src/app/bootstrap.rs`.
5. Rebuild so `apps/chataigne/build.rs` regenerates the app node enum and declared-item catalog via
   `golden_codegen_support`.

## Module Catalog

- A concrete module's type id, default label, item kind, and Add menu path belong in that module's file.
- `ModuleManager` consumes the generated declared-item catalog for `module` items; do not add a parallel module descriptor list there.
- Project creation should compare against generated node constants such as `Self::NODE_TYPE`, not a second hand-written type id.

## Add Menu Paths

- Containers expose Add menu entries through `UserCreatableItem`.
- Use `menu_path = ["Generic"]` on declared items, such as `#[item(..., menu_path = ["Generic"])]`, or `menu_path: ["Generic"]` on `define_user_item_factory_methods!` entries when an item belongs under one or more submenus.
- The path describes only submenu labels; the item label remains the final clickable entry.
- Keep fallback structural items, such as `Folder`, after categorized items so the UI can present them at the bottom behind a separator.

## Controller Modules: Inputs vs Control, and Paging

Controller surfaces (Stream Deck, MIDI control wings, Loupedeck, …) follow a consistent split so the engine, scripts, and the paging system can treat them uniformly:

- **`values/` holds inputs only.** Put the read-only physical state of the device here as a flat list of parameters (e.g. one `bool` per key for `pressed`). Do not nest a container per control on the input side — keep it a flat, model-sized list. Inputs are never paged.
- **`parameters/` holds device control / appearance.** Everything the app pushes *to* the device (color, text, image, an `unpaged` flag, …) lives here. This is the side that is **paged**.
- **Model-driven structure.** When a device has families with different control counts, expose a `model` enum parameter and (re)generate both the `parameters/` control folder and the flat `values/` input list from it at runtime, rather than declaring a fixed count.

### Paging

- Mark the control folder pageable so the generic paging runtime
  (`apps/chataigne/src/module/common/paging.rs`) manages it. The declared/generated layout (`keys`)
  is the always-present `default` page; derived pages are structural clones under a `PageHost`
  container (`pages/`, a **sibling** of `keys`) that exposes a "+ New Page" affordance and standard
  delete.
- Mirror the pages onto the input side with `paging::mirror_pages`: a page exists under `pages/<id>/` in both `parameters/` (control) and `values/` (inputs), keyed by a stable id. The mirror is created on demand, removed when the last page is deleted, and its folders are **locked** (name/existence sync to the control page; not user-editable).
- Do not auto-select user-created nodes from the inspector (`UserCreatableItem::with_select_when_created(false)`): keep the inspector on the current node — the new node appears under it anyway.
- A single `active_page` enum parameter (injected into `parameters/`) selects the active page and is drivable project-wide by the Preset/State system.
- Page ids are stable (`short_name`): renaming a page changes only its display label, never its addresses, so links/expressions/dashboard targets keep working.
- When the device has model variants, resize *every* page (default + derived, both sides) on a model change — adapt pages, don't reset them.
- Resolve a key's appearance against the active page; honor a per-key `unpaged` flag by falling back to the `default` page so permanent keys stay constant across page flips. Route inbound device events to the active page's inputs.

## Important Rules

- Do not manually edit generated node registry output.
- Do not path-import private files from `golden_core` to register nodes.
- Keep node APIs scalable for large graphs and deep hierarchies.
