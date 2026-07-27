# Supported Toolchain Policy

The canonical toolchain manifest is [`tools/bootstrap/toolchain.json`](../tools/bootstrap/toolchain.json).
Bootstrap scripts, generated version consumers, local product gates, and CI must resolve versions
from that contract instead of maintaining independent pins.

## Supported versions

| Tool | Selected version | Decision |
| --- | --- | --- |
| Rust and Cargo | `1.97.0` | Supported Rust/native toolchain |
| Node.js | `26.5.0` | Supported JavaScript/UI runtime |
| npm | `11.17.0` | Version bundled by the selected Node.js release |
| Python | `3.14.x` | Compatible system interpreter range; patch/security updates do not require parallel installs |
| CPAL | `0.18.1` | Private `golden_audio` host adapter for WASAPI, CoreAudio, ALSA, ASIO, JACK, and native PipeWire |
| asio-sys | `0.3.0` | CPAL's Windows ASIO SDK build integration |
| cargo-deny | `0.20.2` | Pinned release advisory, license, source, and bans qualification |
| cargo-machete | `0.9.2` | Pinned release unused-dependency qualification |
| Windows Rust host | `x86_64-pc-windows-msvc` | Primary local development and iteration target |

The Rust selection is based on the official
[Rust 1.97.0 release](https://blog.rust-lang.org/2026/07/09/Rust-1.97.0/). The Node/npm selection is
the official [Node.js 26.5.0 Current release](https://nodejs.org/en/download/archive/v26.5.0),
which includes npm 11.17.0. Node 26 is not yet LTS; it is selected because the UI toolchain supports
it. Reconsider the pin if product validation exposes ecosystem incompatibility.

The developer bootstrap consumes this manifest directly, verifies system-installed tools, and never
provisions a repository-local runtime. Python accepts compatible `3.14.x` patch updates so security
maintenance does not create parallel installations. Dependency qualification tools are required
system-wide only for release qualification so the online advisory refresh does not slow ordinary
Win-x64 iteration.

TypeScript is pinned to `6.0.3` because SvelteKit 2.69.2 declares support for TypeScript 5 and 6,
not 7. Revisit TypeScript 7 when SvelteKit's peer contract includes it.

The Svelte family follows the manifest emitted by the official Svelte CLI v0.16.3 minimal
TypeScript template rather than a hand-selected combination: Svelte `^5.56.1`, SvelteKit
`^2.63.0`, `@sveltejs/vite-plugin-svelte` `^7.1.2`, Vite `^8.0.16`, Svelte Check `^4.6.0`, and
TypeScript `^6.0.3`. The lock resolves those official ranges to the newest compatible releases.
Chataigne uses the official static adapter instead of the template's auto adapter because the
desktop and remote-browser hosts require the generated static artifact.

## Native audio prerequisites

The ordinary `golden_audio` `desktop` feature compiles the native operating-system host: WASAPI on
Windows, CoreAudio on macOS, and ALSA on Linux. The separately named `full-desktop` qualification
feature adds ASIO and JACK on Windows, JACK on macOS, and JACK, native PipeWire, and real-time DBus
integration on Linux. Native dependencies remain private to `golden_audio`; applications do not
select CPAL features directly.

Windows ASIO builds require the Visual C++ toolchain and LLVM/Clang with `libclang.dll` for bindgen.
`tools/bootstrap/configure-asio-sdk.ps1` fetches the exact official `audiosdk/asio` Git revision
recorded in `toolchain.json`, validates the layout consumed by `asio-sys`, and returns its persistent
external per-user cache path. `tools/asio.ps1` configures that path and `LIBCLANG_PATH` for any child
command; `tools/dev.ps1 -Asio` uses the same path for a local Chataigne run. CI uses the same
resolver and revision with an ephemeral external cache. The SDK remains outside the checkout, and
a missing vendor ASIO driver is a runtime `MissingDriver` state rather than a startup failure.

Linux host qualification requires Clang plus the ALSA, JACK, PipeWire, and DBus development
packages. JACK retains dynamic loading, so a missing JACK client library or server is reported as
`MissingServer`. Native PipeWire is a distinct CPAL host and is probed independently from
PipeWire's JACK compatibility layer. Real-time scheduling refusal is surfaced as structured stream
status and does not abort the application.

Use `cargo run -p golden_audio --example backend_probe` to inspect compiled native hosts without
opening a stream. On Windows, `tools/asio.ps1` runs that probe with the ASIO feature by default; use
`--features full-desktop` only in an environment that has all platform prerequisites above. The
default remains the external-prerequisite-free native desktop path so a clean developer checkout
can run after the ordinary workspace bootstrap; release qualification is responsible for the full
host set.

## Upgrade Boundaries

- Change one coherent family at a time: Rust/native, JavaScript/UI, then developer orchestration.
- Update the canonical manifest and every checked-in consumer in the same change.
- Run the applicable local gate before other work builds on the update.
- Run the full cross-platform qualification before accepting a toolchain update. Earlier online
  runs are required when a change affects host startup, native dependencies, target selection,
  packaging, or platform-specific code.
- Record remote platforms as `NOT_RUN` until their exact-commit reports actually pass.
- Prefer supported LTS/stable toolchains and cross-platform dependencies. A newer release is not
  adopted solely because it exists; its release notes, native compatibility, and product gate
  must justify the update.
- Cache identity must include `toolchain.json` and the relevant Rust or npm lock. Build outputs,
  dependency installs, and gate evidence live only under the bounded ignored paths described in
  [workspace hygiene policy](../operations/workspace-hygiene.md); portable SDKs are not stored in the
  checkout.
