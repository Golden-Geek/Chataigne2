# Alchemist Architecture

## Canonical Model

An Alchemist Formula is a real Golden Core node subtree:

```text
Formula
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

## Ownership

`golden_core` owns:

- canonical nodes, parameters, references, metadata, hierarchy, and edit intents;
- project and subtree persistence;
- UUID remapping, lifecycle restoration, undo/redo, and UI synchronization.

`golden_alchemist` owns:

- app-agnostic ANode declarations and value types;
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

A Processor may resolve, instantiate, compile, and execute a Formula. It must
not inspect ANode types or implement Formula workflow. It has no condition,
consequence, mapping, comparator, projection, reducer, or output branches.

Action and Mapping are not engine concepts. They can later be ordinary
`.formula` template assets authored with the same editor.

## Editor Contract

The editor is a projection of the Golden Core hierarchy:

- the add menu comes from the Formula node's `creatable_user_items`;
- graph cards come from direct `alchemist_anode` children;
- configuration controls render the ANode's real parameter children;
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

Custom Formula creation, visible graph editing, dynamic ANode structure,
project reload, subtree asset round-trip, and compile validation are the
current foundation.

There are intentionally no built-in Action or Mapping Formulas.
