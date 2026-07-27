# Supported workspace toolchain contract

`toolchain.json` is the single source for the supported Rust, Node, npm, Python, and release
qualification-tool versions.
`rust-version` and `.nvmrc` are checked-in consumers; CI reads the JSON directly for setup action
inputs. The Rust installer selects an MSVC host on Windows instead of relying on a developer's
rustup default host when it provisions an ephemeral CI runner.

There is deliberately no `rust-toolchain.toml`: that file can pin a version but cannot select
different host triples per operating system. Developers install the recorded toolchain system-wide
and select it with a rustup directory override. The bootstrap verifies that contract; it does not
install or download tools into the checkout.

Update one coherent toolchain family at a time. Each update changes the canonical manifest and all
generated consumers together, passes the local gate, and receives cross-platform qualification.
Selection and upgrade policy are recorded in [`docs/reference/toolchain.md`](../../docs/reference/toolchain.md).

Validate file consistency without changing the current machine:

```powershell
pwsh -NoProfile -File ./tools/bootstrap/verify-toolchain.ps1
```

On macOS or Linux, the equivalent commands do not require PowerShell:

```sh
sh ./tools/bootstrap/verify-toolchain.sh
```

For one safe bootstrap-and-check command, use:

```powershell
./tools/bootstrap/bootstrap.ps1
```

or:

```sh
sh ./tools/bootstrap/bootstrap.sh
```

The bootstrap verifies system-installed Rust, Cargo, Node, npm, and Python, pins Cargo output to the
single root `target`, and runs the workspace-size audit. Pass a command after the wrapper to run it
in the verified environment, for example `./tools/bootstrap/bootstrap.ps1 powershell.exe -NoProfile
-ExecutionPolicy Bypass -File ./tools/product-gate/product-gate.ps1` or
`sh ./tools/bootstrap/bootstrap.sh pwsh -File ./tools/product-gate/product-gate.ps1`.

Windows ASIO development uses the official `audiosdk/asio` source and the exact Git revision
recorded in `toolchain.json`. The standalone resolver installs it into a persistent external
per-user cache and returns the directory expected by `asio-sys`:

```powershell
$env:CPAL_ASIO_DIR = ./tools/bootstrap/configure-asio-sdk.ps1 -PassThru
cargo test -p golden_audio --features asio
```

For a complete local build environment, `./tools/asio.ps1` also validates Visual C++ and
LLVM/Clang, configures both native build variables for its child command, and runs the backend probe
by default. Pass a custom command after PowerShell's `--` parameter terminator:

```powershell
./tools/asio.ps1 -- cargo test -p golden_audio --features asio
```

Ordinary Windows Chataigne builds include ASIO, so `./tools/dev.ps1` performs this setup by default.
`-FullAudioHosts` additionally enables JACK and Windows real-time priority support. No SDK files
are written into the checkout.

CI and the product gate also use `-CheckInstalled`/`--check-installed`, which rejects a different Rust, Cargo, Node, npm, or Python version instead of silently testing another toolchain.

`install-rust-toolchain.*` and `install-qualification-tools.*` are CI provisioning helpers for
ephemeral runners only. Developer tasks require the manifest-pinned `cargo-deny` and
`cargo-machete` commands to be installed system-wide. See
[`docs/operations/workspace-hygiene.md`](../../docs/operations/workspace-hygiene.md) for installation and cleanup policy.
