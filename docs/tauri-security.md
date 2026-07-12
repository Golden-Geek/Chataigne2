# Tauri Host Security Posture

## Current State

- `apps/chataigne/tauri.conf.json` currently sets `withGlobalTauri` to `true`.
- `apps/chataigne/tauri.conf.json` currently sets `security.csp` to `null`.
- `apps/chataigne/capabilities/default.json` allows remote development origins on `localhost` and `127.0.0.1`.
- Direct Tauri command access in UI code is concentrated in
  `packages/golden-ui/host/desktop.ts`, which acts as the current host bridge.

## Rules

- Keep direct `window.__TAURI_INTERNALS__` access inside the host bridge.
- UI components and stores should call bridge functions instead of invoking Tauri globals directly.
- Browser-safe code paths must keep working without a desktop host.

## Follow-Up Hardening

`withGlobalTauri` and the null CSP should be tightened after the desktop bridge no longer depends on
global Tauri internals. The intended migration is:

1. Replace global-internals command calls with the supported Tauri JavaScript API or a narrow
   injected bridge owned by the desktop host.
2. Set `withGlobalTauri` to `false`.
3. Add a CSP that permits the shipped app, local development server, websocket transport, and static
   assets needed by the current UI.
4. Keep localhost capability exceptions scoped to development.

Until then, treat new direct Tauri/global usage outside `golden_ui/host/desktop.ts` as a regression.
