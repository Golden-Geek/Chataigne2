# ADR 0004: One canonical value system

- Status: Accepted
- Date: 2026-07-10

## Decision

`golden-values` owns the sole reusable `Value`, value-type descriptors, projections,
conversion rules, equality/change semantics, `ValueSet`, lane keys, trigger edges, stable
references, and compact storage descriptors. `golden-parameters` layers authoring and
control semantics on those values.

Parameters, contexts, formulas, conditions, module IO, scripting, persistence, and protocol
must not maintain parallel value enums or conversion implementations.

## Consequences

The current parameter/runtime split is replaced in Phase 1. Runtime compilation maps the
canonical authored values into dense storage; it does not reinterpret parameter nodes on
each tick. Extension values remain validated and explicitly bounded.
