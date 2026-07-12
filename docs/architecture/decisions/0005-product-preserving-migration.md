# ADR 0005: Migrate Through a Continuously Runnable Chataigne Product

- Status: Accepted
- Date: 2026-07-11

## Context

The failed rewrite at `174fc5096ac7ab4546b3acc76569bca6a1c9e01d` introduced architectural
names and isolated tests while deleting most of the working UI, modules, assets, host, and product
connections. Foundation-level success therefore did not demonstrate product parity.

## Decision

The migration branch starts from recorded `origin/main` commit
`fb0f3a58f3593df8994bf8bd46f88ddd7612f41d`. Working-product commit
`f0a7c2fe4d192a076c7649c6fe3e6a5ab193a435` and its exact gitlinks are the behavioral,
interaction, visual, module, fixture, and performance oracle.

Import the complete product before reorganizing it. Migrate vertical slices spanning authoring,
runtime/compiler, transport, UI stores and panels, persistence, diagnostics, and tests. Every
accepted cutover remains demonstrable through the real Chataigne frontend and engine.

Every canonical-branch supercommit is `RUNNABLE` and product-gated. A narrowly scoped intentionally
non-runnable interval may exist only on a private/topic branch with documented breakage, rollback,
and immediate restoration. It cannot be a shared head, phase completion, cutover, deletion, or
merge-to-main candidate.

The failed rewrite is retained only as a donor. Donor units require boundary, semantic, product,
and evidence review and are imported or reimplemented individually.

## Consequences

- A demo shell, headless graph, registry metadata, mock module, or synthetic UI test cannot close a
  phase.
- Existing projects remain openable or receive verified converted equivalents.
- UX changes are explicit product decisions with evidence and approval.
- The parity ledger records honest baseline, adapted, shadowing, cut-over, and removed states.
- Unknown or unexecuted evidence blocks completion.

## Compliance

The continuous product gate builds the complete workspaces and real binaries, connects the real UI
and engine, loads and mutates a canonical fixture, exercises core workflows, compares manifests,
tests loopback IO and non-loopback LAN access, captures browser failures, and covers the supported
platform matrix.
