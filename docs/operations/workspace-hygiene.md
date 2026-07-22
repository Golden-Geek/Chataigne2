# Workspace hygiene

The checkout contains source, manifests, and reproducible configuration. Compilers, language
runtimes, package-manager caches, and downloaded browsers are system dependencies or disposable
build data; they must not be embedded as repository-local toolchains.

## System prerequisites

Install the versions recorded in `tools/bootstrap/toolchain.json` and keep them on `PATH`:

- Rust and Cargo through a system `rustup` installation. Install toolchain `1.97.0` with the
  `rustfmt` and `clippy` components, then select it for this checkout with `rustup override set
  1.97.0`.
- Node `26.5.0` and npm `11.17.0` through a system Node installer or version manager.
- Python `3.14.x` as a system installation. Repository scripts use the standard library and do not
  require a project virtual environment.
- Visual Studio C++ Build Tools on Windows, Xcode Command Line Tools on macOS, or the platform
  packages from the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) on Linux.
- PowerShell 7 (`pwsh`) on macOS/Linux for the shared product gate and hygiene audit.
- `cargo-deny 0.20.2` and `cargo-machete 0.9.2` on machines that run qualification.

The `install-rust-toolchain.*` and `install-qualification-tools.*` scripts are retained only for
ephemeral CI runners. Developer bootstrap scripts verify the installed system toolchain and never
call those installers.

## Generated data policy

Cargo always uses the single root `target` directory. The development and test profiles disable
incremental compilation and dependency debug information to prevent feature/profile variants from
accumulating hundreds of gigabytes. The workspace budget for all recognized generated data is
25 GiB.

The checked-in rust-analyzer settings also share that target and use a focused `cargo check` during
editing. Workspace-wide Clippy and all-target checks belong to explicit quality gates; running them
on every save both duplicates artifacts and performs unnecessary work.

Run the audit before a long build or qualification:

```powershell
powershell -NoProfile -File tools/workspace-hygiene.ps1 -Action Audit
```

The audit fails when the budget is exceeded or when it finds another `target-*` tree. The normal
developer bootstrap runs this audit automatically and pins `CARGO_TARGET_DIR` to the canonical root
directory.

Remove all reproducible build output, npm installs, Python caches, and local virtual environments:

```powershell
powershell -NoProfile -File tools/workspace-hygiene.ps1 -Action Clean
```

Use `-KeepDependencies` to retain `node_modules` and `.venv`. Active `.kilo` worktrees are never
deleted. Their generated folders are reported but are cleaned only with
`-IncludeAgentWorktrees`; use that option only when no agent is building in those worktrees.

Project npm packages still belong in `node_modules` because they are lockfile-defined application
dependencies, not a Node runtime. Recreate them with `npm ci` and clean them when they are no
longer needed.
