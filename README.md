# Golden

Golden is the clean-sheet monorepo for reusable authoring, graph, runtime, protocol, host,
and UI foundations. Chataigne is an application composed from those public boundaries.

The active workspace no longer depends on Git submodules. Pre-rewrite sources and imported
repository histories are retained under `legacy/repositories` as read-only characterization
references; new code must not depend on them.

## Active workspace

- `crates/golden-model`: stable identity, revisions, handles, and change primitives.
- `crates/golden-values`: the canonical value, conversion, projection, and `ValueSet` model.
- `crates/golden-parameters`: parameter declarations, constraints, controls, and state.
- `crates/golden-context`: checked context axes, lane layouts, and migration keys.
- `packages/`: reusable JavaScript workspace boundaries.
- `apps/chataigne/backend`: thin Rust product composition.
- `apps/chataigne/ui`: thin UI product composition.
- `docs/architecture`: accepted decisions, dependency rules, and parity evidence.

## Verification

```sh
cargo test --workspace
npm ci
npm run check
python tools/check_architecture_contract.py
python tools/check_workspace_architecture.py
```

Read [the final architecture plan](docs/Golden_Architecture_Final_Plan.md) before making a
cross-layer change. Each implementation phase ends with its evidence and one focused
supercommit; legacy paths are deleted after parity is proven.
