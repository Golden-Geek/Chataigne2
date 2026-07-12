# Product Preservation Records

This directory is the governance entry point for the product-preserving Golden migration. It
records what is authoritative, what evidence is required, and what has actually been validated. It
does not turn an unexecuted scenario into evidence.

Start here:

- [Phase 0 baseline](baseline.md) records the immutable repository and former gitlink revisions.
- [`source-imports.v1.json`](source-imports.v1.json) is the machine-readable record of the exact
  source revisions imported into the Phase 1A monorepo. Its `path` fields are historical Phase 0
  locations; current ownership is documented in [the repository map](../repo-map.md).
- [Migration progress](migration-progress.md) records phase state and the exact validation state of
  required commands and environments.
- [Parity ledger schema](parity-ledger-schema.md) defines the fields and completion rules for every
  independently observable capability and temporary adapter.
- [Architecture decisions](../architecture/decisions/README.md) record the Phase 0 decisions that
  constrain implementation.
- [Final architecture and migration plan](../Golden_Architecture_Final_Plan.md) remains the complete
  normative plan.

## Truth Boundaries

- Repository refs, former gitlinks, and imported commit ancestry are provenance, not
  product-parity evidence.
- A source path, catalog entry, test name, or screenshot by itself is not proof that a workflow
  works.
- Only an actually executed result may be marked `PASS`.
- A failed prerequisite is `FAIL`; commands that cannot run because of it are `BLOCKED`.
- Work not attempted is `NOT_RUN`. Platform or hardware evidence must not be inferred from another
  environment.
- The generated parity ledger and manifests will be added by their owning implementation slices.
  These schema documents deliberately contain no fabricated generated data.

## Precedence

Older repository-boundary documents describe the pre-Phase-1A submodule architecture. Where they
conflict with the final plan or the accepted ADRs, the final plan and accepted ADRs govern the
migration.
