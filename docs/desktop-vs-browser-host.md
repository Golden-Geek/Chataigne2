# Desktop Vs Browser Host

## Desktop Host

The default desktop host lives in `golden_core`.

- `submodules/golden_core/crates/core/app/desktop.rs` launches the built-in UI server and the Tauri window.
- `submodules/golden_core/crates/core/app/desktop_commands.rs` owns the Tauri window commands and native file-dialog commands used by the UI.
- `capabilities/default.json` controls the current Tauri remote permissions.
- Native file dialogs are desktop-only behavior and should remain outside pure engine or persistence layers.
- Apps may override CLI parsing or bootstrap by calling lower-level `golden_core::app` APIs, but they should not need app-shell host files by default.

## Browser And Headless Host

The default built-in browser/headless path also starts from `golden_core`.

- `submodules/golden_core/crates/core/app/ui_server.rs` exposes the current HTTP and WebSocket runtime endpoints.
- `--headless` runs the server without launching the Tauri window.
- Apps can still supply custom bootstrap if they need it, but the reusable default transport server lives in `golden_core`.