# Golden Core UI

Reusable Svelte UI building blocks for Golden Core clients.

This package currently provides:

- transport-agnostic DTO types (`src/lib/types.ts`)
- rune-native graph cache + workbench session stores (`src/lib/store/*.svelte.ts`)
- an HTTP client for engine-driven sync (`src/lib/transport/http.ts`)
- reusable UI components (`src/lib/components`)

`ConnectedWorkbench.svelte` is the default entry point for host apps.
It owns connection bootstrap, graph sync, undo/redo and keyboard history shortcuts.
Panels consume shared state through workbench context so new panels can be added with minimal wiring.

`src-ui` consumes these modules via path alias (`$gc-ui`) so app shells can stay thin.
