# Supported Toolchain Policy

The canonical toolchain manifest is [`tools/bootstrap/toolchain.json`](../../tools/bootstrap/toolchain.json).
Bootstrap scripts, generated version consumers, local product gates, and CI must resolve versions
from that contract instead of maintaining independent pins.

## Phase 1B Selection

| Tool | Selected version | Decision |
| --- | --- | --- |
| Rust and Cargo | `1.97.0` | Current stable release on 2026-07-13; selected for the Rust/native Phase 1B slice |
| Node.js | `26.5.0` | Latest Current release on 2026-07-13; selected for the JavaScript/UI tooling slice |
| npm | `11.17.0` | Version bundled by the selected Node.js release |
| Python | `3.14.4` | Existing bootstrap/codegen pin; review belongs to the developer-orchestration slice |
| cargo-deny | `0.20.2` | Pinned phase/release advisory, license, source, and bans qualification |
| cargo-machete | `0.9.2` | Pinned phase/release unused-dependency qualification |
| Windows Rust host | `x86_64-pc-windows-msvc` | Primary local development and iteration target |

The Rust selection is based on the official
[Rust 1.97.0 release](https://blog.rust-lang.org/2026/07/09/Rust-1.97.0/). The Node/npm selection is
the official [Node.js 26.5.0 Current release](https://nodejs.org/en/download/archive/v26.5.0),
which includes npm 11.17.0. Node 26 is not yet LTS; it is selected deliberately because Phase 1B
is the modernization window and the UI toolchain supports it. Phase qualification must reconsider
the pin if product validation exposes ecosystem incompatibility.

The developer bootstrap now consumes this manifest directly; the former floating `stable`,
`stable-msvc`, Node LTS, and broad minimum-version checks were removed. Python 3.14.4 remains the
supported codegen/bootstrap interpreter after the developer-orchestration review. Dependency
qualification tools are intentionally installed only for phase/release closure so the online
advisory refresh does not slow ordinary Win-x64 iteration.

TypeScript is pinned to `6.0.3` even though npm publishes `7.0.2`: SvelteKit 2.69.2 declares
support for TypeScript 5 and 6, not 7. This is a supported-version ceiling rather than a
compatibility shim. Revisit TypeScript 7 when SvelteKit's peer contract includes it.

The Svelte family follows the manifest emitted by the official Svelte CLI v0.16.3 minimal
TypeScript template rather than a hand-selected combination: Svelte `^5.56.1`, SvelteKit
`^2.63.0`, `@sveltejs/vite-plugin-svelte` `^7.1.2`, Vite `^8.0.16`, Svelte Check `^4.6.0`, and
TypeScript `^6.0.3`. The lock resolves those official ranges to the newest compatible releases.
Chataigne uses the official static adapter instead of the template's auto adapter because the
desktop and remote-browser hosts require the generated static artifact.

## Upgrade Boundaries

- Change one coherent family at a time: Rust/native, JavaScript/UI, then developer orchestration.
- Update the canonical manifest and every checked-in consumer in the same change.
- Run the applicable Win-x64 local gate before another architecture slice builds on the update.
- Run the full cross-platform qualification before Phase 1B closes. Earlier online runs are
  required only when a change affects host startup, native dependencies, target selection,
  packaging, or platform-specific code.
- Record remote platforms as `NOT_RUN` until their exact-commit reports actually pass.
- Prefer supported LTS/stable toolchains and cross-platform dependencies. A newer release is not
  adopted solely because it exists; its migration notes, native compatibility, and product gate
  must justify the update.
- Cache identity must include `toolchain.json` and the relevant Rust or npm lock. Local portable
  SDKs, build outputs, dependency installs, and gate evidence live only under ignored cache/output
  directories described in [`docs/development.md`](../development.md).
