# Golden / Chataigne

This repository is the single Golden monorepo. It contains reusable Rust crates and Svelte
packages plus the Chataigne product composition. There are no submodules, legacy runtime,
or alternate protocol paths.

## Repository map

| Path | Responsibility |
|---|---|
| `crates/golden-model`, `golden-values`, `golden-parameters`, `golden-context` | Stable identity, authored values, parameter declarations, and context expansion |
| `crates/golden-graph` | Domain-neutral graph document, transactions, traversal, and protocol adapter boundary |
| `crates/golden-alchemist`, `golden-statechart`, `golden-condition`, `golden-processor` | Domain authoring and compilation built above the generic graph/value layers |
| `crates/golden-runtime` | Immutable compiled generations and deterministic dense/sparse/idle execution |
| `crates/golden-protocol`, `golden-codegen`, `golden-transport` | One generated public protocol, bounded transport queues, interests, and network safeguards |
| `crates/golden-io`, `golden-script`, `golden-persistence`, `golden-host` | Endpoint recovery, script contracts, crash-safe persistence, and host composition |
| `packages/golden-*` | Reusable Svelte 5 UI, graph/domain adapters, and keyed runtime client stores |
| `apps/chataigne` | Product-owned modules, assets, dashboards, Spatializer, panels, and host policy |
| `benchmarks`, `docs/architecture` | Qualification evidence and enforceable architecture decisions |

Dependency direction is inward: app code may compose Golden packages; Golden packages may
depend only on lower layers declared in
[`dependency-rules.v1.json`](docs/architecture/dependency-rules.v1.json). Generic packages
never import Chataigne policy.

## First checkout

Requirements are Rust 1.95, Node 24.18+, npm 11.12+, Python 3, and Chromium for browser
gates.

```text
npm ci
cargo test --workspace --all-features
npm run check
npm test
npm run test:browser
```

On Windows, use the installed MSVC Rust toolchain. `tools/dev.ps1` / `tools/dev.sh` prepare
and check the workspace; `tools/check.ps1` runs the complete local quality gate.

Start with [Architecture](docs/architecture/README.md) for ownership and
[Contributing](CONTRIBUTING.md) before changing a public boundary.
