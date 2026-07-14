# Chataigne UI

This Svelte 5 workspace is the Chataigne-specific UI shell. It owns routes, product composition,
module and state-machine panels, branding, and app-only assets. Reusable workbench and graph-canvas
code lives in the root packages `golden_ui` and `golden_graph_ui`; Alchemist-specific UI assets and
adapters live in `golden_alchemist_ui`.

## Boundaries

- Consume reusable UI through package exports such as `golden_ui`,
  `golden_ui/components/...`, `golden_graph_ui`, and `golden_alchemist_ui`.
- Keep product DTO adaptation, registry hooks, panels, and assets in this app workspace.
- Keep Svelte code on Svelte 5 runes and direct event props.
- Treat generated state-machine DTOs under `src/lib/state_machine/generated/` as build output.
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
