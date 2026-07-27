# Development Workflows

The repository root owns developer orchestration. Tool versions come from
[`tools/bootstrap/toolchain.json`](../tools/bootstrap/toolchain.json); app and package directories
must not introduce competing bootstrap paths.

## First Setup

Install Git and every system prerequisite recorded in the canonical toolchain manifest: Rust/Cargo,
Node/npm, and Python. Windows also needs Visual Studio C++ Build Tools, Linux needs the Tauri
desktop development packages, and macOS needs Xcode Command Line Tools. Then run:

```powershell
.\tools\dev.ps1 -SetupOnly
```

```sh
bash ./tools/dev.sh --setup-only
```

The setup verifies Rust/Cargo/Node/npm/Python and desktop prerequisites, audits generated workspace
data, and runs root `npm ci` when the lock changed. It never installs a language runtime, mutates a
system package manager, or downloads a portable SDK into the checkout.

### Windows ASIO

ASIO development additionally requires Visual Studio C++ Build Tools and LLVM/Clang with
`libclang.dll`. With those system prerequisites installed, the local probe is one command:

```powershell
.\tools\asio.ps1
```

The wrapper acquires the manifest-pinned
[`audiosdk/asio`](https://github.com/audiosdk/asio) revision into the external per-user tool cache,
validates its CPAL layout, and scopes `CPAL_ASIO_DIR` and `LIBCLANG_PATH` to the child process. It
does not vendor the SDK or permanently mutate the user's environment. Use the same environment for
other commands by placing them after PowerShell's `--` parameter terminator:

```powershell
.\tools\asio.ps1 -- cargo test -p golden_audio --features asio
.\tools\asio.ps1 -- cargo check -p Chataigne2 --features golden_audio/asio
```

Launch Chataigne with ASIO compiled in through the ordinary bootstrap:

```powershell
.\tools\dev.ps1 -Asio
```

An installed vendor ASIO driver is a runtime requirement for devices, not for compilation. Missing
drivers remain a recoverable `MissingDriver` backend state.

## Root Commands

| Intent                                           | Command                                                      |
| ------------------------------------------------ | ------------------------------------------------------------ |
| Bundled desktop application                      | `cargo run`                                                  |
| Live frontend and supervised backend             | `cargo xtask watch`                                          |
| Application connected to an existing Vite server | `cargo run -- --dev`                                         |
| Headless host                                    | `cargo run -- --headless`                                    |
| Rust workspace tests                             | `cargo test --workspace`                                     |
| UI check, lint, tests, or build                  | `npm run check`, `npm run lint`, `npm test`, `npm run build` |
| Engine benchmarks                                | `cargo bench -p golden_engine`                               |
| Release binary                                   | `cargo build --release`                                      |

## Test Placement

Tests stay with their owning subsystem, but always inside a `tests/` directory:

```text
feature/
├── mod.rs
└── tests/
    ├── mod.rs              Rust unit-test module/aggregator
    └── persistence.rs      optional focused suite
```

For a Rust file module, `feature.rs` may own `feature/tests/mod.rs`. Larger feature folders use the
same `feature/tests/` shape. TypeScript and Svelte suites use a local `tests/*.test.ts` directory
beside the source they exercise. Crate-level integration tests and fixtures belong in the crate's
top-level `tests/` directory, with fixtures grouped below `tests/fixtures/` or a more specific name
such as `tests/samples/`.

Do not add inline Rust `mod tests { ... }` blocks, test sources beside runtime files, or `#[path]`
wiring for tests. A reusable cross-crate test API is a `testkit` module rather than a loose
`test_support.rs` file.

Run commands through `tools/bootstrap/bootstrap.ps1` or `tools/bootstrap/bootstrap.sh` to verify the
system toolchain, enforce the single root Cargo target, and run the size audit. The editor tasks do
this automatically. Debug launchers build the UI first and set `GC_UI_ASSUME_BUILT=1`, so the Rust
debug build consumes the verified artifact after the system Node installation passes the contract.

## Diagnostics And Qualification

`tools/dev.* --setup-only` is the full host diagnostic. The lighter canonical version check is:

```powershell
.\tools\bootstrap\bootstrap.ps1
```

Ordinary development uses the local current-platform product gate. For release qualification,
install the pinned qualification tools system-wide, verify them, and include the
dependency profile:

```powershell
.\tools\bootstrap\verify-toolchain.ps1 -CheckQualificationTools
.\tools\bootstrap\bootstrap.ps1 powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\tools\product-gate\product-gate.ps1 -DependencyAudit
```

The dependency profile runs RustSec advisory, license, source, and bans policy; an explicit
duplicate-version baseline; unused-dependency analysis; and the npm production audit. Update the
duplicate baseline only after reviewing why every added or removed version is necessary.

### Live Windows CI diagnosis

Manually dispatch **Cross-platform Product Qualification** on the branch to test, select the
Windows native gate, and enable `windows_debug_session`. The job opens a detached tmate SSH/web
shell before the product gate starts, so the exact hosted runner can be inspected while the real
gate executes. Access is restricted to public SSH keys registered by the GitHub actor who started
the workflow. The session connection details are printed in the job log; terminate the session
when diagnosis is complete so the runner can finish promptly.

Keep this option disabled for normal qualification. The debug path is manual-only, receives no
repository secrets, has read-only repository permissions, and must never be enabled on automatic
pull-request or push events.

## Cache Policy

- The root `target/` is the only Cargo output tree and also holds disposable local product-gate
  evidence. Alternate `target-*` directories are rejected.
- `node_modules/` is reused only while its internal lock is at least as new as `package-lock.json`.
- CI uses the setup-node npm cache, a stable Cargo registry cache, and sccache's content-addressed
  compiler cache. Cache identity includes the canonical toolchain manifest, target, compiler, and
  applicable lockfile; changing application source should not force every dependency to rebuild.
- Generated UI builds, dependency installs, reports, and screenshots are never source inputs and
  remain untracked. Language runtimes and developer tools are installed outside the checkout.

The generated-data budget, audit, and safe cleanup commands are documented in
[`workspace-hygiene.md`](../operations/workspace-hygiene.md). Do not add machine-local paths, downloaded SDKs, or
generated evidence to version control.
