# Desktop Vs Browser Host

## Desktop Host

The default desktop host lives in `golden_core`.

- `crates/golden_core/hosts/desktop/src/lib.rs` is the public desktop-host entry point used through the `golden_core::app` facade.
- `crates/golden_core/hosts/desktop/src/desktop.rs` launches the built-in UI server and the Tauri window.
- By default the desktop runtime points Tauri at the built-in Rust host and serves the shipped frontend bundle from `/`; no separate Vite server is required.
- On Windows, debug builds keep their console output for `cargo run`, while release-style builds are windowed by default and `--show-output` opt-in attaches or creates a console when logs are needed.
- `--dev` swaps the desktop host over to the frontend dev server from `apps/chataigne/ui` so
  `cargo run -- --dev` uses the live Svelte/Vite stack instead of the bundled UI assets.
- `--no-frontend` keeps the Tauri window but disables bundled UI serving, which is useful when you want to start the external frontend yourself.
- `crates/golden_core/hosts/desktop/src/desktop_commands.rs` owns the Tauri window commands and native file-dialog commands used by the UI.
- `apps/chataigne/capabilities/default.json` controls the current Tauri remote permissions.
- Native file dialogs are desktop-only behavior and should remain outside pure engine or persistence layers.
- Apps may override CLI parsing or bootstrap by calling lower-level `golden_core::app` APIs, but they should not need app-shell host files by default.

## Browser And Headless Host

The default built-in browser/headless path also starts from `golden_core`.

- `crates/golden_core/hosts/transport/src/lib.rs` is the public transport-host entry point used through the `golden_core::app` facade.
- `crates/golden_core/hosts/transport/src/ui_server.rs` exposes the current HTTP and WebSocket runtime endpoints and serves any bundled frontend assets provided by the app shell.
- `--headless` runs the server without launching the Tauri window.
- Browser-triggered `Load From...` project imports are handled by the transport host, which currently stores uploaded project JSON files under `~/Documents/Chataigne` before loading them into the live engine.
- Browser-side `Open Remote` and `Save As` remain intentionally unwired until the browser file chooser workflow is designed.
- Apps can still supply custom bootstrap if they need it, but the reusable default transport server lives in `golden_core`.

## I/O Capacity And Overflow

The reusable I/O layer owns admission mechanics; each adapter supplies limits that match its
payloads. Ordered commands are never coalesced or silently discarded.

| Boundary | Item limit | Retained-weight limit | Overflow policy | Service turn |
| --- | ---: | ---: | --- | ---: |
| Generic pending events | 4,096 | 4 MiB caller weight | Return `Full` with ownership; count rejection | Consumer supplied |
| Generic worker commands | 256 | Fixed-size commands | Return `TrySendError::Full` | Worker supplied |
| Authoritative control actor | 1,024 | One typed operation per item | Return `ControlErrorKind::Overloaded` | One operation, then recheck shutdown |
| Generation compiler | 1 pending + 1 in flight | One immutable snapshot per generation | Replace pending; mark stale in flight; bounded completion queue | One generation |
| OSC output commands | 256 | One UDP message per item | Reject synchronously with an overload error | 256 commands |
| OSC input events | 2,048 | 2 MiB decoded datagram weight | Reject new datagram event; count rejection | 256 datagrams / 1,024 events |
| Serial input events | 2,048 | 2 MiB received bytes | Reject new read event; count rejection | 1,024 events |

Readiness is set only after successful admission and remains armed after a partial drain. Dynamic
payload adapters must call the weighted send API; fixed-size status events may use unit weight.
Compiler implementations receive a cooperative staleness token and must check it between expensive
materialization stages. Superseded pending snapshots are returned to the engine immediately for
retirement; stale in-flight results are reported but can never become the live generation.
