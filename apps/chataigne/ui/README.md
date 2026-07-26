# Chataigne UI

This Svelte 5 workspace is the Chataigne-specific UI shell. It owns routes, product composition,
module and state-machine panels, branding, and app-only assets. Reusable workbench, audio-device,
and graph-canvas code lives in the root packages `golden_ui`, `golden_audio_ui`, and
`golden_graph_ui`; Alchemist- and Sound Card-specific UI assets and adapters live in this app
workspace.

## Boundaries

- Consume reusable UI through package exports such as `golden_ui`,
  `golden_ui/components/...`, `golden_audio_ui`, and `golden_graph_ui`.
- Keep product DTO adaptation, registry hooks, panels, and assets in this app workspace.
- Register product module editors through
  `src/lib/panels/modules/module-editor-setup.ts`; the shared descriptor registry supplies both
  inspector-header actions and dock panel definitions.
- Keep Svelte code on Svelte 5 runes and direct event props.
- Treat generated state-machine DTOs under `src/lib/systems/state_machine/generated/` as build output.
- Treat generated Sound Card/audio DTOs under
  `src/lib/modules/audio/sound-card/generated/` as build output. Regenerate them with
  `npm run codegen:sound-card-protocol`.
- Do not recreate transport, engine, or module policy in UI code.

## Development

Run workspace commands from the repository root:

```sh
npm ci
npm run check
npm test
npm run build
```

For live UI development use `cargo run -- --dev`, `watch`, or the root `npm run dev` script. The
normal Vite endpoint is `http://127.0.0.1:5173`; the dedicated browser-tool endpoint is
`http://127.0.0.1:4173`.

The browser smoke tool writes app-owned artifacts under `apps/chataigne/ui/artifacts/`.
