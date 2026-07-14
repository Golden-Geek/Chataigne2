# Foundation Ownership

Phase 3 extracts shared identities and values before graph transactions or UI state depend on them.
The live product consumes these types; the new crates are not parallel models.

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

`golden_alchemist::RuntimeValue` is a governed compatibility export of `golden_values::Value`; it is
not a second declaration. Parameter-specific concepts such as files, enum selections, CSS units,
and node-reference hints cross the canonical boundary as typed extensions and round-trip through
executable tests.

## Dependency Direction

Foundation crates may depend only on serialization and compact storage primitives. They cannot
depend on the engine, Alchemist, statecharts, Chataigne, host code, or UI policy. The executable
contract in `tools/migration/check_phase3_contracts.py` enforces this direction and verifies that the
old owners no longer declare the moved types.

Cutover state and compatibility-export deletion criteria are recorded in
[`phase3-cutovers.v1.json`](../product/manifests/phase3-cutovers.v1.json).
