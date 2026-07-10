# User-Defined Models as Blueprint Node Types

For `golden_core` (host-agnostic)

---

## 1. Purpose

Define an engine-level system where users declare reusable models and instantiate them as regular node subtrees.

Requirements:

- Node graph remains the only runtime representation.
- Instances are eagerly materialized (no hidden structure).
- Prefab semantics exist: override, revert, apply, propagation.
- Runtime access stays fast for large projects.
- Dynamic model node types and built-in node types use one creation/catalog path.

---

## 2. Core Concepts

### 2.1 Model

A model is a stored prototype subtree:

- model root node
- nested child structure
- default parameter values
- stable `DeclId` for every addressable field

### 2.2 Instance

An instance is a normal subtree cloned from a model prototype at creation time.

Each instance carries:

- `model_id`
- `model_version`
- `overrides` set
- `decl_index` (`DeclId -> NodeId`)

### 2.3 DeclId

`DeclId` is the stable semantic field key used for:

- targeting/binding
- override tracking
- propagation
- migration

It must be independent from runtime `NodeId`/UUID.

---

## 3. Dynamic Node Types

Each model is exposed as:

```text
model::<model_id>
```

This allows model instances to be treated like other node types by creation, validation, and UI listing.

---

## 4. Eager Instantiation

Creating `model::<model_id>`:

1. clone full prototype subtree
2. attach instance metadata
3. build instance `decl_index`
4. register instance root in `instances_by_model`

No lazy field creation.

---

## 5. Override Semantics

### 5.1 Representation

Minimal form:

```rust
HashSet<DeclId>
```

Values themselves live in instance parameters; override set only tracks ownership.

### 5.2 Detection

On instance field write:

1. resolve `DeclId`
2. compare instance value with model default (type-aware)
3. mark overridden if different
4. clear override if equal

### 5.3 Revert / Apply

- `revert(field)`: set instance to model default, clear override
- `revert_all`: reset all fields, clear overrides
- `apply(field set)`: write instance values into model defaults, then normal propagation runs

---

## 6. Model -> Instance Propagation

When model default for `DeclId d` changes:

- update each instance where `d` is not overridden
- skip instances where `d` is overridden

Instance enumeration:

```rust
instances_by_model: HashMap<ModelId, Vec<NodeId>>
```

---

## 7. Runtime Indexes

### 7.1 Per instance

```rust
decl_index: HashMap<DeclId, NodeId>
```

Used for O(1) field resolution.

### 7.2 Per model

```rust
decl_index: HashMap<DeclId, PrototypeNodeRef>
```

Used for default lookup and migrations.

---

## 8. Structural Evolution

### 8.1 Add field

- add node/parameter to each instance
- initialize from model default
- update `decl_index`
- mark non-overridden

### 8.2 Remove field

- remove node/parameter from each instance
- remove from override set and `decl_index`

### 8.3 Rename field

Require explicit rename mapping:

```text
old_decl_id -> new_decl_id
```

Transfer value + override metadata.

### 8.4 Versioning

- model stores monotonic `version`
- instances store `model_version`
- migrations run until versions match

---

## 9. Unified Type Catalog Integration

Do not introduce a separate "custom nodes factory".

Use one catalog with multiple providers:

- `BuiltinProvider` (macro/static compiled nodes)
- `ModelProvider` (`model::<id>`)

Conceptual creation contract:

```rust
struct CreateNodeRequest {
    node_type: NodeTypeId,
    label: String,
    placement: NodePlacement,
    origin: CreateOrigin,
}

struct NodePlacement {
    parent: NodeId,
    prev_sibling: Option<NodeId>,
}
```

`placement` is structural edit-time context only. It is not runtime evaluation context.

Benefits:

- one UI type list
- one validation path
- one persistence decode strategy by type id
- no architectural split between built-in and model types

---

## 10. Moving Nodes and Reevaluation

Nodes can move after creation.

That means:

- `MoveNode` still uses normal container/admission checks
- incremental reevaluation rebinding happens for affected dependencies
- context-dependent compiled bindings are updated from new ancestry

Creation placement and runtime context resolution remain separate.

---

## 11. Integration with Parameter Control Modes

If control modes are supported (`context`, `template`, `expression`, `proxy`, `binding`, `animation`), model propagation should distinguish value and control aspects.

Recommended key:

```rust
enum FieldAspect {
    Value,
    Control,
}

struct FieldKey {
    decl_id: DeclId,
    aspect: FieldAspect,
}
```

Semantics:

- model value changes propagate to non-overridden `Value`
- model control changes propagate to non-overridden `Control`
- instances can override one aspect without forcing override of the other

This avoids accidental resets of control setup when only value defaults change.

---

## 12. Engine Hooks (Required)

### Hook A: instance field changed

- resolve model + `DeclId`
- compare against model default
- update override ownership

### Hook B: model field changed

- enumerate instances for model
- propagate only to non-overridden fields

### Hook C: model structure changed

- diff old/new `DeclId` sets
- apply add/remove/rename migration
- rebuild indexes and override maps
- bump versions

---

## 13. Invariants

1. Instances are fully materialized immediately.
2. Addressable fields use stable `DeclId`.
3. Override ownership is explicit.
4. Non-overridden fields follow model.
5. Migrations are `DeclId`-driven, never runtime-id-driven.
6. Built-in and model node types share one catalog/create path.

---

## 14. Recommended Implementation Order

1. Unified node type catalog (built-in + model providers)
2. Catalog-based create/list API with explicit `NodePlacement`
3. Model registry and eager instantiation
4. Instance `decl_index` and value override propagation
5. Structural migration/versioning
6. Control-aspect support (`FieldAspect`) when control modes land
