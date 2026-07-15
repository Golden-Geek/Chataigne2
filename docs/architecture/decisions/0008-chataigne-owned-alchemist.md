# ADR 0008: Keep Alchemist in the Chataigne Product Boundary

- Status: Accepted
- Date: 2026-07-15

## Context

Phase 3 separated generic graph documents, transactions, revisions, presentation, and canvas
mechanics from Alchemist semantics. That extraction makes the remaining ownership clear: formulas,
ANodes, managed regions, formula compilation and evaluation, built-in formula policy, and their UI
are Chataigne product concepts. Treating those concepts as reusable Golden packages would preserve
an artificial repository boundary and invite product policy back into the generic graph stack.

## Decision

`golden_graph` and `golden_graph_ui` are the complete app-agnostic system for building, editing,
persisting, presenting, and displaying typed graphs. They expose public domain-extension contracts
and never import Alchemist or other Chataigne types.

Alchemist is owned by `apps/chataigne`. Its Rust domain, formula model, ANode registry, type solver,
compiler, runtime evaluator, assets, catalog policy, and formula-specific UI depend on the public
Golden graph/value/runtime contracts as a product plugin. The former `crates/golden_alchemist` and
`packages/golden-alchemist-ui` locations are migration sources, not final reusable package
boundaries; Phase 4 relocates their complete behavior under `apps/chataigne` without recreating or
dropping the working product.

Reusable runtime, condition, processor, protocol, persistence, and UI packages must depend on
domain-neutral contracts. Chataigne composition may bind those contracts to Alchemist, but no
reusable Golden package may depend on the app-owned implementation.

Historical import records and executed Phase 3 evidence retain their original package names. They
record provenance and tested state, not final ownership.

## Consequences

- Generic graph mechanics and performance work remain reusable without formula terminology or
  Chataigne policy.
- Alchemist can evolve with the Chataigne product without pretending to be a universal engine.
- Formula-specific backend and UI changes stay atomic inside the app boundary.
- Phase 4 starts with a product-preserving ownership relocation before the typed authoring,
  compiler/runtime, UI, fixture, and final cutover slices.
- Temporary old/new path adapters remain governed by ADR 0006 and cannot own effect authority.

## Compliance

Architecture checks reject Alchemist imports from reusable Golden crates and packages. The Phase 4
cutover manifest records the relocated owners, temporary adapters, deletion criteria, and executed
product gates before either former package path is removed.
