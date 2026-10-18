# Tauri Host Security Posture

## Current State

- `withGlobalTauri` is disabled. `packages/golden-ui/host/desktop.ts` uses the supported
  `@tauri-apps/api` package and is the only UI entry point for native host commands.
- `apps/chataigne/permissions/desktop-host.toml` declares the application-owned command surface,
  and `apps/chataigne/capabilities/default.json` grants it only to the `main` window.
- The main webview loads from the loopback UI server in both bundled and development modes, so its
  capability is limited to `localhost` and `127.0.0.1` origins.
- `apps/chataigne/tauri.conf.json` still sets `security.csp` to `null`.

## Rules

- Do not access `window.__TAURI_INTERNALS__` or other private Tauri globals.
- UI components and stores should call the `golden_ui` host bridge instead of importing Tauri APIs
  directly.
- Browser-safe code paths must keep working without a desktop host.
- Add each new desktop command to the explicit app permission only when the main window needs it.

## Remaining boundaries

Tauri capabilities do not authenticate the separate HTTP/WebSocket UI server. The October audit
found privileged project/script endpoints without session/origin admission and wildcard CORS.
Default loopback binding limits network reach but is not a request trust boundary. No browser
exploit or arbitrary-code-execution path was demonstrated by that audit.

The [implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md#p05--establish-transport-trust-while-keeping-lan-convenient)
owns the transport repair, preserving password-free LAN and development use. CSP also remains
open: it must permit the configured UI assets and transport without restoring broad global Tauri
access. Treat direct Tauri imports outside `golden_ui/host/desktop.ts` and unreviewed permission
additions as regressions. Current qualification is recorded only in the
[status ledger](../progress/implementation-status.md).
