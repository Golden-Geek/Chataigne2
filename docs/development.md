# Development Workflows

The repository root owns developer orchestration. Tool versions come from
[`tools/bootstrap/toolchain.json`](../tools/bootstrap/toolchain.json); app and package directories
must not introduce competing bootstrap paths.

## First Setup

Install Git and the Python version recorded in the canonical toolchain manifest. Windows setup
also needs `winget` or preinstalled Visual Studio C++ Build Tools. Linux needs a supported package
manager with `sudo`; macOS needs Xcode Command Line Tools. Then run:

```powershell
.\tools\dev.ps1 -SetupOnly
```

```sh
bash ./tools/dev.sh --setup-only
```

The setup installs the exact Rust host and checksum-verified portable Node distribution selected
for the current platform, verifies Rust/Cargo/Node/npm/Python, installs desktop prerequisites, and
runs root `npm ci` when the lock changed. It never selects floating Rust `stable` or Node LTS.

## Root Commands

| Intent | Command |
| --- | --- |
| Bundled desktop application | `cargo run` |
| Live frontend and supervised backend | `cargo xtask watch` |
| Application connected to an existing Vite server | `cargo run -- --dev` |
| Headless host | `cargo run -- --headless` |
| Rust workspace tests | `cargo test --workspace` |
| UI check, lint, tests, or build | `npm run check`, `npm run lint`, `npm test`, `npm run build` |
| Engine benchmarks | `cargo bench -p golden_engine` |
| Release binary | `cargo build --release` |

Run commands through `tools/bootstrap/bootstrap.ps1` or `tools/bootstrap/bootstrap.sh` when the
pinned Node directory is not already on the current shell's `PATH`. The editor tasks do this
automatically. Debug launchers build the UI first and set `GC_UI_ASSUME_BUILT=1`, so the Rust debug
build consumes the verified artifact rather than finding an arbitrary global Node installation.

## Diagnostics And Qualification

`tools/dev.* --setup-only` is the full host diagnostic. The lighter canonical version check is:

```powershell
.\tools\bootstrap\bootstrap.ps1
```

Ordinary development uses the local current-platform product gate. At a named phase or release
qualification, install the pinned qualification tools and include the dependency profile:

```powershell
.\tools\bootstrap\install-qualification-tools.ps1
.\tools\bootstrap\bootstrap.ps1 powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\tools\product-gate\product-gate.ps1 -DependencyAudit
```

The dependency profile runs RustSec advisory, license, source, and bans policy; an explicit
duplicate-version baseline; unused-dependency analysis; and the npm production audit. Update the
duplicate baseline only after reviewing why every added or removed version is necessary.

## Cache Policy

- `target/toolchains/` caches checksum-verified portable Node archives and installations.
- `target/` holds Rust outputs and local product-gate evidence.
- `node_modules/` is reused only while its internal lock is at least as new as `package-lock.json`.
- CI cache identity includes the canonical toolchain manifest, target, and applicable lockfile.
- Generated UI builds, toolchains, dependency installs, reports, and screenshots are never source
  inputs and remain untracked.

Delete only the affected ignored cache when diagnosing corruption. Do not add machine-local paths,
downloaded SDKs, or generated evidence to version control.
