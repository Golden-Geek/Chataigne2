# Foundation Ownership

Shared identities, values, parameters, and contexts are the live foundations consumed by graph,
runtime, protocol, persistence, and UI layers. They are not parallel models.

## `golden_model`

`golden_model` owns stable, app-agnostic identities used across engine, protocol, persistence, and UI
boundaries. `NodeId`, `NodeUuid`, and `DeclId` retain their existing serialized and generated
TypeScript shapes. `golden_engine::node` temporarily re-exports them so downstream migration can be
reviewed independently of the ownership cutover.

## `golden_values`

`golden_values::Value` is the canonical runtime value model. It owns value type identifiers,
triggers, scalar/vector/color values, durations, arrays, stable references, and extension payloads.
Color channels are `f64`, matching parameters, protocol DTOs, persistence, and UI numbers without
the previous Alchemist-only precision narrowing.

`chataigne_alchemist` and the rest of Chataigne depend on `golden_values` directly. Alchemist may
use a private semantic name internally, but package consumers receive the canonical
`golden_values::Value` API.
Parameter-specific concepts such as files, enum selections, CSS units, and node-reference hints
cross the canonical boundary as typed extensions and round-trip through executable tests.

## `golden_parameters`

`golden_parameters` owns parameter values, value types, constraints, control state, projections,
snapshots, UI hints, canonical-value conversion, and `NodeReference`. It depends only on
`golden_model`, `golden_values`, and serialization primitives. The engine continues to own the
stateful `Parameter` node and animation-control node and consumes the public parameter contracts.

The former engine `Color` is now an alias of the canonical `golden_values::ColorValue`, exposed
through `golden_parameters` for parameter-facing callers. There is no second color declaration.

## `golden_context`

`golden_context` owns the app-agnostic context registry, snapshots, declarations, updates, and
dynamic context contracts. It depends on stable model identities and parameter values/projections,
not on the engine loop, host runtime, statechart policy, or Chataigne modules.

## Dependency Direction

Foundation crates form a downward-only dependency chain: model and values are lowest; parameters
may consume them; context may consume model and parameters. They otherwise depend only on
serialization and compact storage primitives. They cannot depend on the engine, Alchemist,
statecharts, Chataigne, host code, or UI policy. The executable contract in
`tools/migration/check_phase3_contracts.py` enforces this direction and verifies that the old owners
no longer declare the moved types.

Historical cutover evidence is recorded in
[`phase3-cutovers.v1.json`](../product/manifests/phase3-cutovers.v1.json).
