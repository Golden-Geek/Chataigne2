# Architecture

The canonical architecture is defined by
[Golden Architecture Final Plan](docs/Golden_Architecture_Final_Plan.md).

Dependency direction begins with four independent foundations:

```text
golden-model
├── golden-values
│   ├── golden-parameters
│   └── golden-context
└── future graph and protocol foundations
```

Alchemist and statecharts will be separate typed graph domains. Compiled immutable runtime
generations will separate control, compilation, IO, semantic, effect, and observation planes.
Chataigne remains product composition and concrete modules only.

Machine-readable allowed dependencies live in
[`docs/architecture/dependency-rules.v1.json`](docs/architecture/dependency-rules.v1.json).
CI validates the active Cargo graph and rejects Git submodules.

`legacy/repositories` is not an architectural layer. It contains imported history and
pre-rewrite behavior references only, and active workspace members may not depend on it.
