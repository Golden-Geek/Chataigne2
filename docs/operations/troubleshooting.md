# Troubleshooting

Start with the supported toolchain and repository diagnostics:

```powershell
.\tools\dev.ps1 -SetupOnly
.\tools\bootstrap\verify-toolchain.ps1 -CheckInstalled
```

```sh
bash ./tools/dev.sh --setup-only
sh ./tools/bootstrap/verify-toolchain.sh --check-installed
```

The commands report version, native desktop prerequisite, generated-data, dependency, and root
target problems. Do not fix a mismatch by switching to floating `stable` toolchains or installing
SDKs inside the repository.

## Build or generated-code failures

- Regenerate Rust-owned UI bindings with the commands in [ui-extension.md](../guides/ui-extension.md).
- Use the root `target/` only. [workspace-hygiene.md](workspace-hygiene.md) documents safe cleanup
  and the generated-data budget.
- Run `cargo fmt --all` before retrying Clippy or CI.

## UI does not connect

For bundled mode, use `cargo run`; it serves the embedded UI and engine together. For live UI
development, use `cargo xtask watch` or `cargo run -- --dev` with Vite. Check that the configured
bind port is free and inspect browser console, failed requests, and WebSocket frames. The product
gate records all three under `target/product-gate/<run>/`.

Use `--no-remote` when diagnosing local exposure. LAN behavior must be tested through a real
non-loopback address; a localhost result is not LAN evidence.

## Projects and persistence

Project writes use atomic replacement, a last-complete backup, and a recovery journal. Do not edit
or delete those files while the app is running. If load recovery appears, preserve the primary,
backup, journal, and product-gate report before retrying. Browser/headless tests isolate their app
data under the gate run directory and remove uploaded fixtures after the workflow.

## Modules and devices

Connection diagnostics belong on the module node. Verify endpoint selection, permissions, queue
warnings, and reconnect state before changing engine timing. Linux controller failures commonly
need udev/device permissions; network modules need a reachable bind/interface and firewall rule.
Use deterministic adapter tests to separate protocol/runtime behavior from physical hardware.

## Qualification failures

Open the JSON report first: required checks are `PASS`, `FAIL`, `BLOCKED`, or `NOT_RUN`, and every
result links its log. Fix the first failed prerequisite; dependent `BLOCKED` rows are expected.
Cross-platform and package results cannot be inferred from another OS. See
[release-readiness.md](release-readiness.md) for native package and soak commands.
