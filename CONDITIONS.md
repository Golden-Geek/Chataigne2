# Conditions

## Boundary

Condition behavior belongs in Formula ANodes. A Processor only hosts a Formula
instance and must not interpret condition nodes, comparators, projections,
reference fields, reducers, or temporal policies.

The existing state-owned condition configuration nodes are provisional UI
configuration only. They are not a second runtime and are not executed by
`StateProcessor`. Their replacement must be a real ANode declaration, or a
Managed ANode whose visible Golden Core node subtree lowers to executable
Alchemist nodes.

## Required Model

The condition ANode owns:

- source selection and source typing;
- component extraction;
- comparator selection;
- typed reference parameters;
- validation and invalidation timing;
- group reduction;
- compiler lowering and diagnostics.

All authored configuration remains visible as ordinary Golden Core nodes and
parameters. Dynamic choices are produced by the owning ANode when its real
configuration changes.

## Projection Contract

Projection is extraction-only:

```text
scalar -> All
Vec2   -> X, Y, All
Vec3   -> X, Y, Z, XY, XZ, YZ, All
Color  -> valid scalar/component-group extractions, All
```

`All` is identity. Conditions do not expose scalar-to-vector or
scalar-to-color expansion projections.

The effective type after projection controls:

- available comparator options;
- which reference parameters exist;
- each reference parameter's value type;
- compile diagnostics.

These changes are reconciled by the ANode into real child parameters. The
Processor never mutates condition inspector metadata.

## Next Work

1. Define the condition ANode's visible Golden Core configuration subtree.
2. Add type-driven extraction, comparator, and reference declarations.
3. Lower that configuration to executable Alchemist operations.
4. Add Formula tests for typing, timing, and reduction.
5. Author behavior templates as `.formula` assets rather than Processor code.
