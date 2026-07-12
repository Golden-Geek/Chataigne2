# Phase 0 toolchain contract

`toolchain.json` is the single source for the product-preserving Phase 0 Rust, Node, npm, and Python versions. `rust-version` and `.nvmrc` are checked-in consumers; CI reads the JSON directly for setup action inputs. The Rust installer selects an MSVC host on Windows instead of relying on a developer's rustup default host.

There is deliberately no `rust-toolchain.toml`: that file can pin a version but cannot select different host triples per operating system, and on a Windows machine whose rustup default host is GNU it selected `x86_64-pc-windows-gnu` and broke native builds. The bootstrap therefore installs the manifest's explicit host triple and creates a rustup directory override; Windows always selects MSVC, while macOS and Linux select their recorded native triples.

These versions freeze the existing Phase 0 baseline. Version modernization belongs to Phase 1B and must update the canonical manifest, generated consumers, lockfiles, and all platform evidence together.

Validate file consistency without changing the current machine:

```powershell
pwsh -NoProfile -File ./tools/bootstrap/verify-toolchain.ps1
```

On macOS or Linux, the equivalent commands do not require PowerShell:

```sh
sh ./tools/bootstrap/verify-toolchain.sh
sh ./tools/bootstrap/install-rust-toolchain.sh
```

For one safe bootstrap-and-check command, use:

```powershell
./tools/bootstrap/bootstrap.ps1
```

or:

```sh
sh ./tools/bootstrap/bootstrap.sh
```

The bootstrap installs the pinned Rust host through rustup and downloads the official portable Node distribution into ignored `target/toolchains/` after verifying the SHA-256 recorded from Node's `v24.18.0/SHASUMS256.txt`. It prepends that Node directory only inside the bootstrap process; it never replaces the user's global Node or npm. Pass a command after the wrapper to run it in the selected environment, for example `./tools/bootstrap/bootstrap.ps1 powershell.exe -NoProfile -ExecutionPolicy Bypass -File ./tools/product-gate/product-gate.ps1` or `sh ./tools/bootstrap/bootstrap.sh pwsh -File ./tools/product-gate/product-gate.ps1`.

CI and the product gate also use `-CheckInstalled`/`--check-installed`, which rejects a different Rust, Cargo, Node, npm, or Python version instead of silently testing another toolchain.
