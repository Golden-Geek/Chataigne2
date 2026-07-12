# Stable Watch Workflow

Phase 0 establishes one checked-in development supervisor without changing the existing application entry points:

- `cargo run` continues to launch the bundled product.
- `cargo run -- --dev` continues to use the reusable `golden_core` development-host path.
- `cargo xtask watch` owns the Vite and application process trees and is the stable watch contract.
- `watch`, `watch.cmd`, and `watch.ps1` are thin platform wrappers around the xtask.

## Usage

From the repository root:

```text
cargo xtask watch
cargo xtask watch --headless
cargo xtask watch --frontend-port 15173 --backend-port 17010
cargo xtask watch -- --no-remote
```

The equivalent wrappers are `./watch` on POSIX shells, `.\watch.ps1` in PowerShell, and `watch.cmd` in Command Prompt.

Run `cargo xtask watch --help` for the complete option list. Startup deadlines are bounded and configurable. The defaults allow 60 seconds for Vite, 300 seconds for the Rust build and backend, and 30 seconds for the engine snapshot probe.

The supervisor binds the development services to loopback, sets `GC_UI_FRONTEND_URL` so the host does not start a second Vite process, labels child stdout/stderr, and stops both child process trees on Ctrl+C, application exit, startup failure, or either child failing. A preflight bind check fails with port-owner discovery commands and the relevant port override instead of allowing Vite to silently choose another port.

## Readiness Contract

JSON Lines are written to stdout. Human and child-process logs are written to stderr. Consumers must select events by `event` and `version`; additional fields may be added compatibly.

The terminal startup event is:

```json
{"event":"watch.ready","version":1,"frontend":{"state":"ready","url":"http://127.0.0.1:5173","probe":"http_get_root"},"backend":{"state":"ready","url":"http://127.0.0.1:7010","probe":"http_get_health"},"engine":{"state":"ready","url":"http://127.0.0.1:7010/api/ui/snapshot","probe":"http_post_snapshot"}}
```

The three states deliberately mean different things:

- `frontend` means the Vite root returned a successful HTTP response.
- `backend` means the built-in host returned `{ "ok": true }` from `/api/ui/health`.
- `engine` means `/api/ui/snapshot` returned a valid graph snapshot with a `nodes` array. This proves that the host's engine-backed read model is available; it does not claim that a particular Tauri webview has completed its WebSocket subscription.

Proving a specific webview-to-engine session requires a future public host/UI readiness signal that identifies an active subscribed UI session. Until that boundary exists, automation must not reinterpret `watch.ready` as browser-session parity evidence.
