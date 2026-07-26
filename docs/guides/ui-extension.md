# UI Extension Points

Reusable packages expose application-neutral workbench and graph primitives. Chataigne registers
product behavior from `apps/chataigne/ui`; reusable packages never import Chataigne types.

## Ownership

- `packages/golden-ui` owns docking, common panels, inspector controls, context-menu plumbing,
  transport interfaces, and generated Rust protocol bindings.
- `packages/golden-audio-ui` owns reusable audio device selection, status/error presentation,
  generated audio DTOs, and the generic application binding.
- `packages/golden-graph-ui` owns graph viewport, selection, slots, wires, and canvas interaction.
- `apps/chataigne/ui/src/lib/systems/state_machine/document` owns the Chataigne state-machine projection
  over `golden_graph_ui`.
- `apps/chataigne/ui/src/lib/systems/alchemist` owns Formula, condition, Processor, Input, Filter,
  and Output presentation.
- `apps/chataigne/ui/src/lib/systems/state_machine` owns State Machine presentation.
- `apps/chataigne/ui/src/lib/panels/modules` owns module-specific editors such as Spatializer and
  Sound Card.

Add a public registry or component prop at the reusable boundary when an app needs a new inspector,
panel, node renderer, context-menu action, or dashboard widget. Register the implementation from
the app root. Do not add product-name branches to a Golden package or import a package's private
file path.

Chataigne module editors use the app-owned descriptor registry in
`module-editor-registry.ts`. Add the descriptor to `module-editor-setup.ts`; the same descriptor
then drives the module-inspector action, stable per-module panel identity, title, icon, and
`+page.svelte` panel definition. Do not add module-type conditionals to the inspector header or
move this product registry into `golden_ui`.

## Data and intent flow

Rust DTOs are the protocol source of truth. Regenerate bindings after changing them:

```text
npm run codegen:golden-ui-protocol --workspace chataigne-ui
npm run codegen --workspace golden_audio_ui
npm run codegen:state-machine-protocol --workspace chataigne-ui
npm run codegen:sound-card-protocol --workspace chataigne-ui
```

UI code may collect input and compute viewport presentation. Backend intents own labels, defaults,
control modes, module policy, graph mutation semantics, and internal parameter writes. Stores
depend on transport interfaces and compose behind a thin workbench session facade.

Sound Card matrix creation uses one `CreateUserItem` intent with backend-recognized source,
destination, and gain `initial_params`; gain changes use `SetParam`, removal uses `RemoveNode`, and
pointer painting is enclosed in `BeginEdit`/`EndEdit`. Canvas renders the dense matrix and packed
telemetry, while a sparse DOM route list and focused controls preserve keyboard and assistive
access without mounting one component per matrix cell.

Use Svelte 5 runes and direct event properties (`onclick`, `onfocus`, and similar). Prefer relative
layout units. After an extension, run `npm run check`, `npm run lint`, `npm test`, and the mounted
browser workflow in the product gate.
