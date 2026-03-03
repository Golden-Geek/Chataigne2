# User-Defined Models as Prefab-Style Node Types in a Generic Node Engine

## (Eager Instances, Overrides, Propagation; Golden Core–level Design)

---

## 1. Purpose

Specify a **generic, engine-level** system that allows end users (or extension authors) to declare reusable **models** (class-like object schemas) and instantiate them as **native node types** inside a node/parameter graph.

The design must:

* Treat **Node** as the canonical runtime entity (arena-managed), with no parallel “object” runtime.
* Create **fully materialized** instances at creation time (no hidden/collapsed structure).
* Provide **Prefab-style behavior**:

  * Instances can override defaults.
  * Overrides can be reverted to model defaults.
  * Editing the model updates all non-overridden instance fields.
  * Optional “Apply” flow from instance → model defaults.
* Remain simple to implement and reason about, while keeping runtime access fast.

This system lives in the **core engine** (e.g., `golden_core`). A host application builds higher-level concepts on top of it.

---

## 2. Core Concepts

### 2.1 Node Graph (Engine Canonical Structure)

The engine provides:

* Nodes stored in an arena.
* Nested child structure (containers/folders/child nodes).
* Parameters (typed values) on nodes.
* Metadata (including a stable declaration identifier where relevant).
* Observability (subscriptions to changes).

The model system must be expressed entirely in terms of these primitives.

### 2.2 Model (Prefab / Definition)

A **Model** is a user-authored **prototype node subtree** stored in a registry. It defines:

* A root node (the model root).
* A nested structure of child nodes and parameters.
* Default values for all parameters and configuration.
* A stable identifier for each addressable field via **DeclId** (below).

A model behaves like a *type definition* that can be instantiated anywhere nodes are allowed.

### 2.3 Instance (Prefab Instance)

An **Instance** is a normal node subtree in the arena created by cloning the model’s prototype subtree **eagerly**.

Immediately after creation:

* All nodes exist.
* All parameters exist.
* All children are traversable.
* All values are readable/writable using normal engine APIs.
* All change notifications behave normally.

Each instance stores:

* Link to `model_id` (stable identity)
* Link to `model_version` (recommended)
* `OverrideSet` (prefab override mechanism)
* `DeclIndex` (fast DeclId → NodeId resolution)

### 2.4 DeclId (Stable Field Identity)

A **DeclId** is the stable semantic identifier of a field (parameter or node-level field) within a model/instance.

Examples:

```text
position
rotation
scale
colors.active
colors.inactive
```

DeclId is the canonical key used for:

* Override tracking (instance vs model)
* Model → instance propagation
* Binding/targeting by external systems (static or dynamic)
* Structure evolution and migrations

DeclId must be:

* Stable across sessions
* Independent of runtime UUIDs
* Derived from an intentional declaration scheme (typically semantic path within the model)

---

## 3. “User Models Are Node Types”

### 3.1 Dynamic Node Types

Each model is exposed as a dynamic node type identifier, e.g.:

```text
model::<model_id>
```

This ensures the host can treat models like any other built-in node type:

* Creation uses the same factory path (type → node creation).
* Container admission rules can allow/deny model types.
* Enumeration/menus can list available types.

The engine does not need host-specific concepts; it only needs a way to create node trees for a given model type.

### 3.2 Eager Instantiation (No Hidden Structure)

When creating an instance:

1. Clone the entire prototype subtree into the arena.
2. Attach `InstanceMeta` to the root.
3. Build `DeclIndex` by walking the cloned subtree and indexing DeclIds.
4. Register the instance in the model registry’s instance index.

This matches the rule: if it exists conceptually, it exists concretely.

---

## 4. Prefab Override System

### 4.1 Minimal Override Representation

Each instance tracks override status per DeclId:

* `overrides: HashSet<DeclId>` (minimal, simple)
* Optional extension later: `HashMap<DeclId, OverrideInfo>` for metadata

**No separate storage for overridden values is required**: values already live in the instance’s parameter nodes. Override tracking only says whether the instance “owns” the value or follows the model default.

### 4.2 Override Detection (Instance Value vs Model Value)

When an instance parameter changes (any source: UI, script, external driver, runtime processor):

1. Resolve the field’s DeclId.
2. Fetch the model default value for the same DeclId.
3. Compare instance value `I` with model value `M` using type-aware equality.
4. If `I != M` → mark DeclId as overridden.
5. If `I == M` → clear the override.

This supports “override disappears when value matches default again”.

### 4.3 Revert

#### Revert Field

* Set instance value to model value.
* Remove DeclId from override set.

#### Revert All

* For all DeclIds in the model:

  * Set instance value to model value.
* Clear the override set.

### 4.4 Apply (Instance → Model)

Apply is explicit, user-driven.

For selected DeclIds:

1. Set the model default value to the instance value.
2. Propagate the model change (see Section 5).
3. Recompute override state naturally via the same equality rule.

Apply enables workflows where a user edits an instance and then “commits” it as the new model default.

---

## 5. Model → Instance Propagation

### 5.1 Parameter Default Changes in the Model

When a model parameter default changes for DeclId `d`:

For each instance of the model:

* If `d` is **not** overridden in that instance:

  * Update the instance parameter to the model’s new value.
* If `d` **is** overridden:

  * Do nothing.

This rule yields predictable behavior:

* Model edits affect instances globally.
* Instance-specific changes are preserved.

### 5.2 Efficient Instance Enumeration

The registry maintains an index:

* `instances_by_model: HashMap<ModelId, Vec<InstanceRootNodeId>>`

Propagation iterates this list. At 100–1000 instances, this is operationally simple and fast enough.

---

## 6. Fast DeclId Resolution (Runtime-Critical)

External systems commonly need to target fields repeatedly. The engine must make field resolution constant-time.

### 6.1 DeclIndex per Instance

Each instance maintains:

* `decl_index: HashMap<DeclId, NodeId>` (typically the parameter node id)

Built once during instantiation (or updated during structural migrations).

Field access becomes:

* Resolve DeclId → NodeId in O(1)
* Read/write parameter via normal engine APIs

### 6.2 DeclIndex per Model

Each model maintains a corresponding index for its prototype:

* `decl_index: HashMap<DeclId, PrototypeNodeRef>` (node id or stable serialized path)

Used for:

* Fast model default lookup during override detection
* Structural diffs and migrations

---

## 7. Host-Agnostic Integration Patterns

The engine must support two common access modes that host applications will build on:

### 7.1 Static Targeting (Direct Field References)

A host component stores a stable reference such as:

* `(instance_root_id, decl_id)`

It resolves once through `DeclIndex` to a NodeId, then caches NodeId for fast repeated access.

Examples:

* A command system setting a field on a trigger.
* A UI binding.
* A script-driven automation.

### 7.2 Dynamic/Batched Targeting (Collection Views)

A host component selects a collection of instances and applies operations over many targets:

* Query instance roots by type, tag, container membership, etc.
* Project DeclIds (“columns”)
* Precompute per-instance NodeIds for each DeclId and cache them

Hot loop becomes a tight iteration over NodeIds with minimal overhead.

This is a general “vectorized processing” pattern; the engine only needs:

* instance enumeration
* fast DeclId resolution
* normal parameter set/get
* change notification hooks

---

## 8. Structure Evolution (Add/Remove/Rename) and Versioning

Models evolve over time; existing instances must remain valid and consistent.

### 8.1 Add Field

When a new DeclId appears in the model:

* Add the corresponding node/parameter to each instance in the correct location.
* Initialize instance value to the model default.
* Ensure it is not overridden.
* Update instance `decl_index`.

### 8.2 Remove Field

When a DeclId is removed from the model:

* Remove the corresponding node/parameter from each instance.
* Remove DeclId from override set.
* Remove from instance `decl_index`.

(An “orphan” strategy is possible but increases complexity; hard removal is simplest.)

### 8.3 Rename Field

Rename requires an explicit migration mapping:

* `old_decl_id -> new_decl_id`

Migration steps:

* Update model DeclId.
* For each instance:

  * Move/transfer value (if structure also changes)
  * Transfer override flag
  * Update decl_index entry

Without an explicit rename migration, rename degenerates into remove+add (risking data loss). Provide an explicit rename operation at the tooling level if preserving data matters.

### 8.4 Model Versioning

Each model stores a monotonically increasing `version`. Each instance stores `model_version` at creation/update.

On load or when the model is edited:

* If instance version < model version:

  * Apply migration steps in order
  * Update instance version

---

## 9. Equality and Type Semantics

Override detection depends on equality.

Requirements:

* Equality must be type-aware.
* Floating-point tolerance rules must be explicitly defined if needed.
* Type changes should require explicit migration and conversion rules.

To keep implementation simple:

* Prefer prohibiting silent type changes without a migration step.

---

## 10. UI/Tooling Expectations (Engine-Supported, Host-Implemented)

While UI is host-level, the engine must expose enough information for a prefab-like UX:

* Ability to query whether a field is overridden (`OverrideSet` membership).
* Ability to revert a field or all fields.
* Ability to apply overrides to model defaults.
* Ability to enumerate all DeclIds present in a model/instance (for inspector/tree tooling).

---

## 11. Implementation Architecture (Codex-Oriented)

### 11.1 Model Registry

```rust
struct ModelRegistry {
    models: HashMap<ModelId, ModelDecl>,
    instances_by_model: HashMap<ModelId, Vec<NodeId>>, // instance root ids
}
```

Responsibilities:

* Store models and their prototype subtrees.
* Provide model lookup by id/type.
* Track instances per model for propagation.
* Provide migration tooling and version management.

### 11.2 Model Definition

```rust
struct ModelDecl {
    model_id: ModelId,
    version: u32,
    prototype_root: NodeId, // or serialized tree
    decl_index: HashMap<DeclId, PrototypeNodeRef>,
}
```

### 11.3 Instance Metadata

```rust
struct InstanceMeta {
    model_id: ModelId,
    model_version: u32,
    overrides: HashSet<DeclId>,
    decl_index: HashMap<DeclId, NodeId>,
}
```

Stored on the instance root (or in an engine-side side table keyed by root id).

### 11.4 Engine Hooks (Must Exist)

#### Hook A — On Instance Parameter Change

Inputs: instance parameter node id, new value
Steps:

1. Resolve instance root and DeclId
2. Fetch model value for DeclId
3. Compare instance vs model
4. Update OverrideSet

This hook must capture all writes regardless of source.

#### Hook B — On Model Parameter Change

Inputs: model id, DeclId, new model value
Steps:

1. Enumerate instances of the model
2. For each instance: if not overridden, write new value to instance

#### Hook C — On Model Structural Change

Inputs: model id, old decl set, new decl set, optional rename map
Steps:

1. Diff DeclIds (add/remove/rename)
2. Apply changes to each instance subtree
3. Update DeclIndex and OverrideSet accordingly
4. Increment model version; update instance versions after migration

### 11.5 Node Factory Integration

When requested to create `model::<model_id>`:

1. Clone prototype subtree
2. Attach InstanceMeta
3. Build DeclIndex
4. Register instance root under `instances_by_model[model_id]`

---

## 12. Invariants

1. Every instance is a fully materialized node subtree immediately after creation.
2. Every addressable field has a stable DeclId in model and instances.
3. OverrideSet is the sole authority for “instance-owned vs model-following”.
4. Model changes propagate to instances only when the field is not overridden.
5. Revert always sets instance value to model value and clears override.
6. Apply writes instance values into model defaults and triggers normal propagation.
7. Structural diffs and migrations are keyed by DeclId, never by runtime UUID paths.

---

## 13. Example Model: `Sequence` (Complex Nested Structure)

A user-defined model representing a reusable timeline/sequence object:

```text
Sequence
  duration (Float)              decl_id="duration"
  loop (Bool)                   decl_id="loop"
  tracks
    track_0
      name (String)             decl_id="tracks.0.name"
      enabled (Bool)            decl_id="tracks.0.enabled"
      clips
        clip_0
          start (Float)         decl_id="tracks.0.clips.0.start"
          length (Float)        decl_id="tracks.0.clips.0.length"
          gain (Float)          decl_id="tracks.0.clips.0.gain"
```

Prefab behavior:

* Editing `Sequence.duration` in the model updates all instances unless overridden.
* If an instance overrides `tracks.0.enabled`, model changes to that field do not affect the instance.
* Revert restores instance fields to model defaults.
* Apply commits instance edits back into model defaults, updating other instances.

(If indexed paths are supported, the DeclId scheme for arrays/lists must be specified and stabilized.)

---

## 14. Summary

This design introduces user-declared **models** into a generic node engine by representing them as **dynamic node types** with **eagerly created** instance subtrees. A minimal Prefab-style override mechanism—keyed by stable **DeclId**—supports override/revert/apply semantics and deterministic model-to-instance propagation. Fast runtime access is ensured through per-instance **DeclIndex** mapping DeclId to concrete node ids, enabling both static targeting (direct field references) and dynamic/batched targeting (collection-based processing) without host-specific concepts.
