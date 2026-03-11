# Chataigne2 UI

This package contains the Svelte 5 client for Chataigne2. It owns the workbench UI, transport clients, dock and panel composition, and UI-local stores.

## Current Shape

- `src/lib/golden_ui/components/`: panel and reusable UI components.
- `src/lib/golden_ui/store/`: session state, panel state, graph state, and UI-specific stores.
- `src/lib/golden_ui/store/session/`: focused workbench session helpers for selection, warnings,
  and command/file actions.
- `src/lib/golden_ui/transport/`: transport facade plus HTTP and WebSocket transport clients.
- `src/lib/golden_ui/host/`: browser-vs-desktop host bridges for Tauri-specific behavior.
- `src/lib/golden_ui/generated/rust_protocol/`: generated raw wire bindings exported from Rust DTOs.
- `src/lib/golden_ui/dockview/`: panel registration and dock persistence.
- `src/lib/golden_ui/style/`: shared UI styling primitives.
- `src/lib/golden_ui/docs/source_layout.md`: canonical package layout and ownership rules.

## Direction

- Keep Svelte code on Svelte 5 runes only.
- Push session orchestration behind focused stores and a thin facade.
- Keep transport details behind interfaces instead of importing the websocket implementation directly into large state modules. The workbench now depends on the transport facade in `src/lib/golden_ui/transport/index.ts`.
- Keep desktop host calls behind shared host bridges instead of sprinkling `window.__TAURI_INTERNALS__` access through components and stores.
- Treat raw protocol types as generated Rust-owned bindings.
- Keep `src/lib/golden_ui/types.ts` for UI-local normalized model types, not for hand-maintained wire contracts.
- Treat `src/lib/golden_ui/` as package-owned code even while it still lives under the app tree.
- Do not introduce new `$app/*` dependencies inside `golden_ui`.
- Keep internal imports relative so the package stays ready to move out of `src/lib/`.
- Keep `store/workbench.svelte.ts` as a thin facade over `store/session/` instead of letting it
  absorb new concerns.

## Development

```sh
npm run dev
npm run dev:copilot
npm run dev:lan
```

The regular local UI server uses `http://127.0.0.1:5173`.

The dedicated Copilot browser tooling server uses `http://127.0.0.1:4173`.

## Validation

```sh
npm run check
npm run format
npm run smoke:ui
```

The smoke tool writes output to `src-ui/artifacts/`.

## Runtime Probe

Appending `?gc_debug_runtime=1` enables an in-app runtime probe overlay in development. It captures uncaught errors and unhandled promise rejections so runtime failures are visible in the UI.
