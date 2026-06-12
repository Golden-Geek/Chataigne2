# Alchemist Architecture

## Canonical Model

An Alchemist Formula is a real Golden Core node subtree:

```text
Formula
  Properties
    typed Parameter
    optional Conditions manager
    optional Consequences manager
    optional Inputs manager
    optional Filters manager
    optional Outputs manager
  ANode
    Type parameter
    Position parameter
    Config
      real parameters
    Inputs
      real socket nodes
    Outputs
      real socket nodes
  Connection
    source ANode reference
    source socket parameter
    target ANode reference
    target socket parameter
```

There is no hidden authoring graph and no JSON parameter containing another
graph. The hierarchy, Inspector, graph editor, project persistence, undo/redo,
duplication, and asset persistence all operate on the same nodes.

At compile time, the Formula subtree is materialized into
`golden_alchemist::AlchemistGraph`. That graph is a transient compiler input,
not an authored persistence model.

The `Properties` subtree is the source of truth for Processor exposure.
Typed parameters live directly under its single root. Category managers are
optional definition nodes rather than eagerly created sections. Materialization
projects that hierarchy to the generic `FormulaSurface`; it does not introduce
a second authoring document.

## Ownership

`golden_core` owns:

- canonical nodes, parameters, references, metadata, hierarchy, and edit intents;
- project and subtree persistence;
- UUID remapping, lifecycle restoration, undo/redo, and UI synchronization.

`golden_alchemist` owns:

- app-agnostic ANode declarations and value types;
- the generic Formula surface contract and multi-target Processor overrides;
- the reusable Property getter ANode;
- transient graph typing, compilation, diagnostics, runtime memory, and execution.

`chataigne_state_machine` owns:

- Chataigne value types and ANode declarations;
- Processor lifecycle, context, Formula instances, and emitted-intent integration.

The app layer owns:

- Formula, ANode, socket, connection, and Formula Library node definitions;
- materialization of a Formula node subtree for the Alchemist compiler;
- Alchemist panel registration.

## Processor Boundary

A Processor references one Formula and owns one Formula instance:

```text
Processor A -> Formula X + configuration A + runtime memory A
Processor B -> Formula X + configuration B + runtime memory B
```

The Formula definition is shared. Processor configuration and runtime memory
are not shared.

Each Processor materializes its Formula's `Properties` definition as real
children. Typed Formula properties become Processor-owned editable parameters.
Optional category definitions become Processor-owned manager containers for
conditions, consequences, inputs, filters, and outputs. Stable source UUIDs
link those instance nodes back to their Formula definitions.

Each exposed Property has a stable Formula node identity. Dragging it onto the
graph creates a Property getter ANode bound to that identity. A Formula may
contain many getters for one Property; a Processor override is materialized
into every bound getter before compilation.

A Processor may resolve, instantiate, compile, and execute a Formula. It must
not inspect ANode types or implement Formula workflow. It has no condition,
consequence, mapping, comparator, projection, reducer, or output branches.

Action and Mapping are not engine concepts. They can later be ordinary
`.formula` template assets authored with the same editor.

## Editor Contract

The editor is a projection of the Golden Core hierarchy:

- the graph add menu comes from the Formula node's `creatable_user_items`;
- the hideable Properties blackboard renders the real `Properties` subtree;
- the blackboard has one add control, owned by the `Properties` root, for typed
  parameters and optional category managers;
- selecting a property edits its definition through the normal Inspector;
- dragging a typed Property onto the canvas creates a bound getter at the drop
  position;
- Property getter cards show only their synchronized property name and sockets;
  their internal binding configuration is hidden and read-only;
- graph cards come from direct `alchemist_anode` children;
- configuration controls render ordinary ANode parameter children;
- sockets come from real input/output socket children;
- wires come from `alchemist_connection` children;
- graph and hierarchy selection are the same workbench selection;
- Delete, duplicate, copy, paste, undo, redo, and select-all use shared
  workbench commands;
- ANode creation uses the standard creatable-item context menu from right-click,
  Space, or the Formula manager add button;
- moves, resizes, connections, and deletion use normal Golden Core intents;
- ANode colors are deterministic variations within stable family palettes.

Rust and TypeScript do not maintain a duplicate Formula authoring schema.

## Formula Assets

A `.formula` asset is the sparse Golden Core `ProjectFile` representation of a
Formula subtree. Export and import use the same node codec as project
persistence. Import remaps UUIDs as one unit, preserving internal connection
references while avoiding collisions.

## Current Scope

Custom Formula creation, single-root Properties authoring, Processor-owned
property instances, drag-created read-only Property getters, visible graph
editing, dynamic ANode structure, project reload, subtree asset round-trip,
Processor surface projection, and compile validation are the current
foundation.

There are intentionally no built-in Action or Mapping Formulas.
