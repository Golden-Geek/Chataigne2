# Project Context: Chataigne2

Chataigne2 is the desktop shell and integration layer for the Golden engine and UI stack. It serves as a layered workspace with a thin app shell, shared engine/runtime packages (`golden_core`), and a UI stack (`golden_ui`) that communicates with the engine via explicit protocol boundaries.

## Architecture & Structure
- **App Shell (`src/`)**: Owns app bootstrap, composition, product-level wiring, and app-specific node registration. It relies on the `golden_core` engine for runtime.
- **Core Engine (`submodules/golden_core/`)**: The shared runtime workspace providing explicit crates for engine, protocol, persistence, transport server, desktop host, scripting, and macros.
- **UI Client (`src-ui/`)**: A Svelte 5 frontend shell consuming the `golden_ui` reusable UI package boundary.
- **Protocol Boundary**: UI request, response, event, snapshot, and version types are generated from a canonical Rust protocol definition, avoiding manual duplication in TypeScript.

## Build and Run Commands
### Initial Setup
Run the bootstrap script depending on your platform. This will initialize git submodules, install Rust/Node.js dependencies, and run the app:
- **Windows**: `.\tools\dev.ps1`
- **Linux/macOS**: `bash ./tools/dev.sh`

### Running the App
- **Default Launch**: `cargo run` (embeds the Svelte UI bundle)
- **Live Frontend Dev**: `cargo run -- --dev` (connects to the live Svelte/Vite dev server)
- **Headless (No bundled UI)**: `cargo run -- --no-frontend`

### UI Specific Commands (in `src-ui/`)
- `npm run dev` / `npm run build` / `npm run preview`
- `npm run check` (Svelte Kit sync and type checking)
- `npm run lint` / `npm run format` (Prettier)
- `npm run codegen:golden-ui-protocol` (Updates TypeScript models from Rust protocols)

## Development Conventions
- **Protocol Synchronization**: Do not manually mirror Rust and TypeScript types. Rely on the generated raw transport bindings (`codegen:golden-ui-protocol`).
- **Separation of Concerns**: Avoid placing reusable engine logic, protocol declarations, or persistence formats in the App Shell (`Chataigne2`). These belong in `golden_core`.
- **Documentation**: 
  - Consult `ARCHITECTURE.md` and `docs/architecture.md` for high-level module mapping.
  - See `docs/contributor-map.md` for practical ownership rules.
  - See `CONTRIBUTING.md` for formatting and coding conventions.
  - Engine design notes live in `submodules/golden_core/crates/core/docs/`.