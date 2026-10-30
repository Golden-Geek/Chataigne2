# MapGyver app

This crate owns MapGyver's identity, project policy, frontend composition, and
desktop packaging configuration. Golden owns the engine, persistence, transport,
desktop host, and generic UI. App-owned media contracts and rendering policy
live under `src/media`; mapping and composition systems build on those boundaries.

The initial project contains a Media Manager and Golden's standard empty Dashboard.
The primary app panel creates Solid Color media with backend-owned defaults and
edits independent channel color/alpha, automatic/fixed size and sampling. Media and
channels can be renamed, channels reordered, and media/channels removed through the
same history-aware app operations used by scripts and headless clients. Undo/redo,
project replacement and reconnection refresh the panel without polling.

`src/media/protocol.rs` owns those DTOs; `generate_media_protocol` generates the
frontend contracts. `src/media/catalog.rs` provides ordered, read-only inspection
in one media/channel traversal. The UI under `ui/src/lib/media` registers through
Golden's public panel API and consumes its public workbench session facade. It
does not allocate identities, choose backend defaults or mutate node internals.
App invariants run at the detached candidate-validation boundary, after queued
default/load edits attach and before live activation, for both startup and replacement.

This first authoring panel does not yet provide native channel previews or Picture/
Shared Texture source editors. Module command nodes and app-owned script snippets
also remain part of M04. See the [active acceptance ledger](../../docs/progress/mapgyver-status.md#m04-acceptance-items).
The native feasibility probe under `tools/mapgyver/native-probe` remains a separate
diagnostic executable; it is not the app's rendering implementation.

Run these commands from the workspace root:

| Command | Purpose |
| --- | --- |
| `npm run run:mapgyver` | Launch the bundled desktop app. |
| `npm run watch:mapgyver` | Run the shared development watcher for MapGyver. |
| `npm run dev:mapgyver` | Start only the frontend development server. |
| `npm run build:mapgyver` | Build the frontend. |
| `npm run check:mapgyver` | Check frontend types and Svelte components. |
| `npm run package:mapgyver` | Build the desktop bundles from MapGyver's app directory. |
| `npm run package:check:mapgyver` | Compile through the packaging path without creating bundles. |

`cargo run -p MapGyver -- --dev` launches the app with its frontend development
server. Use `cargo run -p MapGyver -- --headless` for the shared transport without
a window. The packaging commands retain the workspace signing prerequisite.
MapGyver defaults to backend port `7011` and frontend development port `5174`;
`GC_UI_BIND` and `GC_UI_FRONTEND_URL` retain the shared host's explicit overrides.
Unqualified root commands continue to select Chataigne.

`build.rs` selects the `mapgyver-ui` npm workspace through
`golden_codegen_support::ui_bundle`; it embeds the resulting assets into the
app's Cargo output directory. The frontend discovers and validates the backend's
MapGyver metadata before creating a workbench session or accessing app storage.

The editable icon is `ui/static/favicon.svg`. Regenerate native icon exports with
`npm exec -- tauri icon apps/mapgyver/ui/static/favicon.svg --output apps/mapgyver/icons`
from the workspace root; the Windows app tracks and uses only `icon.png` and `icon.ico`.

The shared host's optional native-presentation seam has an explicit Windows lifecycle smoke:
`cargo run --locked -p MapGyver --features native-presentation-smoke --bin native_presentation_smoke`.
Set `APPDATA` for that child process to an owned test directory, and unset `GC_UI_BIND` and
`GC_UI_FRONTEND_URL`. It opens/closes only its own window and uses a separate application ID.
Default tests do not launch it. This checks UI/worker/window lifetime, without GPU qualification.

The production renderer's owned Q01/Q02 runner lives under `tests/native_qualification` and
uses Golden's shared host/runtime with independent GPU source and consumer processes. Build it
with `--features native-qualification --bin native_qualification`; see the
[runner contract](../../docs/progress/mapgyver-m03-qualification-runner.md) for commands,
observations and qualification limits. Short diagnostic controls do not close native quality gates.
