# UI Extension Points

Reusable packages expose application-neutral workbench and graph primitives. Chataigne registers
product behavior from `apps/chataigne/ui`; reusable packages never import Chataigne types.

## Ownership

- `packages/golden-ui` owns docking, common panels, inspector controls, context-menu plumbing,
  transport interfaces, and generated Rust protocol bindings.
- `packages/golden-graph-ui` owns graph viewport, selection, slots, wires, and canvas interaction.
- `apps/chataigne/ui/src/lib/systems/state_machine/document` owns the Chataigne state-machine projection
  over `golden_graph_ui`.
- `apps/chataigne/ui/src/lib/systems/alchemist` owns Formula, condition, Processor, Input, Filter,
  and Output presentation.
- `apps/chataigne/ui/src/lib/systems/state_machine` owns State Machine presentation.
- `apps/chataigne/ui/src/lib/panels/modules` owns module-specific editors such as Spatializer.

Add a public registry or component prop at the reusable boundary when an app needs a new inspector,
panel, node renderer, context-menu action, or dashboard widget. Register the implementation from
the app root. Do not add product-name branches to a Golden package or import a package's private
file path.

## Data and intent flow

Rust DTOs are the protocol source of truth. Regenerate bindings after changing them:

```text
npm run codegen:golden-ui-protocol --workspace chataigne-ui
npm run codegen:state-machine-protocol --workspace chataigne-ui
```

UI code may collect input and compute viewport presentation. Backend intents own labels, defaults,
control modes, module policy, graph mutation semantics, and internal parameter writes. Stores
depend on transport interfaces and compose behind a thin workbench session facade.

Use Svelte 5 runes and direct event properties (`onclick`, `onfocus`, and similar). Prefer relative
layout units. After an extension, run `npm run check`, `npm run lint`, `npm test`, and the mounted
browser workflow in the product gate.
