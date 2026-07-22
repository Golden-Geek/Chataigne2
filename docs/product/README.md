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
- [Toolchain policy](toolchain-policy.md) records supported version selections, update boundaries,
  and local versus cross-platform qualification requirements.
- [Parity ledger schema](parity-ledger-schema.md) defines the fields and completion rules for every
  independently observable capability and temporary adapter.
- [`functional-parity-evidence.v1.json`](manifests/functional-parity-evidence.v1.json) is the
  authored side of the Phase 9 parity ledger. Generated discovery stays separate and no entry is
  accepted as qualified without the complete executable-result contract.
- [Phase 2 seam dashboard](manifests/phase2-seams.v1.json) records each application-facing seam and
  the governed production adapters that remain authoritative during later cutovers.
- [Phase 4 cutover dashboard](manifests/phase4-cutovers.v1.json) records the Chataigne Alchemist
  ownership and package relocation.
- [Phase 5 cutover dashboard](manifests/phase5-cutovers.v1.json) records the statechart, compiled
  condition, context/lane, Processor ownership, and product-composition cutovers.
- [Phase 6 cutover dashboard](manifests/phase6-cutovers.v1.json) records the runtime-center cutovers,
  cross-platform qualification, and governed Phase 8 adapter carried forward.
- [Phase 7 cutover dashboard](manifests/phase7-cutovers.v1.json) records the generated protocol,
  observation planes, panel-area migrations, slow-client isolation, and old-protocol deletion.
- [Phase 8 construction dashboard](manifests/phase8-cutovers.v1.json) records the separately gated
  module families, shared IO cutovers, carried runtime adapter, and qualification state.
- [Phase 8 controller/hardware evidence](manifests/phase8-hardware-evidence.v1.json) names the
  platform scope, deterministic adapter, executable fixtures, and physical-device status per family.
- [Phase 9 qualification dashboard](manifests/phase9-qualification.v1.json) declares the current
  construction interval, preserves the immutable Phase 8 checkpoint, and records every final
  parity, scale, soak, packaging, documentation, and deletion gate. Run
  `python tools/migration/phase9_readiness.py --json` for the machine-readable blocker report.
- [Phase 9 UX approval](manifests/phase9-ux-approval.v1.json) records the product-owner decision and
  immutable hashes of the reviewed mounted-app captures.
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
- Generated and implementation-owned manifests remain distinct from executed evidence. A dashboard
  state or discovered source row is not a `PASS` unless its declared command actually ran.

## Precedence

Older repository-boundary documents describe the pre-Phase-1A submodule architecture. Where they
conflict with the final plan or the accepted ADRs, the final plan and accepted ADRs govern the
migration.
