# Repo Transition Plan

`Chataigne2` is moving from a mixed app-plus-submodules checkout to an app-shell repo that consumes shared SDKs through explicit public boundaries.

## Canonical Ownership

- `golden_core`: canonical Rust SDK repo. Owns engine/runtime crates, protocol DTOs, persistence DTOs, transport hosting, desktop hosting, macros, and build-time support crates.
- `golden_ui`: canonical reusable UI SDK repo/package. Owns reusable Svelte UI components, stores, transport adapters, and generated Rust protocol bindings consumed by UI clients.
- `Chataigne2`: app repo only. Owns app-specific nodes, branding/assets, shell bootstrap, product wiring, capabilities, and app-local UI additions.

## What Stays In `Chataigne2`

- `src/`: app node registration, lifecycle wiring, and product bootstrap.
- `src-ui/src/routes`: app entry routes and composition.
- `src-ui/src/lib/assets`: product-specific assets and icon overrides.
- `capabilities/`, `icons/`, app manifests, and app-level docs.
- Root `build.rs`: app-local node registry generation only.

## What Must Leave `Chataigne2`

- Canonical shared Rust engine/runtime code.
- Desktop host implementation.
- Shared transport server implementation.
- Canonical protocol DTO ownership.
- Shared generated UI protocol artifacts owned by the UI SDK.
- Reusable Svelte session/store framework that is not app-specific.

## Current Vs Target Map

Current:

- `src/` mixes a thin shell with direct shared-SDK assumptions.
- `src-ui/src/lib/golden_ui/` contains the reusable UI SDK inside the app tree.
- `submodules/golden_core/` is consumed directly by path dependency.
- The app repo previously wrote shared UI protocol output during `build.rs`.

Target:

- `Chataigne2` consumes `golden_core` through stable public crates and versioned dependency policy.
- `src-ui` consumes `golden_ui` as a package dependency instead of treating it as app-local source.
- Protocol generation is owned by the shared UI package workflow, not the app build script.
- App code depends only on public SDK boundaries.

## Temporary Migration Rules

- Existing submodules are transitional checkout aids only while the shared SDK repos are still being split and versioned.
- New app code must not add fresh imports into shared SDK internals by filesystem path.
- Shared protocol changes must update Rust source, generated TypeScript output, and consuming adapters in the same change.
- App-local build steps may generate app-owned code only. Shared SDK outputs must be generated from the shared SDK boundary.
- During the transition, package-style consumption of `golden_ui` inside `src-ui` is preferred over direct `$lib/golden_ui/...` imports from app-local code.
- Inside `golden_ui` itself, new code should not depend on `$app/*` or other app-only aliases. The
  package is now source-self-contained and should keep that property before it moves out of
  `src/lib/`.

## Version Pinning Policy

Stable app integrations should pin released SDK tags. Active cross-repo work may pin exact commits temporarily.

Cargo example:

```toml
[dependencies]
golden_core = { git = "https://github.com/Golden-Geek/golden_core", tag = "v0.1.0" }
```

Cargo pinned-commit example:

```toml
[dependencies]
golden_core = { git = "https://github.com/Golden-Geek/golden_core", rev = "0123456789abcdef0123456789abcdef01234567" }
```

UI package example:

```json
{
  "dependencies": {
    "golden_ui": "github:Golden-Geek/golden_ui#v0.1.0"
  }
}
```

UI pinned-commit example:

```json
{
  "dependencies": {
    "golden_ui": "github:Golden-Geek/golden_ui#0123456789abcdef0123456789abcdef01234567"
  }
}
```

## Distribution Rule

Submodules are transitional only. They are not the end-state distribution model for `golden_core` or `golden_ui`.
