# Golden Core UI

Reusable Svelte UI building blocks for Golden Core clients.

This package currently provides:

- transport-agnostic DTO types (`src/lib/types.ts`)
- a deterministic graph cache reducer store (`src/lib/store/graph.ts`)
- a local mock client for protocol-driven UI development (`src/lib/transport/mock.ts`)
- reusable UI components (`src/lib/components`)

`src-ui` consumes these modules via path alias (`$gc-ui`) so app shells can stay thin.
