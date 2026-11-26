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

Cargo uses the single root `target` directory. Development and test profiles disable incremental
compilation, retain only line tables for workspace backtraces, and omit dependency debug information.
This trades some rebuild speed for substantially less disk use. `CARGO_INCREMENTAL=1` can opt in
for a particular session; clear it afterwards. Separate profiles, feature combinations and old
executables still accumulate, so the workspace budget for recognized generated data remains 25 GiB.
The audit is a guard at bootstrap, not a hard filesystem quota on a running Cargo build.

The checked-in rust-analyzer settings also share that target and use a focused `cargo check` during
editing. Workspace-wide Clippy and all-target checks belong to explicit quality gates; running them
on every save both duplicates artifacts and performs unnecessary work.

Run the audit before a long build or qualification:

```powershell
npm run clean:check
```

The audit fails when the budget is exceeded or when it finds another `target-*` tree. The normal
developer bootstrap runs this audit automatically and pins `CARGO_TARGET_DIR` to the canonical root
directory.

For routine cleanup while keeping installed npm dependencies and the prepared MapGyver video SDK:

```powershell
npm run clean
# Preview the exact directories without deleting them:
npm run clean:preview
```

This removes compiled profiles, duplicate nested Cargo targets, qualification output directories
and frontend caches. The next build recompiles Rust and regenerates frontend output. Cleanup
uses Chataigne2's root `target` and leaves the independent `golden/` checkout and its build files alone.
The command refuses candidate directories containing tracked files or reached through filesystem
links. Stop project builds and running apps before cleaning. Do not create a new target directory
for each test or investigation; reuse the root target and remove bulky captures when finished.
An access-denied directory is reported as a cleanup failure and makes the audit incomplete;
resolve its permissions separately before relying on the budget check. Cleanup does not change ACLs.

To also remove npm installs and local virtual environments, use the full reset:

```powershell
powershell -NoProfile -File tools/workspace-hygiene.ps1 -Action Clean
```

Use `-KeepDependencies` to retain `node_modules` and `.venv`. Active `.kilo` worktrees keep their
sources. Their generated folders are reported and are cleaned only with
`-IncludeAgentWorktrees`; use that option only when no agent is building in those worktrees.

Project npm packages still belong in `node_modules` because they are lockfile-defined application
dependencies, not a Node runtime. Recreate them with `npm ci` and clean them when they are no
longer needed.
