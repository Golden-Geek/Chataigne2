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

## Follow-Up Hardening

The remaining hardening step is a CSP that permits the loopback UI server, websocket transport, and
static assets needed by the current UI without restoring broad global Tauri access. Treat direct
Tauri imports outside `golden_ui/host/desktop.ts` and unreviewed additions to the desktop permission
as regressions.
