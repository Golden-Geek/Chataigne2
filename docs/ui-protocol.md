# UI Protocol

The current UI protocol source lives on the Rust side in `golden_core` UI DTOs and is consumed by the Svelte client.

## Current State

- Rust DTOs are defined in `submodules/golden_core/crates/core/ui/ui_sync.rs`.
- Raw transport bindings are generated into `src-ui/src/lib/golden_ui/generated/rust_protocol/` during `cargo check` and other normal Rust builds.
- The HTTP transport adapter in `src-ui/src/lib/golden_ui/transport/http.ts` converts those generated Rust-wire types into the UI-local model types in `src-ui/src/lib/golden_ui/types.ts`.
- `types.ts` is now a frontend model layer, not a second source of truth for the wire protocol.

## Rules

- Request, response, event, snapshot, and protocol-version types must have one source of truth.
- DTO changes must update the generator, generated output, and consumers together once generation is in place.
- Do not introduce new manual Rust/TypeScript wire mirrors.
- Keep frontend-only convenience types and normalization logic in the adapter/model layer instead of copying raw protocol declarations.

## Current Consumers

- The built-in HTTP and WebSocket host in `src/app/ui_server.rs`.
- The UI transport client in `src-ui/src/lib/golden_ui/transport/`.
- The UI workbench/session stores through the normalized types in `src-ui/src/lib/golden_ui/types.ts`.