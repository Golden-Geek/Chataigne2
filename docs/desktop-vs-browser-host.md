# Desktop Vs Browser Host

## Desktop Host

The desktop host currently lives in the Chataigne2 app shell.

- `src/app/desktop.rs` launches the built-in UI server and the Tauri window.
- `capabilities/default.json` controls the current Tauri remote permissions.
- Native file dialogs are desktop-only behavior and should remain outside pure engine or persistence layers.

## Browser And Headless Host

The current built-in browser/headless path also starts from the app shell.

- `src/app/ui_server.rs` exposes the current HTTP and WebSocket runtime endpoints.
- `--headless` runs the server without launching the Tauri window.
- The long-term direction is a clearer transport-server boundary that can be reused without dragging desktop-only code with it.