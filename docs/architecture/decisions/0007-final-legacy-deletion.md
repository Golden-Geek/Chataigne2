# ADR 0007: Delete Old Paths Only After Proven Cutover

- Status: Accepted
- Date: 2026-07-11

## Context

Keeping every old implementation indefinitely would leave duplicated concepts, unclear authority,
and permanent maintenance cost. Deleting first, however, removes the product oracle and recovery
path before a replacement is proven.

## Decision

No baseline implementation, UI, module, asset, formula, fixture, or recovery path is deleted before
its replacement passes applicable automated and manual parity gates in the real application.

After proof, remove the replaced path in the same or immediately following focused, runnable,
product-gated supercommit. Record the removal commit in the capability and adapter records. Maintain
a rollback point until the subsystem's parity and soak gates pass.

The final production architecture contains no old/new dual path, legacy alias, permanent schema
loader, duplicated protocol declaration, submodule bootstrap path, or compatibility facade retained
only for the migration. Former repositories remain immutable read-only history after final cutover,
not active dependencies.

Deliberate product or UX removals are separate owner-approved decisions and cannot be inferred from
architectural cleanup.

## Consequences

- Temporary duplication is bounded by evidence and expiry rather than prohibited prematurely.
- Final ownership becomes singular and obsolete paths are actually removed.
- Deletion commits remain reviewable, bisectable, and reversible until soak completion.
- Final qualification requires every parity row signed off and every adapter either removed or
  explicitly rejected from production.

## Compliance

Phase-closing reports list all removed paths, linked passing evidence, approvals where behavior
changed, rollback refs, and remaining adapters. Phase 9 performs final deletion only after the full
product, platform, performance, soak, persistence, networking, and parity gates are green.
