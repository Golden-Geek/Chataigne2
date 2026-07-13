# Product gate runner

Run the Win-x64 iteration product gate from the repository root:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\tools\product-gate\product-gate.ps1
```

On macOS/Linux, or on Windows with PowerShell 7 installed, use `pwsh` in place of
`powershell.exe -ExecutionPolicy Bypass`.

The runner writes one JSON report and a log for every executed command under the
Git-ignored `target/product-gate/<UTC timestamp>/` by default. Override the report location
with `-ReportPath`. Required checks have exactly one of four states: `PASS`, `FAIL`,
`BLOCKED`, or `NOT_RUN`. The process exits with `0`, `1`, `2`, or `3` respectively
for the overall state.

Build and test commands run only after their declared prerequisites pass. A failed
or unexecuted prerequisite makes its dependents `BLOCKED`; the runner never reports
a downstream pass inferred from source inspection.

## Readiness hooks

The gate executes checked-in hooks as explicit required items:

| Hook file | Required evidence |
|---|---|
| `module-loopback-smoke.ps1` | Literal real-binary `phase0.osc-loopback.v1`, schema-v1 result, locked scenario/save-reload digests, command acknowledgement, input value, ordered effects, and no leaked Chataigne process |
| `cargo-run-smoke.ps1` | Literal root `cargo run`, bounded backend/frontend/engine-connected readiness, fixture mutation, and clean process-tree shutdown |
| `watch-smoke.ps1` | Literal root `watch`, distinct frontend/backend/session readiness, restart behavior, fixture mutation, and clean shutdown |
| `cargo-run-dev-smoke.ps1` | Literal root `cargo run -- --dev`, live frontend plus engine connection, fixture mutation, and clean shutdown |
| `ui-workflow.ps1` | Mounted Chataigne application, canonical fixture, graph/inspector/formula/state/value/save-reload workflow, screenshots, and console/network failure capture |
| `lan-browser.ps1` | Real non-loopback advertised address, mounted browser client, engine connection, mutation feedback, resync, and clean shutdown |

Place implemented hooks in `tools/product-gate/hooks/`, or pass another directory
with `-HookDirectory`. A hook is an executable PowerShell script. The runner supplies
`PRODUCT_GATE_REPOSITORY_ROOT`, `PRODUCT_GATE_RUN_DIRECTORY`, and
`PRODUCT_GATE_COMMIT_SHA`. A hook must perform the real assertions, write its own
artifacts below the run directory, clean up every child process and port, and return
nonzero on missing readiness or failed assertions. Merely starting a process is not
a passing readiness check.

The canonical supported Rust, Cargo, Node, npm, and Python versions live in
`tools/bootstrap/toolchain.json`. `tools/bootstrap/rust-version` and `.nvmrc` are verified
consumers. The gate rejects a different installed toolchain before builds run. After `npm ci`,
the gate explicitly installs the Chromium revision selected by the locked `playwright-core`
package; browser evidence never relies on a runner's pre-populated cache.

Pass `-DependencyAudit` at named phase/release qualification points after running the pinned
qualification-tool installer. This adds RustSec advisory, license/source/bans, reviewed duplicate
version, unused dependency, and npm production-audit results. It is opt-in so routine current-host
product checks remain fast and offline-friendly.

## Validation cadence

Ordinary migration supercommits run this gate locally on `x86_64-pc-windows-msvc`. The GitHub
workflow is reserved for the named cross-platform qualification points in the architecture plan,
plus changes to host startup, native dependencies, target selection, packaging, or
platform-specific code. Keeping a long-lived pull request open is not required for local
validation; open a focused PR when a review or qualification point is ready.

Remote platform results remain `NOT_RUN` between qualifications and are never inferred from the
Windows result.

## Cross-platform evidence

The native product matrix is Windows, macOS, and Linux. Each native result is derived
from the real Rust/UI build, launch workflows, browser evidence, and module loopback.
The canonical aggregate additionally requires compatibility compilation for Raspberry
Pi-like Linux ARM hard-float (`armv7-unknown-linux-gnueabihf`), Linux AArch64
(`aarch64-unknown-linux-gnu`), and Windows ARM64 (`aarch64-pc-windows-msvc`). AArch64
Linux and Windows ARM64 build the complete application. The emulated 32-bit runner checks
the reusable engine and Alchemist/statechart crates that form the portable headless Pi
boundary. The app-owned state-machine crate is covered by the native AArch64 application
build; resolving it through the desktop app workspace would pull unrelated hardware
dependencies into the ARMHF check. Native launch, interaction, and loopback evidence stays
with the three executable desktop runners.

Other native platform results remain `BLOCKED` unless reports from those runners are
supplied:

```powershell
pwsh -NoProfile -File .\tools\product-gate\product-gate.ps1 `
  -EvidenceReportPath windows-report.json, macos-report.json, linux-report.json
```

Imported evidence must use schema version 1, match the exact commit, and contain a
`PASS` result with exit code `0` and the exact command for `platform.windows`,
`platform.macos`, or `platform.linux`. External evidence is rejected for a dirty
working tree. CI may restrict one runner invocation with `-RequiredPlatforms`, but a
release/canonical aggregate must require all six native and compatibility platforms.

`.github/workflows/product-gate.yml` runs the complete gate on Windows MSVC,
macOS, and Linux and the compatibility build on the three ARM targets with the exact
canonical toolchain. Every runner uploads its schema-v1 report and logs; native runners
also upload hook artifacts and screenshots. The aggregate accepts exactly one clean
report per platform, requires the exact workflow commit, requires module-loopback
evidence from native runners and a successful `compatibility.compile` result from ARM
runners, and publishes a schema-v1 aggregate. A workflow definition is not passing
evidence: all remote platform results remain `NOT_RUN` until that exact commit is pushed
and the jobs complete.

Use `-PlanOnly` to validate reporting and dependency propagation without executing external
commands. It is deliberately non-passing. `-SkipUiInstall` is accepted only when
`node_modules/.package-lock.json` exists and is at least as new as the committed root package lock;
otherwise the dependency prerequisite fails.
