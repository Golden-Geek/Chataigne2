# Controller Paging

This document describes how controller surfaces expose inputs and control, how pages work, and
where the implementation lives.

## 1. Core principles

1. **Input vs control split.** Controller surfaces put *inputs* (read-only physical state)
   under `values/` as a **flat list** (e.g. one `bool` per key for `pressed`), and *device
   control / appearance* (color, text, image, …) under `parameters/`. **Only the control
   side is paged.** Inputs are raw physical state and never page.
2. **Model-driven structure.** A `model` parameter selects the device family and
   regenerates both sides at runtime (key count differs per model). No fixed declaration.
3. **Stable addresses.** The declared/generated control layout is the always-present
   `default` page; its child addresses never move. Derived pages are structural clones with
   **stable ids** — renaming a page changes only its display label, never its addresses.
4. **Opt-in, module-local.** Paging is a per-module capability. Project-wide synchronous
   paging is achieved by driving each module's `active_page` parameter from the global
   Preset/State system.

## 2. Tree shape

`keys` (the default layout) and `pages` (derived pages) are **siblings** on each side. A page
exists under `pages/<id>/` in both `parameters/` (control) and `values/` (inputs), keyed by a
stable id. Keys are **1-based**.

```text
parameters/
  model: Enum                 <- mini / standard / xl / plus / pedal (drives structure)
  brightness: Int
  active_page: Enum = "default"  <- injected selector (Preset-orchestrated)
  keys/                       <- default page control (tag = "pageable")
    key_1/ { color, text, image, unpaged }   <- stable addresses
  pages/                      <- PageHost: "+ New Page" in the UI, standard delete
    lighting/ key_1/ { ... }  <- clone of the default layout, stable id "lighting"
values/
  keys/                       <- default page inputs: key_1: bool, ... (flat)
  pages/                      <- mirror (plain folder; created on demand, removed when empty)
    lighting/ key_1: bool, ...
```

- **`model`** drives the key count and resizes *every* page (default + derived) on both
  sides. Changing the model **adapts** pages (adds/removes keys); it does not reset them.
- **`unpaged`** (per control key): when true, the key always shows the `default` page's
  appearance, so it stays constant across page flips (the "permanent key" concept).
- **Routing**: feedback for slot `i` resolves against the active page's control (or the
  default page if that key is `unpaged`); inbound device events write the active page's input
  (`values/keys/key_i` or `values/pages/<active>/key_i`).

## 3. The generic runtime (`apps/chataigne/src/module/common/paging.rs`)

Device-agnostic. It manages the **control** collection; a module mirrors the pages onto its
`values/` side. The container (`pages`) is a sibling of the default folder (`keys`), so calls
take a `pages_parent` (e.g. `parameters/`) plus the default/template folder separately:

- `ensure_container(ctx, snapshot, pages_parent)` — creates the `PageHost` container.
- `complete_pages(ctx, snapshot, pages_parent, template_folder)` — for any freshly created
  (empty) page, assigns a unique stable id (`short_name`) and clones `template_folder` into it.
- `sync_selector(ctx, snapshot, parameters_folder, pages_parent)` — injects/refreshes the
  `active_page` enum so its options match the existing pages; snaps a dangling selection
  (deleted page) back to `default`.
- `page_descriptors` / `derived_descriptors` — the page list `(id, label)`.
- `mirror_pages(ctx, snapshot, source_pages_parent, mirror_pages_parent, build_keys)` — the
  standard counterpart for controllers that carry both control and inputs: mirrors the derived
  pages onto `values/pages/<id>`, syncing names and removing orphans. Mirror folders are
  **locked** (`NodeUserPermissions::none()`) — their name/existence track the control page and
  are not user-editable.
- `active_page_value` / `active_page_root(default_folder, pages_parent, id)` — read the
  selector and resolve the active page's root node.
- `add_page` / `remove_page` — programmatic page CRUD (also exposed as script methods).

Page creation in the UI goes through `PageHost`
(`apps/chataigne/src/module/common/page_host.rs`), a
user-container that offers a **"+ New Page"** item and accepts folders; deletion is ordinary
node removal. `page_host.rs` is **codegen-only** (not declared in `common/mod.rs`) so it is a
single registered `AppNode` type — see the Design notes.

## 4. Reference module — Stream Deck

The reference implementation lives under
`apps/chataigne/src/module/modules/controllers/streamdeck/`.

- `model` parameter: Mini (6) / Standard·MK.2 (15) / XL (32) / Plus (8) / Pedal (3). Changing
  it resizes every key collection (default + derived, on both sides) to match — pages adapt,
  they are not cleared.
- Control shape per key: `color` (Color), `text` (String), `image` (File), `unpaged` (Bool),
  under `parameters/keys` and `parameters/pages/<id>`. Input per key: a single `bool` under
  `values/keys` and `values/pages/<id>`. The `values/pages` mirror is created when the first
  page is added and removed when the last page is deleted; page names/ids stay in sync.
- Feedback is change-driven (only diffs are pushed to the device). Images composite over the
  key color; transparent image pixels show the color through.
- **Device I/O** goes through the `StreamDeckDevice` trait:
  - `SimulatedStreamDeck` — always compiled; powers the test suite headlessly.
  - Real Elgato hardware via `elgato-streamdeck` behind the `streamdeck-hid` cargo feature
    (off by default to keep engine-only builds free of native `hidapi`/`image`). Requires
    `joycon-rs` on `hidapi 2.x` so only one crate links the native library.

## 5. Design notes (deviations from the original spec)

- **Per-frame string-address resolution → cached node ids + change-driven feedback.** The
  spec rebuilt `format!(".../pages/page_{}/...")` and re-resolved every key every tick. The
  engine is perf-tuned (`needs_update`, `update_requires_tree_snapshot`, `effective_enabled`,
  listener subtrees); resolution is positional/cached and feedback only pushes diffs.
- **`active_page: String` → constrained `Enum`.** A free string let you select a
  non-existent page; the selector is an enum constrained to existing page ids.
- **One declaration, pages derived.** The spec had separate `page_layout_template()` and
  `permanent_layout_template()`. There is a single layout: the `default` page; derived pages
  are clones, and "permanent" is the per-key `unpaged` flag.
- **Typed inbound values.** Inbound activity carries a typed `ParamValue`, not a bare `f32`.
- **Stable page ids.** Pages carry an immutable `short_name` id separate from the editable
  label, so renames never move addresses.
- **Layer placement.** The framework lives in the app (`src/module/common/`), not
  `golden_core`, which stays domain-agnostic.
- **Input/control split (added per review).** Inputs live flat in `values/`; control/appearance
  lives in `parameters/` and is the paged side. This convention is documented for future
  controller modules in `docs/adding-a-node.md`.
