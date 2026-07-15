# Architecture Decision Records

These ADRs freeze the decisions required before product-preserving migration work proceeds. They
are subordinate to the complete
[final architecture and migration plan](../../Golden_Architecture_Final_Plan.md), but supersede
older documents when those documents describe the former submodule architecture or failed rewrite.

| ADR | Status | Decision |
| --- | --- | --- |
| [0001](0001-golden-monorepo.md) | Accepted | One Golden monorepo formed by importing the complete working product |
| [0002](0002-generic-graph-boundary.md) | Accepted | `golden-graph` is the sole reusable graph foundation |
| [0003](0003-runtime-planes.md) | Accepted | Six explicit runtime planes with typed handles and queues |
| [0004](0004-canonical-value-system.md) | Accepted | One canonical value and conversion system in `golden-values` |
| [0005](0005-product-preserving-migration.md) | Accepted | Continuous runnable-product migration from the recorded baseline |
| [0006](0006-temporary-migration-adapters.md) | Accepted | Governed temporary adapters and side-effect-safe shadowing are allowed |
| [0007](0007-final-legacy-deletion.md) | Accepted | Old paths are deleted only after parity proof; no permanent compatibility architecture remains |
| [0008](0008-chataigne-owned-alchemist.md) | Accepted | Alchemist is Chataigne-owned and depends on the app-agnostic Golden graph system |

## Status Meanings

- `Proposed`: under review and not an implementation constraint.
- `Accepted`: normative for migration work.
- `Superseded`: retained for history and linked to its replacement.

Changes to an accepted decision require a superseding ADR and corresponding update to the migration
plan and product progress record.
