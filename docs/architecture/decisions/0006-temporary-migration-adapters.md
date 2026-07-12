# ADR 0006: Govern Temporary Migration Adapters and Shadow Execution

- Status: Accepted
- Date: 2026-07-11

## Context

Large vertical cutovers need old and new representations to coexist briefly. Forbidding all
adapters forces unsafe flag-day rewrites; allowing untracked adapters creates permanent duplicated
architecture and ambiguous authority.

## Decision

Typed temporary adapters, disposable fixture converters, dual reads, development-only feature
switches, and shadow execution are allowed only when they keep the real product runnable and reduce
cutover risk.

Every adapter is registered in the parity ledger with:

- stable adapter ID and accountable owner;
- exact scope and authoritative path;
- introduction and expiry phases;
- executable deletion criteria and tracked deletion issue;
- conversion/fallback/failure tests;
- side-effect policy preventing duplicate outputs, commands, triggers, effects, or device traffic;
- current state and removal commit when deleted.

Shadow execution compares deterministic semantic digests. Pure computations may dual-run; effectful
paths must have one authoritative dispatcher and suppress shadow effects by construction.

Adapters do not preserve old public APIs for convenience. An offline converter may migrate valuable
fixtures to a new schema and then be deleted rather than shipped as a permanent legacy loader.

## Consequences

- Safe incremental migration is explicitly compatible with the final “no legacy” goal.
- Unowned, unbounded, untested, or expiry-free adapters block a supercommit or phase close.
- Adapter status is visible row by row rather than hidden in implementation details.
- A fallback path cannot silently become permanent after cutover.

## Compliance

The normative adapter fields are defined in the
[parity ledger schema](../../product/parity-ledger-schema.md). Phase 2 must prove shadow paths cannot
emit duplicate external effects before later subsystem replacements use them.
