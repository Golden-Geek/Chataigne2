# Phase 8: persistence and recovery

The project format now starts at a clean schema version 1. `golden-persistence` owns the
generic envelope, ordered migrations, pre-application limits, immutable save snapshots,
atomic replacement, rolling backups, recovery journal, and corruption diagnostics.

`apps/chataigne/backend` owns `ChataigneProjectV1`: hierarchy and parameters, versioned
graph and formula assets, statecharts, contexts, processors, module configuration,
dashboards, and presentation state. Compiled runtime generations are deliberately absent.
Built-in Action and Mapping manifests are immutable and tied to the application and the
versioned Alchemist formula-file schema.

The development fixture under `apps/chataigne/backend/fixtures` is already in the final v1
format. There is no runtime legacy decoder or compatibility branch. Machine-readable
evidence is in `benchmarks/phase8/persistence-recovery.v1.json`.
