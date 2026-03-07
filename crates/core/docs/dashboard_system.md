# Dashboard System

The dashboard system is modeled entirely as engine nodes so project state, persistence, scripting, undo history, and UI synchronization all flow through the existing Golden node runtime.

## Node Hierarchy

- `dashboard`
  - Root dashboard container.
  - Creates `dashboard_page` user items.
- `dashboard_page`
  - Top-level page or view inside a dashboard.
  - Creates `dashboard_widget_container`, `dashboard_node_widget`, and `dashboard_generic_widget` user items.
- `dashboard_widget_container`
  - Nested layout widget.
  - Creates the same dashboard widget item family recursively.
- `dashboard_node_widget`
  - Widget bound to one arbitrary node reference.
  - Intended to render an automatically generated inspector-like UI for the target node.
- `dashboard_generic_widget`
  - Widget bound primarily to parameter references and optional script/context children.
  - Intended for canonical controls such as text, button, slider, text input, and checkbox.

## Item Kinds

- `dashboard`
  - Reserved for dashboard roots.
- `dashboard_page`
  - Reserved for dashboard pages.
- `dashboard_widget`
  - Shared kind for all page/container widget children.

## Declarative State

Dashboard layout and widget state is stored as declared child parameters on the owning node.
This keeps the schema visible to:

- persistence codecs
- UI sync snapshots
- inspector rendering
- scripting APIs
- undo/redo transactions

Current direct parameters cover:

- page layout strategy and grid density
- widget position, size, and span hints
- node-widget target node references and display mode
- generic-widget kind, text/value configuration, and target parameter references

Sequential widget ordering is not stored as a separate parameter. Widget order is the actual sibling order under each page or container, so reorder operations flow through normal node move history and persistence.

## Binding Contract

- `dashboard_node_widget.target_node`
  - `ReferenceTargetKind::AnyNode`
  - Supports drag-and-drop or picker binding to any engine node.
- `dashboard_generic_widget.target_param`
  - `ReferenceTargetKind::ParameterOnly`
  - Supports binding to canonical parameter nodes for control/display widgets.

## UI Implications

The engine now exposes dashboard pages and widgets through the existing user-item catalog APIs. This means:

- context-menu creation can use the current catalog flow immediately
- outliner drag-and-drop can create or rebind widgets without inventing a separate persistence model
- inspector label drag-and-drop can target the existing node or parameter reference fields

The remaining work is UI behavior:

- dashboard panel/view rendering
- drag-and-drop authoring interactions
- automatic rendering for `dashboard_node_widget`
- specialized Svelte renderers for `dashboard_generic_widget`