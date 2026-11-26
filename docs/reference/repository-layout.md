# Repository Layout

The root Cargo and npm workspaces belong to Chataigne2. The `golden/` Git submodule pins
[Golden Core](https://github.com/Golden-Geek/GoldenCore). It supplies the public Rust crates,
the Golden workbench, audio device UI, graph canvas, generated contracts, and vendored dependencies.

[MapGyver2](https://github.com/Golden-Geek/MapGyver2) has its own workspace, tooling and installers.

## Chataigne

```text
apps/chataigne/
├── src/
│   ├── app/                     thin lifecycle and composition shell
│   └── module/                  concrete product modules
├── tests/
│   └── samples/                 crate-level project fixtures
├── systems/
│   ├── alchemist/
│   │   ├── src/                 Formula/ANode graph, compiler, and runtime
│   │   ├── condition/           condition authoring/compiler/runtime
│   │   ├── processor/           Inputs, Filters, Outputs, lanes, and ValueSet
│   │   └── integration/         Golden Core nodes for the Alchemist surface
│   └── state_machine/
│       ├── model/               graph-backed state/transition model
│       ├── runtime/             execution, arbitration, and protocol
│       └── integration/         Golden Core state/transition nodes
├── resources/
│   └── formulas/builtin/        shipped Action and Mapping formula assets
└── ui/                          Svelte 5 product UI
```

`apps/chataigne/build.rs` generates the app node registry and embeds bundled UI/formula assets.
Desktop capabilities, icons, permissions, and Tauri configuration remain directly under the app.
The persistent Sound Card model and product policy live under
`apps/chataigne/src/module/modules/audio/sound_card/`; reusable device/status presentation lives in
`golden/packages/golden-audio-ui/`.

Within every Rust crate and UI source area, unit tests live in a local `tests/` directory directly
under the feature they exercise. A crate's top-level `tests/` directory is reserved for crate-wide
integration tests and their fixtures. See [Development Workflows](../guides/development.md#test-placement)
for the exact convention.

## Documentation and tooling

- `docs/architecture/`: ownership and dependency design.
- `docs/guides/`: contributor workflows.
- `docs/operations/`: performance, release, troubleshooting, and hygiene runbooks.
- `docs/reference/`: exact maps and policies.
- `docs/plan/`: the Chataigne implementation plan and the repository split plan.
- `docs/progress/`: current product ledgers and qualification evidence.
- `docs/archive/`: non-operative historical plans and qualification evidence.
- `tools/`: bootstrap, checks, qualification, product gates, and release tooling.
- `xtask/`: root watch orchestration.
