# Golden Core UI

Reusable Svelte UI building blocks for Golden Core clients.

This package currently provides:

- transport-agnostic DTO types (`src/lib/types.ts`)
- a deterministic graph cache reducer store (`src/lib/store/graph.ts`)
- an HTTP client for engine-driven sync (`src/lib/transport/http.ts`)
- reusable UI components (`src/lib/components`)

`src-ui` consumes these modules via path alias (`$gc-ui`) so app shells can stay thin.
