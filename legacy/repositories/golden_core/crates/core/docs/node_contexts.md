# Dynamic Context System — Specification

*For `golden_core` (host-agnostic)*

---

## 1. Purpose

This document specifies a **Context System** that supports:

* **Owned context entries** (values and typed references) attached to nodes
* **Lexical (ancestor-based) symbol resolution** for any descendant consumer
* **Metadata access** on referenced nodes (e.g. log a referenced node’s name)
* **Dynamic context views** resolved at runtime using a **processing frame** (e.g. “current instance”)
* **Nested dynamic dimensions** (e.g. 8×8 ⇒ 64 lanes)
* **Stateful node memory partitioning** per contextual lane (e.g. smoothing filter per instance)

This system lives entirely in `golden_core`. Host applications (including, but not limited to, tools with fan-out/iteration constructs) can **use** it without the core depending on any particular high-level concept.

---

## 2. Core Concepts

### 2.1 Context Scope = Owned Parameter Subtree

A **Context Scope** is an owned parameter subtree attached to a node (or embedded in a node) and treated as a **lexical scope** for all descendant nodes.

Properties:

* Owned by the scope owner node
* Contains regular parameters, including folder/group nesting
* Entries may be pure values or typed references
* Descendants can resolve entries by symbol (name) using ancestor traversal

Example structure:

```
ContextProviderNode
 ├─ ContextScope
 │    ├─ songName: String
 │    └─ songSequence: NodeRef<Sequence>
 └─ Descendants...
```

> The scope owns its entries. “Owned value” entries do not exist anywhere else; “reference” entries store typed references to external entities.

---

### 2.2 Context Entry Types

#### A) Owned Value Parameters

Example: `songName: String`

* Stored physically in the Context Scope parameter tree
* No external binding
* Lifetime tied to the owner node

#### B) Owned Typed Reference Parameters

Example: `songSequence: NodeRef<Sequence>`

* The *parameter* is owned by the Context Scope
* The parameter’s *value* is a stable reference (NodeId / Guid / Path-like handle)
* Strongly typed and validated

Recommended parameter kinds:

```rust
enum ParamKind {
  // Existing kinds...
  NodeRef(NodeTypeConstraint),
  ParamRef(ParamTypeConstraint),
}
```

With constraints:

```rust
enum NodeTypeConstraint {
  Exact(NodeTypeId),
  Facet(FacetId), // capability-based typing
}
```

---

## 3. Symbol Resolution

### 3.1 Lexical Lookup (Ancestor Walk)

When resolving `{symbol}` from any node/parameter/script:

1. Start at the current node
2. Walk ancestors upward
3. Find the nearest Context Scope that defines `symbol`
4. Return the address of the entry parameter (e.g. `ParamAddr`)

Rules:

* **Nearest scope wins** (shadowing allowed)
* Missing symbol is an error (should include helpful diagnostics: nearest scopes and available symbols)

### 3.2 Cache Strategy (Recommended)

Resolution is frequent (UI, evaluation, scripting). Cache per consumer:

* Cache key: `(consumer_node_id, symbol_id, expected_type)`
* Cache value: `{ entry_param_addr, scope_owner_node_id, scope_generation }`

Invalidate when:

* Ancestor chain changes (reparenting)
* Scope generation changes (entry schema edited / renamed / moved)

---

## 4. References, Expressions, and Endpoints

### 4.1 Compiled Endpoint Representation

Any system that consumes values (links, expressions, scripts) should compile references into a stable plan:

```rust
enum EndpointRef {
  DirectParam(ParamAddr),

  ContextSymbol {
    symbol: SymbolId,
    expected: ContextType,
  },

  ContextView {
    base: Box<EndpointRef>,
    view: ViewSpec,
  },

  NodeMeta {
    node: NodeEndpoint,
    field: MetaField,
  },
}
```

Where:

* `ContextSymbol` resolves lexical scope → entry parameter
* `ContextView` adds dynamic addressing (runtime “which instance?”)
* `NodeMeta` projects metadata from a referenced node (see §5)

---

## 5. Node Metadata Access

### 5.1 NodeMeta: Built-in Read-only Projection

All nodes should expose a minimal, stable metadata surface:

```rust
enum MetaField {
  Name,      // user-visible display label
  TypeName,  // or NodeTypeId string
  Id,        // stable id/guid
  Path,      // optional, computed if available
}
```

### 5.2 Metadata Access Syntax (Conceptual)

Examples:

* `{songSequence.$name}`
* `{songSequence.$type}`
* `{songSequence.$id}`

Optional convenience:

* If a `NodeRef` is used in a string context without a field, default stringification may map to `$name`.

### 5.3 Evaluation Model

1. Evaluate `NodeRef` parameter (may be dynamic via a view)
2. Dereference to a concrete node
3. Read the metadata field

---

## 6. Dynamic Context Views

### 6.1 Motivation

A context entry is physically a single parameter, but consumers may request a **dynamic view**, such as:

* “Use the value for the **current processing instance**”
* “Use the value for the current instance of a specific dimension”
* “Use a specific index”

This enables patterns like:

* “per-object effect chain” (lighting): `{intensity@current_object}`
* “per-voice processing” (audio): `{cutoff@current_voice}`
* nested instances: `{value@current_outer, current_inner}`

### 6.2 Processing Frames

Runtime evaluation carries a **frame stack** describing the current instance context.

```rust
struct DimFrame {
  dim: DimId,
  index: u32,   // 0..extent-1 (if index-based)
  extent: u32,  // static count (if known)
  // optional: stable instance key (see §8)
  instance: Option<u64>,
}

struct EvalCtx {
  dim_stack: SmallVec<DimFrame>,
}
```

Frames are pushed/popped by **host-defined iteration/instance mechanisms**. These mechanisms may be implemented as nodes, engine-level loops, per-object processing, etc. The Context System does not depend on how frames are produced.

### 6.3 ViewSpec (Dynamic Addressing)

Dynamic context is expressed as a view over a base reference:

```rust
enum ViewSpec {
  Static,                 // default
  AtCurrent,              // nearest frame on the stack
  AtDimCurrent(DimId),    // explicitly selected dimension
  AtIndex(u32),           // constant index
  AtDimIndex(DimId, u32), // constant index for a dimension
}
```

Conceptual examples:

* `{songName}` → `Static`
* `{songName@current}` → `AtCurrent`
* `{songName@dim("Objects").current}` → `AtDimCurrent(Objects)`

### 6.4 Runtime Resolution

Evaluation pipeline:

1. `ContextSymbol` → resolve to entry `ParamAddr` (lexical, cached)
2. Apply `ContextView(view)` using `EvalCtx.dim_stack`
3. Produce a concrete value source (readable param, computed source, etc.)

---

## 7. Dimension Providers Use the System (They Don’t Define It)

The Context System defines:

* the **frame model** (`EvalCtx`)
* the **dynamic view semantics** (`ViewSpec`)
* how consumers request dynamic views

A host’s “instance mechanism” (whatever it is) only needs to:

* push/pop frames appropriately
* optionally provide projection behavior for dynamic views (below)

### 7.1 Optional: Projection Interface (Advanced)

Some hosts will want dynamic views to do more than “pick an index”; they may want dynamic views to **redirect** a reference based on instance selection (e.g. per-object parameter overrides, per-instance subgraphs).

For that, define an optional core trait:

```rust
trait InstanceProjection {
  fn project(
    &self,
    base: ParamAddr,
    dim: DimId,
    selector: InstanceSelector, // current/index
    eval: &EvalCtx
  ) -> ProjectedSource;
}

enum ProjectedSource {
  Param(ParamAddr),     // redirected param address
  ValueSource(SourceId),// computed/virtual source
  Missing,
}
```

This allows hosts to interpret dynamic views in a domain-specific way while still using the same core references and evaluation pipeline.

> This is optional. Many systems only need frame-driven state partitioning (see §8) and can keep references reading the same underlying param.

---

## 8. Stateful Nodes with Dynamic Inputs (Per-Lane Memory)

### 8.1 Problem

A stateful node (e.g. smoothing filter) reads a dynamic input (view depends on `EvalCtx`). It must keep independent memory per contextual lane.

Example: nested instances (8 × 8) must produce **64 independent memories**, not 1.

### 8.2 Solution: Partition Node State by Instance Key

There are two complementary strategies:

* **Dense lanes** (best when extents are static and relatively small)
* **Sparse lanes** (best when instance sets are dynamic/large)

Because you currently have static counts in some hosts, dense is the primary recommendation.

---

### 8.3 Dense Lanes (Static Extents, Hot-Path Optimal)

#### 8.3.1 Compile-Time Node State Metadata

When compiling a node (or when endpoints change), compute the dimensions that the node’s state depends on:

```rust
struct CompiledNodeStateMeta {
  state_dims: SmallVec<DimId>,
  state_extents: SmallVec<u32>,
  lane_count: usize, // product(state_extents)
}
```

Dimension dependency policy:

* Default: if a node uses any `@current`, depend on **all active dims** in its execution context.
* If a view explicitly names a dimension, that dimension is included.
* Optionally allow node-level override: “state depends on these dims only”.

#### 8.3.2 Flatten Nested Indices → Lane Index

Given dims in canonical order (outer → inner), compute:

```
flat = i0
flat += i1 * extent0
flat += i2 * extent0 * extent1
...
```

Example: extents `[8, 8]` (outer, inner):

* `flat = outer + inner * 8`
* lane_count = 64

#### 8.3.3 Stateful Storage

```rust
struct SmoothState {
  lanes: Vec<LaneState>,
}

struct LaneState {
  prev: f32,
  initialized: bool,
}
```

Per tick:

1. `lane = compute_lane_index(meta.state_dims, eval_ctx)`
2. `s = &mut lanes[lane]`
3. update smoothing memory for that lane

#### 8.3.4 Structural Changes

If the host changes extents/dims:

* rebuild `lanes` to the new `lane_count`
* reset memory (predictable and simple)

---

### 8.4 Sparse Lanes (Dynamic Instance Sets)

For hosts where instances are not a static grid (e.g. arbitrary object IDs), key by a stable instance identifier:

* key = tuple of `(DimId, instance_id)` for each relevant frame
* store in a hash map + (optional) LRU/TTL or provider-driven cleanup

This can coexist with dense lanes:

* dense for index-based static dims
* sparse for id-based dims

> Even if a host “doesn’t have fan-out nodes”, it can still push frames per object and provide stable `instance_id`s.

---

## 9. Nested Dimensions

Nested dynamic context is naturally represented by the `dim_stack`.

Example (two dims):

* outer dim: Tracks (extent 8)
* inner dim: Faders (extent 8)

Runtime stack:

```
[
  { Tracks, t, 8, instance=None },
  { Faders, f, 8, instance=None }
]
```

Dense lane_count = 8 × 8 = 64
Lane index computed by flattening.

---

## 10. Compile-Time Responsibilities

Compilation (or incremental recompilation when graph/links change) should produce:

1. Compiled `EndpointRef` plans
2. Resolved type expectations (`ContextType`)
3. Node state metadata (`CompiledNodeStateMeta`)
4. Optional precomputed helpers (e.g. dim → stack position mapping)

**No compilation work should occur in the hot processing loop.**

---

## 11. Runtime Responsibilities

The runtime must provide:

* An `EvalCtx` instance with a maintained `dim_stack`
* Endpoint evaluation that accepts `EvalCtx`
* Stateful nodes that index state by lane (dense) or key (sparse)
* Invalidation when:

  * scope schemas change
  * node graph structure changes
  * dimension configuration changes

---

## 12. Required Engine API Surface

### 12.1 EvalCtx

```rust
impl EvalCtx {
  fn push_dim(&mut self, dim: DimId, index: u32, extent: u32, instance: Option<u64>);
  fn pop_dim(&mut self);

  fn current_dim(&self) -> Option<&DimFrame>;
  fn find_dim(&self, dim: DimId) -> Option<&DimFrame>;
}
```

### 12.2 Endpoint Evaluation

```rust
fn evaluate(endpoint: &EndpointRef, eval: &EvalCtx) -> Value;
```

### 12.3 Lane Index Helper (Dense)

```rust
fn compute_lane_index(state_dims: &[DimId], state_extents: &[u32], eval: &EvalCtx) -> usize;
```

---

## 13. Determinism & Performance Guarantees

For dense-lane hosts:

* No per-tick allocations
* No hash maps on hot path
* O(1) lane indexing
* Stable behavior under nesting
* Predictable memory usage (`product(extents)`)

For sparse-lane hosts:

* Determinism depends on stable instance IDs
* Memory bounded by host policy (LRU/TTL/cleanup events)

---

## 14. Summary

| Capability            | Mechanism                                                              |
| --------------------- | ---------------------------------------------------------------------- |
| Owned context         | Parameter subtree attached to node                                     |
| Values vs references  | Owned value params + typed `NodeRef`/`ParamRef`                        |
| Symbol access         | Lexical ancestor lookup + caching                                      |
| Node metadata         | Built-in `NodeMeta` projection (`$name`, `$type`, `$id`, `$path`)      |
| Dynamic context       | `ViewSpec` applied using `EvalCtx` frame stack                         |
| Host independence     | Any system can push frames (nodes, engine loops, per-object pipelines) |
| Nested dimensions     | Frame stack + flattening                                               |
| Stateful partitioning | Dense lanes (static extents) or sparse keys (instance IDs)             |

---

## 15. Implementation Order (Recommended)

1. Context Scope as owned parameter subtree
2. Symbol resolution (`ContextSymbol`) + caching + diagnostics
3. Typed reference params (`NodeRef`, optionally `ParamRef`) + validation
4. Node metadata projection (`NodeMeta`)
5. `EvalCtx` frame stack + `ViewSpec` in endpoints
6. Compile-time extraction of state dimension dependencies
7. Dense lane state storage + flattening
8. (Optional) Sparse-lane keyed state for ID-based hosts
9. (Optional) Projection interface for advanced dynamic redirection

---

## 16. Placement Context vs Evaluation Context

These are different concepts and must stay separate:

- **Placement context** (creation/move time):
  - used to validate admission rules and choose insertion location
  - depends on target parent/container and edit intent
  - belongs to edit application / graph mutation
- **Evaluation context** (`EvalCtx`, processing time):
  - used to resolve dynamic views like `@current`
  - depends on active runtime frame stack
  - belongs to endpoint evaluation during processing

`EvalCtx` must not be used as node-creation API input.

---

## 17. Reparenting and Incremental Rebinding

When nodes move in the hierarchy, lexical context ancestry can change. The engine should:

1. mark affected subtrees and dependent endpoints as dirty
2. rerun context symbol resolution for those consumers
3. rebuild only impacted dependency/update metadata
4. preserve user-authored source specs; only compiled bindings are replaced

This is the right place to "set contexts" after structure edits: rebinding compiled references, not mutating authored context entries.

---

## 18. UI Context Discovery APIs

UI must query context information for a specific parameter efficiently (inspector menus, token completion, expression helpers).

Recommended engine-facing APIs:

```rust
fn ui_context_candidates_for_param(param: ParamAddr, expected: ContextType) -> Vec<UiContextCandidate>;
fn ui_context_symbol_lookup(consumer: NodeId, symbol: SymbolId, expected: ContextType) -> UiContextLookupResult;
```

Where each candidate includes:

- symbol name / id
- value type
- owning scope node
- lexical depth (shadowing clarity)
- resolved entry address
- compatibility result (exact/convertible/incompatible)

Caching should reuse resolver generations (graph structure + scope schema generations) so repeated UI requests stay O(k) in local candidates, not O(N) in graph size.

---

## 19. Integration with Parameter Control Modes

Context infrastructure is shared by multiple parameter control modes (see `parameters_control_modes.md`):

- direct context link mode
- token interpolation in text templates
- expression inputs
- proxy metadata projection when required

Using one endpoint/resolver path keeps behavior and diagnostics consistent across all modes.
