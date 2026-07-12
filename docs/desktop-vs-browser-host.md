# Desktop Vs Browser Host

## Desktop Host

The default desktop host lives in `golden_core`.

- `crates/host_desktop/src/lib.rs` is the public desktop-host entry point used through the `golden_core::app` facade.
- `crates/host_desktop/src/desktop.rs` launches the built-in UI server and the Tauri window.
- By default the desktop runtime now points Tauri at the built-in Rust host and serves the shipped frontend bundle from `/`, instead of assuming a separate Vite server is running.
- On Windows, debug builds keep their console output for `cargo run`, while release-style builds are windowed by default and `--show-output` opt-in attaches or creates a console when logs are needed.
- `--dev` swaps the desktop host over to the frontend dev server from `apps/chataigne/ui` so
  `cargo run -- --dev` uses the live Svelte/Vite stack instead of the bundled UI assets.
- `--no-frontend` keeps the Tauri window but disables bundled UI serving, which is useful when you want to start the external frontend yourself.
- `crates/host_desktop/src/desktop_commands.rs` owns the Tauri window commands and native file-dialog commands used by the UI.
- `apps/chataigne/capabilities/default.json` controls the current Tauri remote permissions.
- Native file dialogs are desktop-only behavior and should remain outside pure engine or persistence layers.
- Apps may override CLI parsing or bootstrap by calling lower-level `golden_core::app` APIs, but they should not need app-shell host files by default.

## Browser And Headless Host

The default built-in browser/headless path also starts from `golden_core`.

- `crates/transport_server/src/lib.rs` is the public transport-host entry point used through the `golden_core::app` facade.
- `crates/transport_server/src/ui_server.rs` exposes the current HTTP and WebSocket runtime endpoints and serves any bundled frontend assets provided by the app shell.
- `--headless` runs the server without launching the Tauri window.
- Browser-triggered `Load From...` project imports are handled by the transport host, which currently stores uploaded project JSON files under `~/Documents/Chataigne` before loading them into the live engine.
- Browser-side `Open Remote` and `Save As` remain intentionally unwired until the browser file chooser workflow is designed.
- Apps can still supply custom bootstrap if they need it, but the reusable default transport server lives in `golden_core`.
