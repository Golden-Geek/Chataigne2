# MapGyver app

This crate owns MapGyver's identity, project policy, frontend composition, and
desktop packaging configuration. Golden owns the engine, persistence, transport,
desktop host, and generic UI. Media and mapping systems will be added here as
app-owned systems through those public boundaries.

The initial project uses Golden's standard empty Dashboard. It contains no media,
composition, screen, or surface nodes yet. The native feasibility probe under
`tools/mapgyver/native-probe` remains a separate diagnostic executable.

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
