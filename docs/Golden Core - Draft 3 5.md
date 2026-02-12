# Golden Core - Draft 3.5
Version: 3.5

# Chapter 1 — Golden Core’s Role in Your App

Golden Core is the **authoritative runtime** at the centre of your application. It holds the live project state (values, structure, metadata), applies changes in a controlled way, and produces a deterministic stream of observable updates. Everything else in your app—UI, networking, device I/O, scripting, plugins—*talks to the Core* rather than mutating state directly.

If you picture your app as a concert system: Golden Core is the **stage manager**. Everyone can request changes (“bring lights up”), but only the stage manager actually commits them, in a known order, and everyone receives the same confirmed result.

## 1.1 What Golden Core provides (at a glance)

In practice, Golden Core provides:

- **A shared project model** that the whole app can inspect (UI, network peers, tools).
- **A single place where changes happen**, so your state can’t “split” across subsystems.
- **Determinism**, so the same inputs produce the same outputs (critical for sync, replay, debugging, networking).
- **A clean integration surface**: external systems express intent; the Core returns confirmed changes.

You don’t use Golden Core by “calling random setters everywhere”. You use it by:

1. expressing intent to change something,
2. letting the Core apply it,
3. reacting to what the Core says actually happened.

## 1.2 Everything is a node (the one entity you need to know)

Golden Core models your project as a **hierarchy of nodes**.

A node is the canonical “thing” in the system:

- a container of other nodes (folders, groups, managers),
- a parameter (a value you can edit),
- or any other structural entity your app needs.

At this stage, the only thing you need to internalise is:

> **If it exists in the project model, it is represented as a node somewhere in the hierarchy.**
> 

This matters because it means:

- UI inspectors can be generic (they inspect nodes),
- networking can mirror the same model (nodes + values + metadata),
- persistence and undo/redo can operate on the same primitives (nodes being created/edited/removed).

We’ll explain the “what kinds of nodes exist” and “how they’re stored” later. For now: **nodes are the universal building block**.

## 1.3 Stability comes from a Single Source of Truth

Golden Core enforces a simple rule:

> **The Core owns the authoritative state.**
> 
> 
> UI, network, and behaviours never “just mutate” state on their own.
> 

This makes your app stable in the real world:

- Multiple UI panels can show the same value without diverging.
- Network peers can converge on the same model.
- Undo/redo can be consistent because changes are recorded at the source.
- Debugging is tractable because there is one authoritative timeline of what changed.

So the UI is not “the truth”. A network peer is not “the truth”. A behaviour module is not “the truth”.

**Golden Core is.** 

## 1.4 Determinism via one execution entry point: `process()`

Golden Core’s runtime behaviour is intentionally funnelled through a single, well-defined execution entry point:

> **`process()` is where node logic runs.**
> 

You’ll see this as: “the engine runs passes, nodes execute their `process()`, and the model evolves deterministically”.

At the Chapter 1 level, the takeaway is simply:

- **No hidden callbacks everywhere**
- **No accidental re-entrancy**
- **No ‘whoever wrote last wins’ chaos across threads**

Instead, there is an explicit, deterministic progression: the Core decides *when* logic runs, and the system stays explainable.

We’ll later cover:

- how events reach nodes,
- why some nodes run only when needed,
- what “tick vs immediate flush” means,
- and what’s inside the context passed to `process()`.

Not now. For Chapter 1, the point is: **determinism is a feature of the execution model, not a hope.** 

## 1.5 A concrete mental model (typical app scenario)

Imagine your app has:

- a UI slider “Intensity”
- a remote tablet UI connected over the network
- a behaviour node that reacts to intensity changes

What happens:

1. The user moves the slider (UI expresses intent: “set Intensity to 0.8”).
2. Golden Core applies the change (authoritative commit).
3. Golden Core produces an observable update (“Intensity is now 0.8”).
4. Both UIs refresh from the same confirmed state.
5. The behaviour node runs `process()` and reacts deterministically to the new value.

In other words: **one truth, one timeline, consistent outcomes everywhere**.

---

# Chapter 2 — The Project Model (Nodes, Hierarchy, Metadata)

Golden Core represents your project as a hierarchy of **nodes**. A node is the one universal entity in the system: folders, parameters, and domain-specific elements are all nodes. This is what makes Golden Core easy to integrate: your UI, persistence, networking, and tooling can all work against the same model.

This chapter explains what a node is, what it contains, and how nodes form a coherent project tree. We intentionally avoid deeper runtime mechanics (events, scheduling, command application) until later.

## 2.1 Everything is a node

If something exists in the project model, it exists as a node somewhere.

That includes:

- structural organisation (folders, groups, containers),
- values you edit (parameters),
- domain entities (fixtures, cues, clips, devices…),
- and any “system element” you want visible and editable in the model.

This single-primitive design is what enables generic features:

- a single inspector UI that can navigate anything,
- a single persistence model that can save/restore anything,
- a single sync model that can mirror anything.

## 2.2 Hierarchy is structure, not execution order

Nodes live in a parent/child hierarchy. The hierarchy is for:

- ownership (“this belongs to that”),
- organisation and navigation (tree views, grouping),
- and shaping what gets saved/mirrored.

The hierarchy does **not** define evaluation order or dependencies. Execution is introduced later via the processing model; for now, treat the hierarchy as your project’s *structure*.

## 2.3 Anatomy of a node

A node is a self-contained unit that combines:

- a **runtime handle** (`NodeId`) used everywhere in the live engine,
- **intrusive structure links** (parent/child/siblings),
- **metadata** (`NodeMeta`) that makes it self-describing,
- a **data payload** (`NodeData`) that tells what this node *is*,
- and optional **behaviour** (executed under engine control).

```rust
/// Conceptual shape of a Golden Core node (simplified).
pub struct Node {
    // Runtime identity (slotmap key / fast handle)
    pub id: NodeId,

    // Intrusive hierarchy links
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,

    // Metadata (self-description + stable identifiers)
    pub meta: NodeMeta,

    // Payload (what the node is)
    pub data: NodeData,

    // Optional behaviour
    pub behaviour: Option<Box<dyn NodeBehaviour>>,
}

pub struct NodeMeta {
    // identifiers (details later)
    pub uuid: NodeUuid,
    pub decl_id: DeclId,
    pub short_name: ShortName,

    // universal flags
    pub enabled: bool,

    // human-facing metadata
    pub label: String,
    pub description: Option<String>,
    pub tags: Vec<String>,

    // tool hints
    pub semantics: SemanticsHint,
    pub presentation: PresentationHint,
}

pub enum NodeData {
    None,                     // purely structural
    Container(ContainerData),  // owns children + enforces structure policies
    Parameter(ParameterData),  // carries a value (Chapter “Parameters”)
    Custom(CustomData),        // domain-specific payload
}

/// Generic “this node can contain children” payload.
/// This is where authoring constraints and organisation capabilities live.
pub struct ContainerData {
    // What user/dev is allowed to place under this container (by type).
    pub allowed_types: AllowedTypes,

    // Whether this container allows folder organisation under it.
    // (Folders are nodes too; this flag declares the *capability*.)
    pub folders: FolderPolicy,

    // Optional container-specific limits/policies (kept generic here).
    pub limits: ContainerLimits,
}

pub enum AllowedTypes {
    Any,
    Only(Vec<NodeTypeId>),
}

pub enum FolderPolicy {
    Forbidden,
    Allowed,
}

pub struct ContainerLimits {
    pub max_children: Option<usize>,
    // other generic limits can exist, but keep this intentionally small
}

pub trait NodeBehaviour {
    fn process(&mut self, ctx: &mut ProcessCtx);
}

```

### 2.3.1 `NodeId` (runtime handle)

Inside a running engine, nodes are referenced by `NodeId`. This is the handle used by:

- structure links,
- references (with caching),
- lookups and iteration.

At this level, remember: **`NodeId` is the live engine handle.**

### 2.3.2 Intrusive hierarchy links

The hierarchy is stored with intrusive links (`parent/first_child/next_sibling/...`). You still think “tree”, but this representation makes structural edits stable and cheap without rebuilding vectors.

### 2.3.3 `NodeMeta` (self-description)

`NodeMeta` is the node’s self-description capsule:

- universal flags like `enabled`,
- user-facing labels and docs,
- and hints that let tools (UI, sync layers) interpret and present the node.

The identifiers (`uuid`, `decl_id`, `short_name`) exist on every node but are explained later.

### 2.3.4 `NodeData` (what this node *is*)

`NodeData` is the payload classification:

- structural nodes (`None`),
- containers (`ContainerData`),
- parameters (`ParameterData`),
- and custom domain payloads.

The important placement change here is: **“what can contain what” and “can I organise into folders” are container policies**, therefore they belong in `ContainerData`, not in some special “manager” variant.

### 2.3.5 Behaviour: optional logic, engine-controlled execution

Nodes may be passive (data only) or active (they compute / react). When behaviour exists, it runs through the engine-controlled entry point `process()`.

We’re not covering when/why `process()` runs yet—only that behaviour is attached to nodes and executed under engine control.

## 2.4 Nodes are the base you derive from

When you implement your own elements in Golden Core, you’re effectively defining node types that:

- participate in the same universal node model,
- provide metadata (including stable identifiers),
- define what payload exists (parameters / custom data / children),
- and optionally define behaviour (`process()`).

The benefit is consistency: the project can grow arbitrarily complex while the primitives remain the same, and tooling remains generic.

## 2.5 Example: reading a project as a node tree

A typical project view is simply a subtree of nodes:

- containers that organise,
- parameters that expose values,
- and domain nodes that hold meaning and behaviour.

Every subsystem speaks the same language:

- UI reads `NodeMeta` + `NodeData` to render editors,
- persistence saves nodes and their metadata/payload,
- networking mirrors node changes,
- behaviour runs on nodes under `process()`.

That’s the practical meaning of “Golden Core is the project model”.

---

# Chapter 3 — Managers (User-Extensible Subtrees)

A **Manager** is a container node designed for end users: it is the place in the project where users can **create, remove, and organise** their own nodes from a curated list of allowed types.

Managers are how you expose “user-authored subgraphs” without inventing a new concept in the engine. They are still just nodes:

- structurally: `NodeData::Container(ContainerData { ... })`
- ergonomically: a node type that provides higher-level helper methods

## 3.1 What a Manager does in an app

From the end user’s perspective, a Manager is typically a panel/tree section like:

- “Mappings”
- “Automations”
- “Devices”
- “Cues”
- “Outputs”

Inside that section, the user can:

- click “+” and pick a node type from a list (palette),
- organise items into folders,
- reorder items,
- delete items.

That is the Manager’s role: **a curated, user-extensible subtree**.

## 3.2 Why Manager is not a new `NodeData` variant

Manager is a *pattern*, not a primitive. If we added `NodeData::Manager`, we would be baking a particular UI workflow into the engine’s core model.

Instead:

- the engine only needs a generic “container with policies” (`ContainerData`)
- managers are implemented as **node types** that:
    - configure those container policies (allowed types, folders allowed),
    - and expose convenience methods for dynamic edits.

This keeps the core minimal and keeps your app free to define different manager behaviours.

## 3.3 Manager capabilities live in `ContainerData`

A Manager’s “what users can add here” comes from container policies:

- `allowed_types`: the palette (curated list of node types)
- `folders`: whether folder organisation is allowed
- optional limits: max children, etc.

Conceptually:

```rust
#[derive(GoldenNode)]
#[container(allowed = ["MappingItem", "Folder"], folders = "Allowed")]
pub struct MappingManager {
    #[node_id]
    pub id: NodeId,

    // manager’s own parameters (filters, view mode, etc.) are just normal parameters
    #[param(default = true)]
    pub enabled: ParameterHandle<bool>,
}
```

## 3.4 Managers provide helper methods (the authoring surface)

Managers should offer a small, explicit helper API that matches user intent:

- create a child of a chosen type,
- remove a child,
- create/move into folders,
- query/manipulate the managed children as a collection.

Important: these helpers are **the public developer API**. Internally they will enqueue deterministic edits, but callers shouldn’t need to care about the underlying command machinery here.

Conceptual examples (shape only):

```rust
impl MappingManager {
    pub fn add_item<T: GoldenNode>(&self, ctx: &mut ProcessCtx, init: impl FnOnce(&mut T)) -> NodeId {
        // create a new child node of type T under this manager
        // apply init closure to set initial parameters/meta
        todo!()
    }

    pub fn remove_item(&self, ctx: &mut ProcessCtx, item: NodeId) {
        todo!()
    }

    pub fn create_folder(&self, ctx: &mut ProcessCtx, label: &str) -> NodeId {
        todo!()
    }

    pub fn move_to_folder(&self, ctx: &mut ProcessCtx, item: NodeId, folder: NodeId) {
        todo!()
    }

    pub fn items<'a>(&self, ctx: &'a ProcessCtx) -> impl Iterator<Item = NodeId> + 'a {
        todo!()
    }
}
```

## 3.5 Example scenarios

### Scenario A — “Mappings” manager

- Allowed types: `MappingItem`, `Folder`
- User creates 10 mapping items, organises into folders (“Lights”, “Video”).
- The UI tree is readable; the engine model remains generic.

### Scenario B — “Devices” manager

- Allowed types: `OscDevice`, `ArtNetDevice`, `MidiDevice`, `Folder`
- The manager can expose helper methods like “enumerate all devices”, “find device by label”, “disable all devices”, without any special engine support.

---

## Proposal for the next chapter

Chapter 4 — Parameters as Nodes (Values, Meaning, Control)

---

# Chapter 4 — Parameters as Nodes (Values, Meaning, Control)

Parameters are the value-carrying nodes of Golden Core. If something can be edited by a user, driven by automation, mirrored to peers, or persisted as part of a project, it should usually be represented as a parameter node.

At the surface level, a parameter node is:

- a node whose payload stores a **typed value**,
- whose meaning and editor behaviour come from the node’s metadata,
- and whose runtime/persistence behaviour is controlled by a small set of policies.

This chapter defines the conceptual shape of parameters without yet introducing the runtime mechanics (events, inbox ordering, command application).

## 4.1 A parameter is “a value node”

A parameter is a node where:

- `node.data = NodeData::Parameter(ParameterData { ... })`
- `node.meta` carries the human/tool-facing description and interpretation hints

The separation is deliberate:

- `NodeMeta` answers: **“What is this and how should tools interpret/present it?”**
- `ParameterData` answers: **“What is the current value, and what policies/constraints apply when it changes?”**

## 4.2 Anatomy of a parameter (canonical shape)

```rust
/// Conceptual parameter payload (simplified).
/// Interpretation (units, intent, UI) lives in NodeMeta.
/// Value + policies + hard constraints live here.
pub struct ParameterData {
    pub value: Value,
    pub default: Option<Value>,

    // Core parameter knobs
    pub read_only: bool,      // engine-enforced: cannot be set through normal edits
    pub update: UpdatePolicy, // latency / when the update becomes visible
    pub save: SavePolicy,     // persistence strategy
    pub change: ChangePolicy, // when "a change" should be emitted

    // Type-aware hard constraints (only what depends on the value type)
    pub constraints: ValueConstraints,
}

/// Canonical value domain used by parameters.
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),

    Vec2 { x: f64, y: f64 },
    Vec3 { x: f64, y: f64, z: f64 },
    ColorRgba { r: f64, g: f64, b: f64, a: f64 },

    // Discrete action (no separate ParameterKind needed)
    Trigger,

    // Selected variant of a schema-registered enum definition
    Enum { enum_id: EnumId, variant: EnumVariantId },

    // Reference to another node (persistable identity + optional runtime cache)
    Reference(ReferenceValue),
}

pub struct ReferenceValue {
    pub uuid: NodeUuid,
    pub cached_id: Option<NodeId>,
}

pub enum UpdatePolicy {
    Immediate,
    EndOfTick,
    NextTick,
}

/// Whether a write that results in the same value should still be treated as a change.
pub enum ChangePolicy {
    ValueChange, // emit only if value != old_value
    Always,      // emit even if value == old_value
}

/// Persistence strategy for this parameter node.
pub enum SavePolicy {
    None,  // never persisted (derived/transient)
    Delta, // persist only overrides relative to schema/default
    Full,  // persist full payload (used for dynamic / standalone definitions)
}

/// Type-specific constraints (read_only is NOT here; it is a core parameter knob).
pub enum ValueConstraints {
    None,

    Int {
        min: Option<i64>,
        max: Option<i64>,
        clamp: bool,
        step: Option<i64>,
    },

    Float {
        min: Option<f64>,
        max: Option<f64>,
        clamp: bool,
        step: Option<f64>,
    },

    String {
        max_len: Option<usize>,
        pattern: Option<String>, // optional engine-defined pattern constraint
    },

    Enum {
        enum_id: EnumId,
        allowed: Vec<EnumVariantId>,
    },

    Reference {
        target: ReferenceTargetHint,
    },
}
```

This shape is intentionally compact and consistent:

- There is no separate “parameter kind”: **Trigger is a value type**.
- Vectors and colours use **`f64`** to match `Float(f64)` and avoid precision mismatch.
- Semantics/presentation are not duplicated: they remain in `NodeMeta`.

## 4.3 Meaning and UI come from NodeMeta (no duplication)

Parameters need more than raw values: tools must know what the value *means* and how to edit it safely.

That information belongs in `node.meta`, not inside `ParameterData`:

- **Semantics hints** describe meaning (units, intent like “gain”, “intensity”, “tempo”…).
- **Presentation hints** describe preferred editors (slider, colour picker, dropdown, button…).
- Label/description/tags make parameters discoverable and self-documenting.
- `enabled` applies uniformly to parameters because parameters are nodes.

`ParameterData.constraints` is different: it is the **engine-enforced** validity layer (ranges, allowed variants, reference targeting). It should be sufficient to prevent invalid states even if a UI is buggy or a peer sends bad edits.

## 4.4 Policies: update, change, save

Parameters expose a few policies that define how they behave as part of the core state model.

### UpdatePolicy

Controls when a write becomes visible/applied at the engine level:

- `Immediate` for responsive control surfaces,
- `EndOfTick` for end-of-tick micro-step stabilization,
- `NextTick` for tick-aligned or stability-sensitive changes.

The deeper mechanics of “tick vs flush” are defined later; here, treat this as a user-facing knob to choose responsiveness vs coalescing.

### ChangePolicy

Controls what counts as “a change”:

- `ValueChange`: only emit when the value actually differs from the previous value.
- `Always`: emit on every write attempt, even if the value compares equal.

This is useful for “reassert/retrigger” semantics where “setting the same value again” should still notify downstream observers.

### SavePolicy

Controls how the parameter is persisted:

- `None`: not saved (derived/transient values).
- `Delta`: only save overrides relative to schema/defaults.
- `Full`: save the full parameter payload (typically for dynamic/standalone items).

The exact JSON/project-file shape is covered in the persistence chapter; this is the conceptual lever.

## 4.5 Enums: value stores ids, schema stores definitions

For enum parameters, the parameter’s `Value::Enum { enum_id, variant }` stores only:

- which enum definition is being used (`enum_id`),
- and which variant is selected (`variant`).

The enum definition itself (variants, labels, ordering) is stored in the **schema/registry**, so multiple parameters can share the same option set and tools can render consistent dropdowns.

We’ll detail where and how enums are registered in the chapter about schema and declarations.

## 4.6 Examples

### Example A — Intensity (float)

- `value = Float(0.8)`
- `constraints = Float { min: Some(0.0), max: Some(1.0), clamp: true, step: Some(0.01) }`
- `change = ValueChange`
- `update = Immediate`
- `save = Delta`
- `node.meta.semantics`: intent “intensity”, unit “%”
- `node.meta.presentation`: slider

### Example B — Reset (trigger)

- `value = Trigger`
- `constraints = None`
- `change = Always` (common for triggers)
- `update = Immediate`
- `save = None` (often transient) or `Delta` depending on your persistence rules
- `node.meta.presentation`: button
- `node.meta.semantics`: intent “reset”

### Example C — Blend mode (enum)

- `value = Enum { enum_id: BLEND_MODE, variant: ADD }`
- `constraints = Enum { enum_id: BLEND_MODE, allowed: [NORMAL, ADD, MULTIPLY, SCREEN] }`
- `update = NextTick` (if you want stable tick-aligned application)
- `save = Delta`
- `node.meta.presentation`: dropdown
- `node.meta.semantics`: intent “blend_mode”

### Example D — Target reference

- `value = Reference(ReferenceValue { uuid, cached_id })`
- `constraints = Reference { target: ParameterOnly /* (hint) */ }`
- `save = Delta`
- `node.meta.presentation`: reference picker
- `node.meta.semantics`: intent “target”


## 4.7 Stable identity and declared bindings (`uuid`, `decl_id`, `short_name`)

Golden Core uses **three** common identifiers, each with a distinct job:

- **`uuid`**: stable identity across save/load and across runtime allocations. References persist **only** the UUID.
- **`decl_id`**: stable *schema binding key* **within a parent scope**. It is used to locate *declared* structure during load and to keep patches stable across renames.
- **`short_name`**: human/tool-friendly local naming. Useful for UI and addressability, but **not** a persistence binding key.

### `decl_id` is local, not global

A `decl_id` is interpreted under a specific parent node. This is the rule that makes persistence deterministic:

> A Delta record `{ decl_id: "host" }` is resolved *only* among the declared children of the current parent scope.

### Potential slots (declared identity, optional existence)

Some declared identities are **optional**: they reserve a `decl_id` even when no node instance exists at runtime. This is the “potential node/parameter” concept:

- The parent schema declares an **optional slot** (stable `decl_id`, e.g. `"value"`).
- The slot may be absent, or it may be materialised as one of several allowed concrete node types.

This matters for persistence:

- A *present* potential slot must be saved as **Full** (because its concrete type must be recreated),
- but it still carries `decl_id` so it attaches to the reserved slot on load (it must not create a sibling).

The full persistence rules are formalised in Chapter 12.


---

# Chapter 5 — Creating Nodes (Macros, Declared Structure, and Dynamic Helpers)

Golden Core is strict about how the model changes, but it should be *pleasant* to author nodes. Node authors should spend their time describing **what exists** (parameters, children, folders) and writing behaviour—not writing boilerplate to “wire things into the engine”.

This chapter focuses on the **developer-facing authoring surface**:

- how you create your own node types,
- how you declare parameters and child nodes using macros/attributes,
- how you dynamically add/remove nodes (and sometimes parameters) using ergonomic helpers,
    
    without going into the underlying “command” machinery.
    

## 5.1 Creating your own node types with `#[derive(GoldenNode)]`

The canonical way to define a node type is to write a Rust struct and derive `GoldenNode`. The derive macro generates the schema information and the runtime binding glue so:

- the engine can allocate the node and its declared structure,
- the UI/network layers can introspect the node’s structure consistently with compiled code.

Minimal example:

```rust
#[derive(GoldenNode)]
pub struct DelayEffect {
    #[node_id]
    pub id: NodeId,

    #[param(default = 0.5, min = 0.0, max = 1.0)]
    pub feedback: ParameterHandle<f64>,

    #[param(default = Trigger, semantics = "Trigger", behavior = "Append")]
    pub panic: ParameterHandle<Trigger>,
}
```

The key mental model: the struct is your *authoring surface*, while the engine still stores “everything as nodes”.

## 5.2 Declaring parameters with `#[param(...)]`

`#[param(...)]` declares a **parameter node** owned by your node type and binds it to a typed handle field. The doc’s examples emphasise:

- defaults (`default = ...`),
- constraints (`min/max/...`),
- semantics/presentation hints for tooling,
- and inbox behaviour like `behavior = "Append"` (notably for triggers).

### 5.2.1 Potential parameters / potential children (optional slots)

Sometimes a node must expose a child that is **schema-reserved** but **optional**, and whose concrete type can vary at runtime.

Typical pattern:

- a parameter `my_type` selects a mode (`Float`, `Color`, `Nothing`…)
- a slot `value` is materialised accordingly:
  - `Float` → `value` exists and is a float parameter
  - `Color` → `value` exists and is a colour parameter
  - `Nothing` → `value` is absent

Model this as an **optional slot** in the schema:

- the slot has a stable `decl_id` (e.g. `"value"`) even when absent
- if present, it must be one of an allowed set of node types / parameter value domains

#### Authoring surface (recommended)

Conceptually (macro names are illustrative):

```rust
#[derive(GoldenNode)]
pub struct TypedValue {
    #[node_id]
    pub id: NodeId,

    #[param(default = MyType::Float)]
    pub my_type: ParameterHandle<MyType>,

    #[potential_child(
        decl_id = "value",
        allowed = ["Parameter(Float)", "Parameter(ColorRgba)"],
        preserve_uuid = true
    )]
    pub value: PotentialSlotHandle,
}
```

#### `PotentialSlotHandle`

`PotentialSlotHandle` is a **schema-reserved optional child slot**: it represents a *declared identity* (`decl_id`) that may or may not be materialised as a runtime child node.

It exists to model “a child node that is optional and whose concrete type can vary”, without treating that child as a free-floating dynamic node.

**Key properties**

- **Stable identity:** the slot is addressed by its `decl_id` *within the parent scope* (exactly like a declared child).
- **Optional presence:** the slot may be **absent** (no runtime child node attached).
- **Polymorphic concrete type:** when present, the attached child may be one of a constrained set of types/kinds (declared in the schema, e.g. `Parameter(Float)` or `Parameter(ColorRgba)`).
- **UUID stability (recommended):** when `preserve_uuid = true`, changing the slot’s concrete type **replaces** the attached node while keeping the same UUID, improving UI bindings and Undo/Redo.

**What it contains (conceptual)**

A `PotentialSlotHandle` is *not* itself a node. It is a handle stored on the parent node that tracks:

- the slot `decl_id` (compile-time constant from the schema),
- an optional cached `NodeId` / `Uuid` for the currently materialised child (if any).

**Recommended API surface (illustrative)**

- `is_present(&self) -> bool`
- `uuid(&self) -> Option<NodeUuid>` (or always returns the slot UUID if you reserve it even when absent)
- `get(&self, ctx) -> Option<NodeId>`
- `ensure_parameter(&mut self, ctx, kind: ParamKindSpec) -> NodeId`
- `clear(&mut self, ctx)`
- `replace(&mut self, ctx, new_type: NodeTypeSpec) -> NodeId` (emits `ChildReplaced`)

> Important: `PotentialSlotHandle` operations are **structural edits** (add/remove/replace child), not “value edits”.



#### Runtime helpers (recommended)

Your node behaviour should not “mutate a parameter type in place”. Treat a type switch as a **structural edit** on the slot:

- `ensure_value_slot(kind)` → materialise or replace the slot node as the requested kind
- `clear_value_slot()` → remove the slot node instance

If `preserve_uuid = true`, a replacement keeps the slot UUID stable across `Float ↔ Color` swaps, improving:

- UI bindings stability,
- reference behaviour,
- Undo/Redo coherence.

#### Persistence consequences

Potential slots sit between “declared” and “dynamic”:

- they are **declared identities** (so they use `decl_id`)
- but they must be persisted as **Full** when present (because concrete type must be recreated)

Therefore Golden Core allows a **Full record that also carries `decl_id`** *only* for potential slots. Chapter 12 defines the exact file rules and loader behaviour.

Example:

```rust
#[derive(GoldenNode)]
pub struct OscOutput {
    #[node_id]
    pub id: NodeId,

    #[param(default = "127.0.0.1", folder = "connection")]
    pub host: ParameterHandle<String>,

    #[param(default = 9000, min = 1, max = 65535, folder = "connection")]
    pub port: ParameterHandle<i64>,
}
```

## 5.3 Declaring structural children: slots, containers, and folders

Many nodes own more than parameters: they own other nodes.

The draft distinguishes three patterns.

### 5.3.1 Static child “slots” (schema-declared)

Use this when a node type has well-known, always-present children (or well-known lists):

```rust
#[derive(GoldenNode)]
pub struct StateMachine {
    #[node_id]
    pub id: NodeId,

    #[child(slot = "states", kind = "List", allowed = "State")]
    pub states: ChildListHandle,

    #[child(slot = "transitions", kind = "List", allowed = "Transition")]
    pub transitions: ChildListHandle,
}
```

Slots are mainly for ergonomics and introspection: they let tools reason about *what this node can contain* without hardcoding application rules.

### 5.3.2 Dynamic containers (`#[container(...)]`)

Use this when children are user-authored or runtime-authored (graphs, mapping layers, managers). The node declares “I can contain children”, optionally constrained by allowed types:

```rust
#[derive(GoldenNode)]
#[container(allowed = ["MappingItem", "Automation", "Folder"])]
pub struct MappingLayer {
    #[node_id]
    pub id: NodeId,
}
```

### 5.3.3 Folder groupings with flat access (recommended)

Folders are real nodes in the hierarchy, used to organise the tree for humans—while your Rust API remains flat on the owning node.

```rust
#[derive(GoldenNode)]
pub struct OscOutput {
    #[node_id]
    pub id: NodeId,

    #[folder(slot = "connection")]
    pub connection: FolderHandle,

    #[folder(slot = "timing")]
    pub timing: FolderHandle,

    #[param(default = "127.0.0.1", folder = "connection")]
    pub host: ParameterHandle<String>,

    #[param(default = 9000, min = 1, max = 65535, folder = "connection")]
    pub port: ParameterHandle<i64>,

    #[param(default = false, folder = "timing")]
    pub flush_immediate: ParameterHandle<bool>,
}
```

This avoids “lookup-by-path” in hot code while keeping the external tree readable.

## 5.4 Compact declarations: `params!{...}` (option B)

When a node declares many parameters, attribute-heavy structs become noisy. The draft proposes a `params!{...}` DSL that generates the same schema and the same handles, but keeps declarations compact. It explicitly supports folder grouping (1 level and 2 levels).

No folders:

```rust
impl DelayEffect {
  params! {
    feedback:  f64 = 0.5  [0.0..1.0];
    delay_ms:  f64 = 120.0 (sem="Time", unit="ms", min=0.0, max=10_000.0);
    panic:     Trigger (sem="Trigger", behavior="Append");
  }
}
```

Folder grouping (1 level):

```rust
impl OscOutput {
  params! {
    folder(output, label="Output") {
      host: String = "127.0.0.1";
      port: i64    = 9000 (min=1, max=65535);
    }

    folder(timing, label="Timing") {
      timeout_ms: i64 = 250 (min=0, max=10_000, unit="ms");
      flush_now:  Trigger (behavior="Append");
    }
  }
}
```

Folder grouping (2 levels):

```rust
impl VideoMapper {
  params! {
    folder(output, label="Output") {
      folder(color, label="Colour") {
        gamma:      f64 = 2.2 (min=0.1, max=6.0);
        saturation: f64 = 1.0 (min=0.0, max=2.0);
      }
    }
  }
}
```


## 5.4.1 Direct-access aliases (`alias` / `direct_access`)

Nested folders are *real nodes* in the declared hierarchy. That means the canonical way to reach a parameter is its **structural path** (e.g. `self.output.color.gamma`), which matches persistence and load-time `decl_id` resolution.

For ergonomics, the `params!{...}` DSL MAY also generate **owner-level aliases** that point to an existing declared parameter handle. Aliases are **pure Rust conveniences**:

- they do **not** create additional parameters,
- they do **not** affect `decl_id`,
- they do **not** change persistence format (the tree is still the source of truth).

### Parameter-level alias

A parameter MAY declare an explicit alias name:

```rust
impl VideoMapper {
  params! {
    folder(output) {
      folder(color) {
        gamma: f32 = 2.2 (min=0.1, max=6.0, alias="gamma");
      }
    }
  }
}
```

This generates both:

- `self.output.color.gamma` (structural)
- `self.gamma` (alias to the same handle)

### `direct_access`

As a shorthand, a parameter MAY request direct access using its own identifier:

```rust
gamma: f32 = 2.2 (direct_access);
```

This is equivalent to `alias="gamma"`.

### Folder-level alias prefix

Large groups MAY provide a prefix to reduce manual alias naming:

```rust
folder(color, alias_prefix="color_") {
  gamma: f32 = 2.2 (direct_access); // => self.color_gamma
}
```

### Compile-time conflict checking (MUST)

The macro MUST reject alias generation if it would create ambiguous or colliding identifiers on the owner node.

At minimum, the macro MUST emit `compile_error!` when:

- two distinct parameters request the same alias name, or
- an alias name collides with a generated folder/parameter accessor name produced by the DSL.

(If aliases are generated via `#[derive(GoldenNode)]` on the struct, the derive MAY additionally check collisions against user-declared fields. For `impl Type { params!{...} }`-style expansion, Rust’s normal name resolution will still catch many conflicts, but the macro should catch *its own* collisions deterministically.)


## 5.5 Dynamic graph edits: use helpers, not boilerplate

Even though structure edits are *defined* internally as deterministic model mutations, node authors should primarily interact with them through **ergonomic helpers** that enqueue the correct edits through the context.

The draft explicitly calls out these helpers as the developer-facing surface. Typical examples include:

```rust
// Create a child node under `parent`, with a typed init closure.
let child_id = parent.create::<T>(ctx.edit(), |init| {
    // set initial parameter values / init payload
});

// Remove a node (policy hidden behind helper defaults).
node.remove(ctx.edit());

// Move / reparent / reorder (via explicit helper methods).
node.move_to(ctx.edit(), new_parent);

// Patch metadata ergonomically.
node.meta(ctx.edit())
    .label("New Name")
    .description("...")
    .apply();
```

### 5.5.1 Helper design rule: keep semantics explicit

The draft warns against surprising implicit behaviour (e.g. a “create” that silently turns into “move”). Instead, helper methods should keep intent explicit (e.g. `attach_existing(...)` instead of overloading `create(...)`).

## 5.6 Dynamically adding parameters (advanced pattern)

Most parameters are declared via macros so the schema stays consistent with code and tools. Dynamic parameters are the exception, used for cases like:

- user-authored lists where each item carries its own parameters,
- generated sub-graphs based on user content,
- or adapters that must materialise parameters from external descriptors.

When you do need dynamic parameters, the authoring goal is the same as dynamic nodes:

- provide helper methods that create/remove the parameter nodes and bind them into the container,
- keep the operation deterministic and tool-visible,
- and avoid “special-casing parameters vs other nodes” at the model layer.

(Implementation details of how persistence treats dynamic parameters are covered later; this chapter’s focus is the authoring surface.)

---

# Chapter 6 — Engine Cycle and Node Execution Modes

Golden Core executes the whole project by running a deterministic engine cycle over the node graph. As a node author, you don’t decide *when* your node runs; you decide *what kind of node you are* (passive, reactive, continuous) and implement the corresponding methods. The engine then calls those methods at well-defined points of the cycle.

This chapter focuses on:

- the graph loop cycle at a high level,
- the three execution modes (passive / reactive / continuous),
- the methods exposed by each mode,
- the distinction between the normal tick loop and `flushImmediate`,
- and the end-of-tick stabilisation micro-steps that avoid multi-tick dependency latency.

We intentionally do **not** explain the inbox/event model here; we only mention it when needed to make the cycle understandable. The next chapter covers inboxes and events in detail.

## 6.1 The engine progresses in phases

Golden Core has two ways to progress:

1. **Normal tick loop** (`EngineTick`)
    
    This is the main loop. Time advances, continuous nodes update, reactive nodes process, and then the engine stabilises the graph for the tick.
    
2. **Immediate flush phase** (`FlushImmediate`)
    
    This is a micro-step phase used to propagate certain changes immediately. It runs without advancing time and without calling continuous updates.
    

Both phases execute node logic under engine control. They differ in *what is allowed to run* and *when they can occur*.

## 6.2 Node execution modes

Golden Core distinguishes nodes by whether they need to run at all, and if so, under what conditions.

### 6.2.1 Passive nodes (data-only)

Passive nodes do not execute code. They exist to hold structure and/or values:

- folder nodes,
- many containers,
- many parameter nodes (as data holders).

Passive nodes still fully participate in the model (they can be edited, referenced, persisted); they simply have no behaviour to run.

### 6.2.2 Reactive nodes (run when relevant)

Reactive nodes execute logic only when the engine decides there is relevant work to do (typically because something affecting them changed since the last time they ran). They implement:

```rust
pub trait NodeReactive {
    fn process(&mut self, ctx: &mut ProcessCtx);
}
```

`process()` is the canonical reactive entry point.

### 6.2.3 Continuous nodes (also run every normal tick)

Some nodes must run every normal tick regardless of explicit changes:

- time-based generators (LFO, envelopes),
- interpolators that advance over time,
- periodic pollers.

These nodes are continuous: reactive nodes that also receive a per-tick update:

```rust
pub trait NodeContinuous: NodeReactive {
    fn update(&mut self, ctx: &mut ProcessCtx);
}
```

Key rule: **`update()` is called only in the normal tick loop**, never during micro-steps (`FlushImmediate` or end-of-tick stabilisation).

## 6.3 Lifecycle hooks (`init` and `destroy`)

Nodes often need one-time setup/teardown (device connection, resource allocation, graceful stop). Golden Core provides deterministic lifecycle hooks executed under engine control:

```rust
pub trait NodeLifecycle {
    fn init(&mut self, ctx: &mut ProcessCtx) { }
    fn destroy(&mut self, ctx: &mut ProcessCtx) { }
}
```

- `init()` runs once when the node becomes live.
- `destroy()` runs once when the node is removed or the engine stops.

## 6.4 What `ProcessCtx` is (only what you need for the cycle)

Every execution method receives a `ProcessCtx`. In this chapter, you only need to know:

- it provides read access to the model,
- it provides the safe surface to emit edits,
- it carries phase/origin information so the engine can enforce the correct behaviour.

The inbox/event surface inside `ProcessCtx` is introduced in the next chapter.

## 6.5 Normal tick loop (high-level flow)

A normal engine tick progresses like this:

1. **Prepare tick**
- advance engine timing (tick/time context),
- determine which nodes need work this tick.
1. **Continuous update pass**
- for each scheduled continuous node: call `update(ctx)`
- this is where time-driven progression happens.
1. **Reactive process pass**
- for each node selected to run reactively: call `process(ctx)`
- reactive nodes respond to changes accumulated since their last run.
1. **Apply edits**
- edits emitted during `update()`/`process()` are applied deterministically by the engine.
- applying edits may produce downstream effects (new pending work).
1. **End-of-tick stabilisation (auto micro-steps)**
- if the model changes produced new pending work for other nodes, the engine performs micro-steps to converge the graph *within the same tick*, up to a bound.
- these micro-steps call `process(ctx)` only (no `update()`), and do not advance time.

The stabilisation step exists to avoid “dependency chain latency” where A depends on B depends on C and would otherwise take multiple ticks to converge.

## 6.6 End-of-tick stabilisation micro-steps

At the end of a tick, once the main passes have run and edits have been applied, the engine checks whether there is remaining pending work (conceptually: nodes that received new inputs/events during this tick and therefore should run).

If there is, the engine runs **stabilisation rounds**:

- Each round selects the nodes that became newly relevant.
- The engine calls `process(ctx)` for those nodes in a deterministic order.
- The engine applies any edits they emit.
- If that produces new pending work, another round occurs.

This repeats until:

- there is no pending work (the graph is quiescent), or
- a safety bound is reached.

Important properties:

- stabilisation does **not** call `update()`,
- stabilisation does **not** advance time/tick counters,
- stabilisation is deterministic (selection and ordering are stable),
- stabilisation is bounded (to prevent pathological graphs from consuming the whole frame budget).

The exact definition of “pending work” and how inboxes are tracked is covered in the next chapter.

## 6.7 `FlushImmediate` (micro-steps that interrupt the normal flow)

`FlushImmediate` is a separate micro-step mechanism that can occur outside the “end-of-tick stabilisation” moment.

What matters here:

- `FlushImmediate` can run *immediately* in response to specific operations that require instant propagation.
- It “pauses” the normal flow to converge the model right away.
- It may call `process(ctx)` on reactive nodes as needed.
- It does **not** call `update()`.
- It does **not** advance time.

So:

- **End-of-tick stabilisation** is “clean-up convergence at the end of a normal tick”.
- **FlushImmediate** is “urgent convergence right now”.

## 6.8 Two short scenarios

### Scenario A — Normal tick with a continuous node

- The engine enters `EngineTick`.
- A continuous node receives `update(ctx)` and advances its internal phase.
- Some nodes run `process(ctx)` based on what is relevant this tick.
- Edits are applied.
- Stabilisation micro-steps run (if needed) until quiescent (or bounded).
- The tick ends; time advanced exactly once.

### Scenario B — Immediate propagation without advancing time

- A value is set in a way that requires immediate propagation.
- The engine enters `FlushImmediate`.
- Reactive nodes may run `process(ctx)` to converge the model.
- No continuous `update()` runs; no time advances.
- The engine returns to the normal loop.

---

# Chapter 7 — Inboxes and Events (What Nodes Receive, Ordering, and How `process()` Is Driven)

Golden Core is deterministic because node execution is driven by a single, ordered stream of facts: **events**. Nodes do not “react immediately” when something happens; instead, the engine records what happened as events, routes them to interested nodes, and nodes consume those events through their **inbox** when they run.

This chapter explains:

- what an event is,
- what an inbox is,
- how events are routed (subscriptions and bubbling at a high level),
- the ordering guarantees,
- and how inbox state determines which nodes get scheduled for `process()` (normal tick, stabilisation micro-steps, and `flushImmediate`).

## 7.1 Events are facts, not callbacks

An **event** is a record that something happened in the model:

- a parameter value changed,
- a node was created or removed,
- a child was added or reordered,
- metadata changed (label/enabled/etc.).

Events are produced by deterministic edits applied by the engine. They are not arbitrary user callbacks.

Key principle:

- **Edits mutate state.**
- **Events describe the resulting mutation.**

Nodes observe events; they do not intercept edits inline.

## 7.2 The inbox: per-node event queue since last run

Each node that can run has an **inbox**, which contains the events that node has received since the last time it processed them.

Conceptually:

```rust
pub struct Inbox {
    /// Ordered stream of received events since last drain.
    pub events: Vec<Event>,
}

/// A deterministic time marker attached to every event.
/// This is engine time, not wall-clock time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventTime {
    /// Monotonic engine tick counter. Increments only on EngineTick.
    pub tick: u64,

    /// Micro-step index within the same tick.
    /// 0 = main tick pass, 1.. = stabilisation rounds or flushImmediate rounds within that same tick.
    pub micro: u32,

    /// Total ordering within the same (tick, micro).
    pub seq: u32,
}

```

A node processes its inbox when it runs (typically inside `process()`), draining or acknowledging events according to the node’s chosen style:

- some event types are state-like and can be coalesced,
- others are stream-like and must be preserved in order.

The exact coalescing rules are defined later in this chapter.

## 7.3 Canonical event kinds

At the surface level, Golden Core needs a small set of event categories.

```rust
pub struct Event {
    pub time: EventTime,
    pub kind: EventKind,
}

pub enum EventKind {
    // Value-level
    ParamChanged { param: NodeId },

    // Structure-level
    ChildAdded     { parent: NodeId, child: NodeId },
    ChildRemoved   { parent: NodeId, child: NodeId },
    ChildReplaced  { parent: NodeId, old: NodeId, new: NodeId },
    ChildMoved     { child: NodeId, old_parent: NodeId, new_parent: NodeId },
    ChildReordered { parent: NodeId, child: NodeId },

    // Identity/lifecycle-level
    NodeCreated { node: NodeId },
    NodeDeleted { node: NodeId },

    // Metadata-level
    MetaChanged { node: NodeId, patch: NodeMetaPatch },
}

```

Important: these events reference live entities using `NodeId` (runtime handle). Stable identifiers (like UUID) exist in metadata and persistence, but the inbox is about the live graph.

## 7.4 Routing: who receives which events?

Not every node should receive every event. Golden Core routes events to nodes that have expressed interest.

There are two primary routing mechanisms:

### 7.4.1 Direct subscriptions (explicit interest)

A node can subscribe to:

- a specific parameter node,
- a subtree,
- or a category of events under some scope.

A subscription means: “deliver events matching this filter into my inbox”.

The mechanism is deterministic: a given event deterministically maps to a set of subscribers.

### 7.4.2 Bubbling (structural propagation)

Some events are also delivered upward through the hierarchy:

- a child changing may matter to its parent container,
- managers often need to observe changes inside their managed subtree,
- structural changes (child added/removed) naturally matter to parents.

### Structural replacement (`ChildReplaced`) and potential slots

Some edits are best modelled as **replacement** rather than “delete + create”:

- an optional slot switches concrete type (Float → Color),
- a specialised child node is swapped while keeping the same declared intent (“this is *the* value slot”).

`ChildReplaced { parent, old, new }` is emitted when the engine performs an atomic replacement under a parent.

Recommended semantics:

- The replacement is a **structural edit**.
- The engine SHOULD preserve the slot UUID across the swap when the replacement targets a potential slot (so UI bindings and references remain stable).
- Consumers that don’t care about replacement can treat it as `ChildRemoved(old)` + `ChildAdded(new)` in that order.

Bubbling is a policy-driven propagation up the parent chain. It is not “free”; it is intentional and optimised.

(Exact bubbling rules and optimisations can be deep; here we only need the concept. The main point is that managers/containers can observe what happens inside their subtree without subscribing to every leaf manually.)

## 7.5 Inbox behaviour: coalesced vs appended events

Events fall into two practical classes:

### Coalesced (state-like)

For state-like facts (e.g., “parameter value is now X”), it is often enough to keep only the latest value per tick/micro-step window. Coalescing reduces inbox size and prevents redundant work.

### Appended (stream-like)

For stream-like facts (e.g., triggers, child-added sequence, ordered structural edits), preserving the full ordered sequence matters. These must be appended and processed in order.

This is why parameter declarations include behaviour hints like “coalesce” vs “append” (especially important for triggers).

## 7.6 How inbox state drives scheduling

A node is eligible to run `process()` when the engine decides it has relevant pending work. The inbox is the primary signal for that.

This ties directly into Chapter 6’s phases:

### Normal tick

- engine enters `EngineTick`
- calls `update()` for continuous nodes
- calls `process()` for reactive nodes that are scheduled this tick (including those with non-empty inboxes)

### End-of-tick stabilisation

After applying edits from the main pass:

- if applying those edits produced events that filled other nodes’ inboxes,
- those nodes become pending work,
- the engine runs stabilisation rounds calling `process()` until inbox-driven pending work is empty or bounded.

### `flushImmediate`

Some edits request immediate propagation:

- the engine applies edits
- routes resulting events
- runs `process()` micro-steps for nodes whose inboxes became non-empty, immediately, without advancing time and without calling `update()`.

In all cases, the inbox is the deterministic “work queue” that turns model changes into node execution.

## 7.7 Ordering guarantees (what you can rely on)

Golden Core must define ordering precisely because determinism depends on it.

At the level of this chapter, the guarantees should be:

1. **Events are ordered by the engine’s application order of edits.**
    
    If edit A is applied before edit B, then the events produced by A appear before events produced by B in all inboxes that receive them.
    
2. **Within a node’s inbox, event order is preserved for appended event kinds.**
    
    Triggers and structural events preserve sequence.
    
3. **Coalesced event kinds preserve last-write-wins within the coalescing window.**
    
    If multiple value changes occur before the node processes them, the inbox may contain only the final state (by policy). This is intentional and declared.
    
4. **Stabilisation rounds preserve determinism.**
    
    The engine uses a deterministic selection and processing order for nodes that became pending due to events during the same tick.
    

These are the invariants node authors should rely on. Anything else (like exact batching boundaries between event categories) is an implementation choice and should not be assumed unless explicitly specified.

## 7.8 The author’s view inside `process()`

Inside `process(ctx)`, a node typically:

- reads the events it cares about from its inbox,
- reads any required state from the model,
- and emits edits (set parameters, create/remove/move nodes, patch metadata).

The engine applies those edits deterministically, producing more events, which may schedule more nodes (same tick via stabilisation, or immediately via `flushImmediate`, or next tick depending on policies).

This is the core deterministic loop:

**apply edits → produce events → deliver to inboxes → schedule nodes → process inboxes**.

---

# Chapter 8 — Propagation Policies (When Changes Become Visible)

Golden Core is built around a low-frequency deterministic loop (typically ~100–200 Hz). In such a system, the key UX and correctness question is not “can we change state?”, but:

**When does a change become visible to the rest of the graph?**

Propagation policies generalise this question beyond parameters. They apply to any change that can affect other nodes:

- value updates (parameters),
- metadata changes (enabled, label, tags),
- structural edits (add/remove/move/reorder),
- and even some domain payload changes (custom node data).

This chapter defines:

- the common propagation modes,
- the author-facing APIs to request them,
- and how they interact with the cycle phases from Chapter 6 (normal tick, end-of-tick stabilisation, `flushImmediate`).

We still avoid deep inbox internals; we focus on the policy surface and observable consequences.

## 8.1 The three moments a change can take effect

In Golden Core, “when a change takes effect” means: **when it is applied to the authoritative model and therefore can generate events and trigger processing.**

There are three relevant moments:

1. **Immediate (now, with a `flushImmediate` micro-step)**
    
    The change is applied right away and propagation runs immediately.
    
2. **End-of-tick (stabilised within the same tick)**
    
    The change is applied during the normal tick, and any consequences propagate via end-of-tick stabilisation micro-steps—still without pausing the loop mid-phase.
    
3. **Next tick (deferred)**
    
    The change is staged and does not affect the authoritative model until the next normal tick begins.
    

These moments are the foundation of the policy surface.

## 8.2 Propagation policy is a property of *an edit*

To generalise beyond parameters, treat propagation as a property of the **edit operation**, not of “a value”.

Every edit request (set value, patch meta, change structure) is submitted to the engine through `ProcessCtx`. Along with the edit, you specify a propagation mode:

```rust
pub enum Propagation {
    Immediate,   // apply now + flushImmediate
    EndOfTick,   // apply this tick, converge via stabilisation
    NextTick,    // stage for next EngineTick
}
```

This keeps the model consistent:

- setting a parameter and renaming a node use the same propagation vocabulary,
- structural edits can be deferred or immediate under the same rules.

(You may still define defaults per operation type or per declared field; the important point is that the mechanism is general.)

## 8.3 The author-facing API: default + forcing variants

The core API shape should follow what you already decided for parameters, but generalised:

- `set(...)` / `edit(...)` respects the default policy for that target/operation
- `_immediate(...)` forces Immediate
- `_next_tick(...)` forces NextTick

Examples (conceptual):

```rust
// Value edit (parameter)
self.intensity.set(ctx, 0.8);              // respects default propagation for that parameter
self.intensity.set_immediate(ctx, 0.8);    // forces Immediate
self.intensity.set_next_tick(ctx, 0.8);    // forces NextTick

// Meta edit (node)
ctx.meta(node).set_enabled(true);          // respects default propagation for meta
ctx.meta(node).set_enabled_immediate(true);
ctx.meta(node).set_enabled_next_tick(true);

// Structural edit (node graph)
ctx.graph(parent).create::<Foo>();         // respects default propagation for structural edits
ctx.graph(parent).create_immediate::<Foo>();
ctx.graph(parent).create_next_tick::<Foo>();
```

Even if your final API names differ, the principle is:

- **one default path**, plus **two forcing paths**.

## 8.4 Recommended defaults

Since your engine is not audio-critical and you now have end-of-tick stabilisation, a healthy default split is:

- **Most edits: EndOfTick**
    
    They apply during the tick and converge before the tick ends, without pausing mid-cycle.
    
    This gives low latency and preserves a clean main-loop structure.
    
- **Explicit “must feel instant”: Immediate**
    
    Use only when the caller truly needs synchronous propagation semantics (UI interactions that must update downstream right now, or external sync constraints).
    
- **Batch-friendly / stability-sensitive: NextTick**
    
    Use when you want to accumulate multiple edits and apply them as a stable step at the next tick boundary.
    

This avoids “everything immediate” while preventing multi-frame dependency chains.

## 8.5 How propagation interacts with the cycle phases

This section is the practical mental model that node authors rely on.

### 8.5.1 Immediate

- The engine applies the edit right away.
- Events are produced and routed.
- Reactive `process()` may run immediately as part of `flushImmediate`.
- No continuous `update()` is called.
- Engine time does not advance.

### 8.5.2 EndOfTick

- The edit is applied during the current tick.
- Consequences propagate through end-of-tick stabilisation micro-steps.
- The loop is not interrupted mid-pass.
- Continuous `update()` still runs exactly once this tick (as usual).

### 8.5.3 NextTick

- The edit is staged (recorded as pending).
- The authoritative model does not reflect it yet.
- No downstream propagation occurs until the next normal tick begins.
- Useful for batching and predictability.

## 8.6 Change emission vs propagation

Separately from “when an edit becomes visible”, there is “when it emits a change event”.

For parameters, you already have `ChangePolicy`:

- `ValueChange` vs `Always`

For general node edits, the same concept exists in different forms:

- some edits are idempotent (setting enabled to true when it’s already true),
- some are inherently eventful (a trigger, a child-added, a reorder).

The propagation policy decides **when** the edit is applied; the change policy (or operation semantics) decides **whether** it generates observable events when applied.

## 8.7 Cycle walkthroughs

### Walkthrough A — End-of-tick convergence avoids multi-tick latency

You have a dependency chain:

- Node C writes to a value that Node B reacts to
- Node B writes to a value that Node A reacts to

With EndOfTick default:

1. Tick runs, C processes and emits its edit.
2. Edit is applied, events routed.
3. Stabilisation sees B’s inbox non-empty → runs B.
4. B emits an edit; apply; events routed.
5. Stabilisation sees A’s inbox non-empty → runs A.
6. Tick ends with quiescent model.

Result: the chain converges within one tick, not three.

### Walkthrough B — Immediate is an explicit interrupt

A UI action must reflect instantly (e.g., a “panic” trigger):

1. Caller uses the forced immediate API.
2. Engine applies the edit and enters `flushImmediate`.
3. Reactive nodes process right away to converge.
4. Control returns without waiting for tick boundary.

### Walkthrough C — NextTick batches a burst

A text field edits “port” and emits many intermediate values:

1. Caller uses default NextTick policy for that field.
2. Intermediate edits are staged.
3. Next EngineTick applies the final staged value deterministically once.

---

# Chapter 9 — External vs Internal Edits, Event Ingestion, and Coalescing

Golden Core is the single source of truth, but changes can originate from different places: the UI, the network, scripts, devices, or node logic itself. The engine must ingest all of these **as edits**, turn them into **events**, and deliver them deterministically—while remaining responsive and avoiding event storms.

This chapter defines:

- what “external” vs “internal” means,
- how external inputs are ingested safely,
- how edits are coalesced (and why),
- and how this ties into the cycle phases (normal tick, end-of-tick stabilisation, `flushImmediate`).

## 9.1 Two sources of change: external and internal

### External edits

External edits originate outside the engine’s controlled node execution:

- UI interactions (sliders, typing, drag/drop),
- network replication (remote peers),
- scripting calls from outside the engine loop,
- device I/O callbacks.

External edits are typically bursty and may arrive at any time relative to the engine tick.

### Internal edits

Internal edits originate from node execution under engine control:

- `process(ctx)` emitting edits,
- `update(ctx)` emitting edits,
- `init(ctx)` / `destroy(ctx)` emitting edits.

Internal edits are already within the deterministic execution timeline.

Key principle:

- **Both external and internal changes must go through the same edit pipeline.**
- What differs is when and how they are accepted and staged.

## 9.2 The ingestion boundary: external inputs don’t mutate the model directly

Because the engine must remain deterministic, external inputs do not directly mutate the authoritative model. Instead, they are ingested into an **incoming edit queue** (or equivalent), then applied at a controlled boundary.

Conceptually:

- UI/network push “edit intents” into an engine-owned queue.
- The engine drains that queue at defined times (typically at tick boundaries, and optionally via immediate flush when requested/allowed).
- Applying edits produces events, which then drive node execution.

This keeps:

- thread safety (external threads never touch core state),
- determinism (all external edits are linearised into a single application order),
- and predictable performance (bounded draining/coalescing).

## 9.3 Propagation choice differs by origin (default behaviours)

External sources often benefit from different default propagation choices than internal node logic:

- **UI**: usually wants responsiveness, but also generates bursts (dragging a slider).
- **Network**: often wants convergence and order, but may replay or resend.
- **Internal logic**: usually wants clarity and controlled propagation within the engine cycle.

This is why Golden Core distinguishes:

- the *origin* of an edit (external vs internal),
- from the *propagation policy* of that edit (Immediate / EndOfTick / NextTick).

The engine can apply sensible defaults per origin, while still allowing explicit forcing APIs.

## 9.4 Coalescing: controlling burstiness and latency

Coalescing exists because external inputs can overwhelm a 100–200 Hz loop if treated as “every change must be processed fully”.

Coalescing has two layers:

1. **Edit coalescing at ingestion** (before applying to the model)
2. **Event coalescing at delivery** (inside inboxes)

Both are deterministic and policy-driven.

### 9.4.1 Edit coalescing (ingestion stage)

When external sources push many edits to the same target between ticks (e.g., slider drag), the engine may coalesce them before application.

Canonical behaviour:

- If multiple edits target the same parameter/value/meta field within the same ingestion window,
- keep only the latest one (last-write-wins),
- unless the edit kind is explicitly “append/stream” (e.g., triggers).

This reduces:

- redundant state transitions,
- unnecessary event generation,
- and wasted processing.

The ingestion window is typically:

- until the next tick boundary, or
- until an explicit flush boundary (immediate).

### 9.4.2 Event coalescing (inbox stage)

After edits are applied, events are generated. Some event kinds are state-like:

- “value is now X”
- “enabled is now false”

For these, inboxes may keep only the latest state per target within a coalescing window.

Other event kinds are stream-like:

- triggers,
- child-added/removed sequences,
- reorder operations.

These must remain ordered and are appended, not coalesced.

This is why declarations carry behaviour hints like “coalesce vs append” for relevant inputs.

## 9.5 Trigger and stream events: never silently collapsed

Triggers are the canonical example of a stream-like signal. Even if you represent a trigger as `Value::Trigger`, the *meaning* is “a discrete occurrence”, not “a state that is true”.

Therefore:

- triggers should be ingested as append-like events,
- delivered as ordered occurrences,
- and processed accordingly.

Similarly, structural edits are generally stream-like:

- child added then removed is not equivalent to “no change” if observers care about the sequence,
- reorder sequences matter.

So the engine must preserve ordering for these event categories.

## 9.6 Determinism and ordering with mixed origins

External and internal edits may interleave in time. Determinism requires the engine to define a single linear application order.

A practical, explainable rule set:

- External edits are drained at defined boundaries (tick start, tick end, or flush boundary).
- Internal edits emitted during node execution are applied in the engine’s deterministic order.
- When external draining happens, it is inserted deterministically into the timeline (e.g., “all queued external edits are applied before the tick’s main pass” or “after the tick’s main pass, before stabilisation”).

The exact boundary choice is an engine design decision, but it must be explicitly stated and stable.

## 9.7 How this interacts with stabilisation and `flushImmediate`

- **Stabilisation** (end-of-tick micro-steps) is the default way to converge cascading dependencies without interrupting the loop.
    - External edits applied during a tick can still converge within the same tick via stabilisation.
- **`flushImmediate`** is used when an edit requires convergence *right now*.
    - External edits may request Immediate propagation; the engine applies them and runs a flush phase.
    - Internal logic can also force immediate when necessary (rare, but supported).

Coalescing still applies:

- for Immediate flush, the ingestion window is small (often “coalesce until flush starts”),
- for EndOfTick, coalesce until tick boundary / end-of-tick application point,
- for NextTick, coalesce across the whole tick window.

## 9.8 Example scenarios

### Scenario A — UI slider drag (coalesced state-like)

- UI emits 200 value edits in 100 ms.
- Engine ingests them and coalesces to the latest value per tick window.
- On tick, apply final value, generate one value-changed event, converge via stabilisation.

Result: responsive output with bounded work.

### Scenario B — UI “panic” trigger (append-like)

- UI emits a trigger.
- Engine ingests as append-like (no collapsing).
- Apply immediately, run `flushImmediate` so downstream nodes react now.

Result: trigger is not lost and feels instant.

### Scenario C — Network resends (idempotent + coalesced)

- Peer resends “enabled = true” repeatedly.
- Engine coalesces duplicate edits at ingestion.
- If applied anyway, change emission is controlled so it doesn’t spam observers unless explicitly configured.

Result: convergence without storms.

---

# Chapter 10 — Listening to Changes (Listeners, Subscriptions, and Bubbling)

Nodes run because something they care about changed. Golden Core makes this explicit: a node expresses *interest*, the engine routes matching events into that node’s inbox, and the inbox being non-empty is what drives scheduling (`process()` in normal ticks, stabilisation micro-steps, or `flushImmediate`).

This chapter defines the complete “how a node gets interested” surface:

- direct listening (subscriptions),
- hierarchical listening (bubbling),
- and the practical patterns (managers, containers, aggregators).

It does not re-explain the engine cycle; it builds on Chapters 6–7.

## 10.1 The core model: interest → events → inbox → scheduling

A node becomes scheduled when its inbox receives events.

So “listening” is simply the set of rules that determine:

- which events are delivered to which inboxes,
- and in what form (raw vs summarised).

Golden Core supports two complementary listening mechanisms:

1. **Subscriptions**: explicit interest in specific targets or scopes
2. **Bubbling**: hierarchical propagation of selected events to ancestors

Most systems use both:

- subscriptions for precise dependencies,
- bubbling for subtree supervision (especially managers).

## 10.2 Subscriptions (explicit interest)

A subscription is an explicit rule: “deliver events matching this filter to my inbox”.

Conceptually:

```rust
pub struct ListenerSpec {
    pub subscriber: NodeId,
    pub filter: EventFilter,
    pub delivery: DeliveryMode,
}

pub enum EventFilter {
    // Narrow: one exact target
    Node(NodeId),
    Param(NodeId),

    // Structural scopes
    Subtree { root: NodeId },

    // Category filters
    Kind(EventKind),

    // Composition
    Any(Vec<EventFilter>),
    All(Vec<EventFilter>),
}

pub enum DeliveryMode {
    Raw,        // deliver full events
    Summarised, // deliver aggregated signals ("subtree dirty", etc.)
}
```

This is a mental model; the real API can be more ergonomic (macros, typed handles).

### 10.2.1 What you typically subscribe to

Common patterns:

- **A reacts to B’s parameter**
    
    Subscribe to `ParamChanged` on B’s parameter node.
    
- **A supervises a dynamic container**
    
    Subscribe to structural events under a subtree root.
    
- **A maintains an index**
    
    Subscribe to `ChildAdded/Removed/Replaced/Moved/Reordered` for a container and optionally to “descendant dirty” summaries.
    

### 10.2.2 Where subscriptions are declared

Subscriptions can come from two places:

- **Static subscriptions (schema-declared)**
    
    A node type declares “I listen to these things by default” (common for fixed wiring).
    
- **Dynamic subscriptions (runtime)**
    
    A node adds/removes listeners when the graph changes (common for managers and user-authored links).
    

Dynamic subscriptions must remain deterministic: they are created/removed through the same edit surface as other model changes (even if helpers hide the machinery).

## 10.3 Bubbling (hierarchical interest)

Bubbling is the second listening mechanism:

> When an event happens on node X, the engine may also deliver a bubbled form of that event to X’s ancestors, according to policy.
> 

This lets parent nodes observe activity in their subtree without subscribing to every descendant individually.

### 10.3.1 Bubbling is a routing rule, not a broadcast

Bubbling is:

- deterministic (the parent chain is deterministic),
- bounded by policy (it does not have to reach the root),
- and selective (only chosen event categories bubble).

### 10.3.2 What should bubble

Bubbling is most useful for:

- **Structural events** (typically bubble at least to the parent)
    - child added/removed/moved/reordered
    - node created/deleted in the subtree (optional)
- **Subtree activity summaries**
    - “some descendant parameter changed”
    - “some descendant metadata changed”
    - “subtree dirty”

For high-frequency leaf changes, bubbling raw per-leaf events to many ancestors is often too expensive. Summaries are the default scalable pattern.

### 10.3.3 Bubbling boundaries (depth control)

Bubbling must be bounded. Typical boundary policies:

- **Parent-only**: bubble one level (safe default)
- **Until boundary node**: bubble until encountering a node that declares itself a manager/supervisor boundary
- **Max depth**: bubble up to N ancestors
- **To root**: only for small graphs or explicit debugging

A practical default for real projects:

- structural events bubble to parent and to the nearest manager boundary,
- parameter/meta changes bubble as coalesced “subtree dirty” to the nearest manager boundary.

## 10.4 Raw vs summarised delivery

Both subscriptions and bubbling can deliver either:

- **Raw events** (full fidelity, ordered)
- **Summaries** (coalesced, cheaper)

Summaries are not “lossy by accident”; they are an explicit contract:

- instead of “which 50 params changed”, you receive “something changed under subtree X”.

This is essential for managers:

- they often only need to know “refresh caches / rebuild list”, not every leaf detail.

## 10.5 Ordering guarantees for listening

Listening must preserve determinism. The minimum guarantees a node author relies on:

1. **Edits define event order.**
    
    If the engine applies edit A then edit B, any delivered events respect that order.
    
2. **Raw events preserve sequence.**
    
    For append-like events (triggers, structural sequences), order is preserved in inbox delivery.
    
3. **Summaries may coalesce.**
    
    Summarised delivery can collapse multiple underlying events into one “dirty” signal per coalescing window, but it must still be deterministic about when that summary appears.
    
4. **Bubbling order is nearest-ancestor first.**
    
    For a single source event, bubbled delivery proceeds parent → grandparent → … within the same conceptual delivery step.
    

## 10.6 How managers typically use listening

Managers are the canonical consumer of listening and bubbling.

A manager usually wants:

- **precise structural listening** (raw)
    - to update folder trees, ordering, item lists incrementally
- **cheap subtree activity listening** (summarised)
    - to refresh derived views when “something inside changed” without processing every leaf event

This yields an efficient split:

- raw structural events = correctness for organisation
- dirty summaries = performance for high-frequency value churn

## 10.7 Recommended pattern: listen at boundaries, not everywhere

A scalable project typically has clear boundaries:

- a manager supervises a subtree,
- inside that subtree, nodes use direct subscriptions for their precise dependencies,
- bubbling is configured to stop at the manager boundary.

This prevents the “everything bubbles to root” trap while keeping manager-level observability.

## 10.8 Scenarios

### Scenario A — Direct dependency (subscription)

- Node A needs to react when Node B’s `gain` parameter changes.
- A subscribes directly to B’s `gain`.
- A receives `ParamChanged` events and processes them.

### Scenario B — Manager supervision (bubbling + summaries)

- A mapping manager contains folders and mapping items.
- Structural changes bubble as raw events to the manager so it can maintain its index.
- Parameter changes inside mapping items bubble as a coalesced “subtree dirty” summary so the manager can refresh derived UI once.

### Scenario C — Large subtree (bounded bubbling)

- A deeply nested graph produces many parameter updates.
- Bubbling is configured to stop at the nearest manager boundary.
- Root does not receive spam; local supervisors do.

---

# Chapter 11 — Undo/Redo, Edit Sessions, and Coalescing

Golden Core treats the node graph as the single source of truth. To support creative workflows, every mutation of that graph must be:

- deterministic,
- serialisable,
- reversible (Undo/Redo),
- and efficient under bursty input (dragging sliders, typing, network resends).

This chapter explains how the engine records edits for history, how it groups them into user-meaningful actions using explicit **begin/end edit sessions**, and how coalescing integrates with Undo/Redo.

## 11.1 History is built from graph mutations

Undo/Redo operates on the same mutation stream that drives the engine:

- value edits (parameters),
- metadata patches,
- structural edits (create/remove/move/reorder),
- and any other model mutation.

The engine records these mutations as **history entries**. Each entry must contain enough information to:

- apply the change (redo),
- and apply the inverse change (undo).

The exact internal representation is implementation-defined, but the contract is:

- **every applied edit has an undoable form**, unless explicitly marked “non-undoable”.

## 11.2 Edit sessions: `begin_edit()` / `end_edit()`

User interactions often generate bursts of edits:

- a slider drag can emit hundreds of values,
- typing a name emits per-keystroke updates,
- dragging nodes can emit repeated intermediate positions.

Undo should not step through each intermediate value. Instead, Golden Core groups many raw edits into one **user action** using explicit edit sessions.

Conceptually:

```rust
let token = engine.begin_edit(EditOrigin::UI);

// many edits happen here...

engine.end_edit(token);
```

An edit session defines a **coalescing window** and a **history grouping boundary**.

### Origins

Sessions should carry origin information (at least):

- UI
- Network
- Script
- Internal (node logic)

This allows different defaults:

- UI sessions tend to coalesce aggressively and produce a single undo step.
- Network sessions may be non-undoable locally (often the right default), or recorded separately depending on app needs.

## 11.3 What coalescing means in history

Within an edit session, the engine coalesces raw edits into a smaller set of meaningful changes before committing them to history.

There are two main coalescing patterns:

### 11.3.1 Last-write-wins coalescing (state-like targets)

For state-like properties:

- parameter values,
- enabled flag,
- label text (if edited continuously),

the session can keep only the final value per target.

Example: slider drag

- raw edits: 0.10 → 0.11 → 0.12 → … → 0.85
- history entry stores: “set value to 0.85”
- undo restores the value that existed before the session began.

### 11.3.2 Append-preserving (stream-like targets)

For stream-like operations:

- triggers,
- structural sequences where order matters (child add/remove/reorder),

coalescing must not silently discard meaning.

Typical behaviour:

- triggers are either not recorded to undo history, or recorded as discrete occurrences depending on app semantics
- structural edits are recorded as the effective final transformation, but the session must preserve correctness

(Your exact choice is a product decision; the engine should make it explicit.)

## 11.4 Session semantics for Undo/Redo

When a session begins, the engine conceptually captures a “before snapshot” for any target that will be modified in the session.

When the session ends, it commits a single history entry containing:

- the list of effective changes (after coalescing),
- and the inverse changes (using the captured “before” states).

So:

- **Redo** reapplies the session’s final effective changes.
- **Undo** restores the pre-session states.

This yields intuitive behaviour:

- one slider drag = one undo step,
- one text edit = one undo step,
- one drag-and-drop reorganisation = one undo step.

## 11.5 Nested sessions and explicit grouping

Advanced UIs may want nested grouping:

- a macro action that performs multiple UI operations,
- or a gesture that internally performs sub-steps.

Provide either:

- nested sessions (stack) with “only outermost commits to history”, or
- a single session with explicit “group markers”.

The crucial constraint: grouping must remain deterministic and serialisable.

## 11.6 Editing without an explicit session

Not every mutation is initiated by the UI. Node logic can emit edits during processing.

Default policy:

- internal edits are recorded in history only if they are within an explicit edit session started by an external origin (UI/script), or if the app explicitly enables “record internal edits”.

This prevents undo history from filling with engine-internal housekeeping.

## 11.7 Relationship with propagation (Immediate / EndOfTick / NextTick)

Propagation policy affects *when changes take effect*, not *whether they are undoable*.

Within a session:

- some edits may be Immediate (for responsiveness),
- others EndOfTick (for convergence),
- others NextTick (for batching).

History grouping should follow the session boundary, regardless of propagation timing, as long as the edits are applied as part of that session’s timeline.

## 11.8 Example: slider drag with begin/end

```rust
let token = engine.begin_edit(EditOrigin::UI);

for v in drag_values {
    // UI emits a burst; engine may coalesce per target within the session
    param.set(engine.ctx(), v);
}

engine.end_edit(token);

// Undo restores the pre-drag value.
// Redo restores the final value.
```

## 11.9 Example: move multiple nodes as one user action

```rust
let token = engine.begin_edit(EditOrigin::UI);

graph.move(node_a, folder_x);
graph.move(node_b, folder_x);
graph.reorder(folder_x, node_b, Insert::After(node_a));

engine.end_edit(token);

// Undo restores original parents + ordering.
// Redo reapplies the final structure.
```

## 11.10 Example: text edit (coalesce keystrokes)

```rust
let token = engine.begin_edit(EditOrigin::UI);

for partial in ["M", "Ma", "Map", "MapG", "MapGy", "MapGyv", "MapGyve", "MapGyver"] {
    node.meta().set_label(partial);
}

engine.end_edit(token);

// One undo step returns to the original label.
```

---


## 11.11 Potential slots: type switches are structural edits (and must be grouped)

A “typed value” pattern (where a selector parameter changes the concrete type of an optional `value` slot) must be treated as a **structural mutation**, not as a simple value write.

Recommended transaction shape:

1. `begin_edit("Switch value type")`
2. set selector parameter (`my_type := Color`)
3. materialise / replace / clear the `value` slot node accordingly
4. optional: migrate previous value if it is meaningfully convertible
5. `end_edit()`

Coalescing rules:

- Do **not** coalesce across type switches unless explicitly requested.
- A `ChildReplaced` boundary SHOULD terminate value coalescing for the affected slot, because “same key stroke / drag” semantics no longer apply.

Undo/Redo expectation:

- One Undo should restore both: the selector change and the corresponding slot state (present/absent + concrete type + value).


# Chapter 12 — Persistence and Reference Resolution

Golden Core saves and reloads a project by serialising the node graph in a way that is stable across runs and independent of runtime internals.

**Core rules**

- `NodeId` is **runtime-only** (slotmap key). It is never saved.
- `NodeUuid` is **persistent identity**. It is saved and used to restore references.
- The file mixes **Full** records (recreatable) and **Delta** records (schema patches).
    
    A record is **Full** iff it contains `type`. Otherwise it is **Delta**.
    

This chapter defines the on-disk shapes, the load pipeline, and how references (`Value::Reference`) resolve back to runtime ids.

---

## 12.1 What is stable on disk

Every node has `NodeMeta` with identifiers. For persistence, we rely on:

- `uuid`: stable identity across sessions.
- `decl_id`: stable “declared binding key” inside a parent’s declared scope (used for locating declared nodes **and** for attaching Full records to potential slots during load).
- `short_name`: human-facing name; not used for reconstruction.

`decl_id` is not explained here; it is simply the key used to match a Delta record to the correct declared node.

---

## 12.2 Two record shapes: Full vs Delta

### Full record (recreatable)

A Full record contains enough information to instantiate the node without relying on schema:

- has `type`
- has `uuid`
- has full persistable `meta` fields
- has persistable `data`
- has `children` (each Full or Delta)

Full records are used for **dynamic nodes** and anything that must be recreated from the file.

### Delta record (schema patch)

A Delta record patches a node that already exists because the schema declares it:

- **no `type`**
- has `decl_id` (to find the declared node under the current parent scope)
- may have `uuid` (to bind a stable uuid to this declared node instance)
- stores only overridden fields (e.g. parameter `value`, meta patches, etc.)

Delta records are used for **schema-declared nodes** so unchanged data does not bloat the file.

---

### 12.2.1 Potential slots: a schema-reserved `decl_id` persisted as Full

A **potential slot** is a schema-reserved child identity that may be absent at runtime, and if present may have varying concrete type. This requires one extension to the record rules:

- A **Full record** (has `type`) MAY also include `decl_id`, but **only** to attach to a potential slot declared by the parent schema.

This enables the “typed value slot” pattern:

- Slot absent → no record in file
- Slot present → Full record with both `{ decl_id, type, uuid, ... }`

#### Loader rule (attach-to-slot)

When loading a child record under parent `P`:

- Full + no `decl_id` ⇒ create a dynamic child under `P`
- Full + `decl_id` ⇒ MUST match a potential slot declared under `P`:
  - materialise or replace that slot instance (do not create a sibling)
  - bind the persisted `uuid` to that slot

#### UUID stability (recommended)

For potential slot replacements (Float ↔ Color), the engine SHOULD preserve a stable slot UUID across swaps. Persisted files can therefore keep stable UI bindings and references even when the underlying node kind changes.

## 12.3 Save policy: when something is written at all

For parameters, persistence participation is controlled by `SavePolicy`:

- `None`: never saved
- `Delta`: saved only as overrides (typically as Delta records when schema-declared)
- `Full`: saved as recreatable records (typical for dynamic content)

General rule:

- **Dynamic nodes/parameters cannot be saved as Delta** (Delta cannot recreate missing structure). If they must persist, they are saved as Full.

---

## 12.4 Reference closure: referenced nodes must be present in the file

A project file must be self-consistent: **every persisted reference must be resolvable (or deterministically dangling) after reload**.

Because schema-declared nodes are often omitted from the file when unchanged, we need one additional rule:

> **Reference closure rule:** if a node is the target of a persisted reference and would otherwise be absent from the file (because it has no overrides and is schema-declared), the saver must still emit a minimal Delta record for it that at least binds its `{ decl_id, uuid }`.
> 

This minimal Delta record is a **UUID binding record**: it does not change values; it only ensures the target has a persisted UUID identity so references can round-trip.

---

## 12.5 Project file structure (canonical JSON)

A project is stored as a **rooted hierarchy**. The on-disk `children[]` structure mirrors the runtime parent/child structure:

- **Folders are nodes.** If a parameter lives “inside” a folder, it is a child of that folder node in the file.
- A child entry is either **Full** (has `type`) or **Delta** (no `type`).

This removes inconsistencies between declared vs dynamic content and allows folder metadata (e.g. `enabled`) to persist naturally.

### 12.5.1 Full record shape (recreatable)

```json
{
  "uuid": "22222222-2222-2222-2222-222222222222",
  "type": "OscOutput",
  "meta": {
    "decl_id": "osc",
    "short_name": "osc",
    "enabled": true,
    "label": "OSC Output"
  },
  "data": {},
  "children": [ /* Full or Delta */ ]
}
```

### 12.5.2 Delta record shape (schema patch)

A Delta record patches a schema-declared child located by `decl_id` **within the current parent scope**:

```json
{
  "decl_id": "port",
  "uuid": "aaaaaaa0-0000-0000-0000-000000000002",
  "value": 9000
}
```

### 12.5.3 Potential slot instance (Full + `decl_id`)

A potential slot instance is persisted as **Full** (because the concrete type must be recreated), but also carries `decl_id` to attach to the reserved optional slot rather than creating a sibling:

```json
{
  "decl_id": "value",
  "type": "Parameter",
  "uuid": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
  "data": { "param_kind": "ColorRgba", "value": { "r": 1, "g": 0, "b": 0, "a": 1 } }
}
```

### 12.5.4 Delta record shape (UUID binding only)

This is emitted to satisfy reference closure when the node has no overrides:

```json
{
  "decl_id": "some_declared_target",
  "uuid": "99999999-9999-9999-9999-999999999999"
}
```

### 12.5.5 Example with folders, patches, and a persisted reference

In this example, `OscOutput` has a declared folder `connection` containing declared params `host` and `port`. `connection.enabled` is overridden, `host/port` are overridden, and `target` references a declared node by UUID.

Even if `some_declared_target` itself has no overrides, we still write a UUID binding record for it (reference closure).

```json
{
  "version": "3.3",
  "root": {
    "uuid": "11111111-1111-1111-1111-111111111111",
    "type": "Root",
    "meta": { "decl_id": "root", "short_name": "root", "enabled": true, "label": "Root" },
    "children": [
      {
        "uuid": "22222222-2222-2222-2222-222222222222",
        "type": "OscOutput",
        "meta": { "decl_id": "osc", "short_name": "osc", "enabled": true, "label": "OSC Output" },
        "children": [
          {
            "decl_id": "connection",
            "uuid": "aaaaaaa0-0000-0000-0000-000000000100",
            "meta": { "enabled": false },
            "children": [
              { "decl_id": "host", "uuid": "aaaaaaa0-0000-0000-0000-000000000001", "value": "192.168.0.10" },
              { "decl_id": "port", "uuid": "aaaaaaa0-0000-0000-0000-000000000002", "value": 9000 }
            ]
          },

          { "decl_id": "target", "uuid": "aaaaaaa0-0000-0000-0000-000000000010",
            "value": { "ref": "99999999-9999-9999-9999-999999999999" } },

          // UUID binding record for the referenced declared node (no overrides of its own)
          { "decl_id": "some_declared_target", "uuid": "99999999-9999-9999-9999-999999999999" }
        ]
      }
    ]
  }
}
```

Notes:

- The exact JSON encoding of `Value` is a DTO detail; persistence only requires that a reference stores a UUID payload like `{ "ref": "<uuid>" }`.
- Whether the folder itself is stored as Delta or Full follows the same rule: declared folder → Delta when only patched; dynamic folder → Full when it must be recreated.


## 12.6 Saving: producing Full/Delta plus reference closure

When saving a subtree:

1. For each node, decide record shape:
    - **Full** if the node must be recreatable from file (dynamic or SavePolicy=Full).
    - **Delta** if the node is schema-declared and only overrides must be stored.
2. For Delta records, emit only overridden fields:
    - parameter `value` if it differs from the declared default (or if SavePolicy demands)
    - selected meta overrides (label/enabled/etc.) if changed
3. Build reference closure:
    - scan persisted values for `Value::Reference { uuid }`
    - for each referenced `uuid` that points to a schema-declared node that would be absent from the file,
        
        emit a **UUID binding Delta record** (`{ decl_id, uuid }`) under the correct parent scope.
        

This guarantees references can resolve after reload without forcing the file to include unchanged nodes as Full.

---

## 12.7 Loading: schema first, then apply file records (deterministic)

Loading is **two-phase** and **scope-local**. Nothing is “searched globally”.

### Step 1 — Instantiate the declared hierarchy from schema (including folders)

The engine first constructs the **schema-declared node tree** (the “skeleton”):

- declared nodes and parameters are created
- **declared folders are created as real nodes** (so folder metadata exists as a node’s `meta`)
- macro-declared folder groupings (e.g. `#[folder]`, `#[group]`, or whatever surface you expose) are *just schema declarations* that produce these folder nodes and their declared children

After this step, every parent node has a deterministic notion of:

- **declared eager children** (must exist),
- **potential slots** (may exist, but are schema-reserved by `decl_id`).

### Step 2 — Walk the file and apply child records under each parent scope

For each parent runtime node `P`, the loader iterates `record.children[]` and handles each child record `C` **under that parent scope**.

There are **three** meaningful cases (not just “Full vs Delta”):

#### Case A — Declared override (Delta record)

Condition: `C.type` is absent and `C.decl_id` is present.

Action:

1. Resolve `C.decl_id` **only among `P`’s declared eager children**.
   - This naturally supports “declared nodes inside folders”: when `P` is a folder node, `decl_id` resolution happens *within that folder*.
2. If `C.uuid` is present: bind/rebind the child’s persistent UUID to that value.
3. Apply the override payload (`value`, `meta` patch, etc.).
4. Recurse into `C.children`.

If `decl_id` is unknown under `P`, treat it as **incompatible/migratable input**:
- recommended default: ignore and continue (forward/backward compatibility),
- strict mode: error.

#### Case B — Potential slot instance (Full record **with** `decl_id`)

Condition: `C.type` is present **and** `C.decl_id` is present.

Interpretation: this is **not** a normal dynamic child. It is a **materialisation of a schema-reserved optional slot**.

Action:

1. Verify that `P`’s schema declares a **potential slot** with `decl_id == C.decl_id`.
   - If not, this is invalid (or a migration case). **Do not** silently append a sibling.
2. Materialise or replace that slot instance to match `C.type` and data:
   - if the slot is absent: create the node instance in that slot,
   - if present with another concrete type: perform a **slot replacement** (structural edit).
3. UUID handling:
   - recommended invariant: the slot preserves a stable UUID across replacements;
   - on load, the file is authoritative: bind/rebind the slot UUID to `C.uuid` (if present).
4. Restore `meta`, restore persistable `data`, recurse into `children`.

This rule is the “pre-declaration” guarantee: if the file contains the slot, it **attaches** to the reserved identity instead of creating a new child.

#### Case C — Dynamic child (Full record without `decl_id`)

Condition: `C.type` is present and `C.decl_id` is absent.

Action:

- Create a **dynamic** child node under `P` via the runtime type registry.
- Restore `meta`, restore persistable `data`, recurse into `children`.

### Why this is deterministic

- All resolution is **parent-local**:
  - Delta records bind by `decl_id` within the current parent (which may be a folder node).
  - Potential slot instances bind by `(parent scope, decl_id)` within the current parent.
- The file never “injects” nodes into arbitrary scopes: it either patches a declared child, attaches to a reserved slot, or creates a dynamic child explicitly.

---
## 12.8 Reference persistence: only the UUID is persisted

References are values:

- runtime optimisation: `cached_id: Option<NodeId>`
- persistent identity: `uuid: NodeUuid`

On disk, only `uuid` is stored. `cached_id` is never stored.

---

## 12.9 Reference resolution after load

After the tree exists (schema + Full nodes created + Delta patches applied), the engine builds a mapping:

- `uuid -> NodeId` for the live graph.

Then each `Value::Reference { uuid, cached_id }` resolves as:

- if `uuid` exists: set `cached_id = Some(node_id)`
- otherwise: set `cached_id = None` (dangling)

Dangling references are valid and must round-trip through save/load unchanged.

This resolution can be:

- **eager** (one post-load pass over all parameter values), or
- **lazy** (resolve on first use and fill the cache),
    
    but in both cases the source of truth is the UUID.
    

---

## 12.10 Cache invalidation rules (runtime)

Because `NodeId` is runtime-only, cached ids may become invalid due to:

- node deletion,
- undo/redo resurrecting nodes,
- reload,
- large structural changes.

Rules:

1. `uuid` remains the source of truth.
2. `cached_id` is best-effort and may be cleared at any time.
3. accessors must tolerate `cached_id == None` and re-resolve via the current uuid-map.

---

## Chapter 13 — UI Access and Sync (Svelte, Tauri, Web Browser)

Golden Core is the single source of truth. The UI (whether a native desktop app via Tauri or a web page in a browser) is a **client** that:

- reads a projection of the node graph,
- sends edit intents,
- and stays in sync by consuming events.

This chapter defines the UI integration contract:

- what the UI can access,
- how communication happens (transport-agnostic),
- what data format is exchanged (DTOs, not persistence),
- how updates flow (push + pull),
- and how we keep it responsive with coalescing and stable identifiers.

This chapter is intentionally UI-facing: Svelte is the reference front-end, but the protocol is framework-agnostic.

---

## 13.1 One rule: UI never “owns” state

The UI does not store authoritative state. It may cache:

- view models (derived, denormalised),
- a local snapshot of the graph for rendering,
- pending user edits.

But the only authoritative truth is the engine model.

Therefore:

- the UI sends **edit intents**,
- the engine applies them deterministically,
- the engine emits **events** (with `EventTime`),
- the UI updates its cached snapshot accordingly.

---

## 13.2 Targets: desktop (Tauri) and web browser

### Desktop (Svelte + Tauri)

- Svelte renders the UI.
- Tauri provides the bridge between the webview and the Rust engine.
- Communication is typically request/response for commands plus a push stream for events.

### Web browser (remote UI)

- Same UI code can run in a browser.
- Communication is over a network transport (WebSocket is the default assumption).
- Multiple clients may connect concurrently; the engine remains authoritative.

The protocol described below must work identically in both environments.

---

## 13.3 Transport-agnostic contract

The UI sync is defined as a set of messages. The transport can be:

- Tauri invoke + event emit,
- WebSocket,
- any other bidirectional channel.

What matters is that the channel supports:

1. **request/response** (send edit intent, receive ack),
2. **server push** (events), and
3. **ordering** (per connection, FIFO).

---

## 13.4 DTOs: UI sync format is *not* persistence format

The project file format (Full/Delta) is optimised for saving/loading and stability across versions.

The UI format is optimised for:

- introspection (the UI needs “everything” for editors),
- incremental updates,
- minimal payload for frequent changes.

So we define a separate DTO model for UI sync.

### 13.4.1 Node DTO (UI-facing)

A node DTO should include:

- runtime identity for this session: `node_id`
- stable identity: `uuid`
- `meta` (at least enabled/label/short_name + optional additional fields)
- a summary of `data` (parameter value, container capabilities, node type)
- children list (as `node_id`s)

Example (shape):

```json
{
  "node_id": 42,
  "uuid": "22222222-2222-2222-2222-222222222222",
  "type": "OscOutput",
  "meta": {
    "short_name": "osc",
    "label": "OSC Output",
    "enabled": true
  },
  "data": {
    "kind": "Container",
    "allowed_types": ["MappingItem", "Folder"],
    "folders": "Allowed"
  },
  "children": [43, 44, 45]
}
```

Notes:

- `node_id` exists only for UI session efficiency; it never goes to persistence.
- `uuid` remains the stable cross-session identifier and is required for references and durable UI bindings.

### 13.4.2 Parameter DTO

A parameter DTO should include:

- current value,
- constraints/edit rules required by the UI,
- read-only flag,
- update policy (Immediate / NextTick),
- change policy (ValueChange / Always),
- and the presentation/semantics hints needed to choose widgets.

(These fields exist in the core model; the UI simply needs them in one place.)

---

## 13.5 Initial sync: snapshot + schema view

When a client connects, it needs:

1. a **graph snapshot** of the relevant subtree (often the whole project),
2. the **type/schema information** needed to render editors:
    - value types,
    - enum definitions (options),
    - container palettes (allowed types),
    - presentation hints.

This can be done as:

- one big snapshot message, or
- paged/streamed snapshots (recommended for large graphs).

A minimal initial sync sequence:

1. `Hello` (client identifies protocol version, desired subtree)
2. `Snapshot` (nodes + edges + required schema fragments)
3. `Subscribe` (client asks for ongoing events starting from a given `EventTime`)

---

## 13.6 Live sync: events drive incremental updates

After the snapshot, the UI stays up-to-date by consuming the engine’s event stream.

### 13.6.1 Event delivery to UI

Events sent to UI must include:

- `EventTime` (tick/micro/seq),
- event kind payload,
- node ids (and optionally uuid if needed for robustness).

Example:

```json
{
  "time": { "tick": 120, "micro": 1, "seq": 7 },
  "kind": "ParamChanged",
  "param": 44
}
```

The UI uses this to:

- update the cached node DTO/parameter value,
- re-render,
- and maintain ordering correctness.

### 13.6.2 UI subscription scopes (performance)

Clients should subscribe to scopes:

- whole graph,
- a manager subtree,
- a panel-relevant subtree.

This avoids pushing unrelated events to lightweight clients.

---

## 13.7 Edit intents from UI: begin/end, coalescing, and acks

The UI sends edits as intents. The engine replies with acknowledgements. For responsive UX and clean Undo, the UI should bracket interactions with begin/end boundaries.

### 13.7.1 Begin/end edit session (Undo boundary)

For UI-driven interactions:

- `BeginEdit { origin: UI, label?: "Slider drag", client_edit_id }`
- multiple edit intents (set value, move node, rename, etc.)
- `EndEdit { client_edit_id }`

This corresponds to the Undo/Redo grouping model (Chapter 11).

### 13.7.2 Coalescing from UI

The UI is allowed to send frequent intermediate edits (e.g. slider drag), but both sides should coalesce:

- UI coalescing: optional (reduce bandwidth)
- engine coalescing: mandatory (reduce workload and history noise)

The UI should never assume every intermediate value becomes visible; it should treat the engine as the arbiter and render the latest confirmed state from events.

### 13.7.3 Ack semantics

Every UI edit intent should receive an ack:

- accepted + applied (possibly deferred),
- rejected (validation failed),
- or accepted but staged (NextTick).

A minimal ack should include:

- success boolean,
- optional error code/message,
- and optionally the earliest `EventTime` at which the UI can expect resulting events.

---

## 13.8 Multi-client considerations (browser + desktop + remote)

When multiple UIs connect:

- the engine remains authoritative,
- all clients receive the same event stream (within their subscription scopes),
- and external edits from one client are just “external edits” to the engine.

Two important rules:

1. **No client-side authority**: clients must reconcile with server events.
2. **Deterministic ordering**: the engine serialises concurrent external edits into a single applied order (the UI observes that order via `EventTime`).

If the UI wants optimistic rendering, it must still reconcile when server events arrive.

---

## 13.9 References and enums in UI

### References

UI editors for references should operate on:

- displayed labels/paths for browsing,
- but store references by **uuid** (stable identity),
- optionally backed by `node_id` for fast local operations during a session.

### Enums

Enums must be delivered to the UI as definitions:

- `enum_id` + list of `(variant_id, label, tags, ordering)`
    
    so the UI can render dropdowns reliably and independently of language/labels stored in the value.
    

---

## 13.10 Practical Svelte architecture (recommended)

In Svelte, the simplest stable approach is:

- a single store holding the local graph cache:
    - `nodesById: Map<NodeId, NodeDto>`
    - `childrenById: Map<NodeId, NodeId[]>`
    - `paramsById: Map<NodeId, ParamDto>` (or inlined into node DTO)
- an event reducer that applies incoming events deterministically:
    - update value / meta / structure in the cache
- view components subscribe to stores and render.

The UI remains dumb:

- it dispatches edit intents,
- it updates visuals based on the engine’s events,
- it does not invent state transitions.

---

# Chapter 14 — UI DTO Specification (Messages, Snapshots, Subscriptions, and Patches)

This chapter specifies the **UI sync protocol** between Golden Core (engine) and a UI client (Svelte in Tauri or a web browser). It is transport-agnostic: the same message shapes apply whether you move them via Tauri IPC or WebSocket.

The protocol is built around four ideas:

1. **Snapshot**: send enough state to render.
2. **Events**: push incremental changes with deterministic ordering (`EventTime`).
3. **Edit intents**: UI requests mutations; engine acknowledges.
4. **Schema fragments**: the UI receives the definitions it needs to build editors (enums, node-type palette, constraints).

---

## 14.1 Versioning and compatibility

Every connection begins by agreeing on:

- `protocol_version` (string or semver),
- feature flags (optional),
- and an optional `root_scope` (subtree the client cares about).

Rule:

- If protocol versions mismatch incompatibly, the engine rejects the connection with a structured error.

---

## 14.2 Core identifiers

UI DTOs use both:

- `node_id` (runtime-only, fast for this session)
- `uuid` (stable identity across sessions, used for references and durable UI binding)

`node_id` is never persisted. `uuid` is.

---

### `decl_id` in DTOs

UI DTOs carry both `uuid` and `decl_id` when available:

- `uuid` is the stable identity.
- `decl_id` is the schema binding key **within the current parent scope**.

A node instance may be:

- declared (Delta-patchable) → has `decl_id`
- dynamic → no `decl_id`
- **potential slot instance** → has `decl_id` but may also be a Full record (because its concrete type varies)

This mirrors the persistence rule: Full records may include `decl_id` **only** to attach to potential slots.

## 14.3 Core data structures

### 14.3.1 EventTime DTO

```json
{ "tick": 120, "micro": 1, "seq": 7 }
```

- `tick`: increments only on normal `EngineTick`
- `micro`: micro-step index within the tick (0 = main tick pass; 1.. = stabilisation / flush rounds)
- `seq`: total ordering within the same `(tick, micro)`

### 14.3.2 Node DTO

Minimal node snapshot payload:

```json
{
  "node_id": 42,
  "uuid": "22222222-2222-2222-2222-222222222222",
  "type": "OscOutput",
  "meta": {
    "short_name": "osc",
    "label": "OSC Output",
    "enabled": true,
    "tags": ["output"]
  },
  "data": { "kind": "Container" },
  "children": [43, 44, 45]
}
```

Notes:

- `type` is always present in UI DTOs (unlike persistence).
- `children` is ordered.

### 14.3.3 Parameter DTO

Parameters are nodes, but the UI needs extra “editor” fields in one place:

```json
{
  "param_node_id": 44,
  "value": 9000,
  "value_type": "Int",
  "read_only": false,
  "update_policy": "NextTick",
  "change_policy": "ValueChange",
  "constraints": { "min": 1, "max": 65535, "step": 1, "clamp": true },
  "presentation": { "widget": "Number" },
  "semantics": { "intent": "port" }
}
```

If you prefer not to duplicate, you can inline `param` data into `NodeDto.data` when `kind == "Parameter"`.

### 14.3.4 Enum definitions (schema fragment)

```json
{
  "enum_id": "blend_mode",
  "variants": [
    { "variant_id": "normal", "label": "Normal" },
    { "variant_id": "add", "label": "Add" },
    { "variant_id": "multiply", "label": "Multiply" }
  ]
}
```

---

## 14.4 Message envelope

All messages should share a minimal envelope:

```json
{
  "msg": "Snapshot",
  "req_id": "c-00123",
  "payload": { }
}
```

- `req_id` is required for request/response messages, optional for pushed messages.
- For pushed messages, omit `req_id` or set it null.

---

## 14.5 Connection handshake

### 14.5.1 Client → Engine: Hello

```json
{
  "msg": "Hello",
  "req_id": "c-00001",
  "payload": {
    "protocol_version": "1.0",
    "client": { "name": "GoldenUI", "version": "0.1.0" },
    "root_scope": { "mode": "Subtree", "root_uuid": "11111111-1111-1111-1111-111111111111" }
  }
}
```

### 14.5.2 Engine → Client: HelloAck

```json
{
  "msg": "HelloAck",
  "req_id": "c-00001",
  "payload": {
    "protocol_version": "1.0",
    "server": { "name": "GoldenCore", "version": "3.3" },
    "features": ["subscriptions", "edit_sessions", "schema_fragments"]
  }
}
```

---

## 14.6 Snapshot and schema delivery

### 14.6.1 Client → Engine: GetSnapshot

```json
{
  "msg": "GetSnapshot",
  "req_id": "c-00002",
  "payload": {
    "scope": { "mode": "Subtree", "root_uuid": "11111111-1111-1111-1111-111111111111" },
    "include_schema": true
  }
}
```

### 14.6.2 Engine → Client: Snapshot

```json
{
  "msg": "Snapshot",
  "req_id": "c-00002",
  "payload": {
    "as_of": { "tick": 120, "micro": 0, "seq": 0 },
    "nodes": [ /* NodeDto[] */ ],
    "params": [ /* ParamDto[] (optional if inlined) */ ],
    "schema": {
      "enums": [ /* EnumDef[] */ ],
      "node_types": [
        {
          "type": "OscOutput",
          "label": "OSC Output",
          "palette_allowed_children": ["Folder", "MappingItem"]
        }
      ]
    }
  }
}
```

`as_of` is the baseline time for incremental event subscription.

---

## 14.7 Subscriptions and event streaming

### 14.7.1 Client → Engine: Subscribe

```json
{
  "msg": "Subscribe",
  "req_id": "c-00003",
  "payload": {
    "scope": { "mode": "Subtree", "root_uuid": "11111111-1111-1111-1111-111111111111" },
    "from": { "tick": 120, "micro": 0, "seq": 0 }
  }
}
```

### 14.7.2 Engine → Client: EventBatch (push)

Events are pushed in batches for efficiency:

```json
{
  "msg": "EventBatch",
  "payload": {
    "events": [
      { "time": { "tick": 120, "micro": 0, "seq": 1 }, "kind": "ParamChanged", "param": 44 },
      { "time": { "tick": 120, "micro": 0, "seq": 2 }, "kind": "MetaChanged", "node": 42, "patch": { "label": "OSC Out" } }
    ]
  }
}
```

Rule:

- batches are strictly ordered; the UI applies them in-order.
- `EventTime` is the authoritative ordering key.

---

## 14.8 Patch DTOs carried by events

UI needs enough payload to update cache without re-fetching everything.

### 14.8.1 ParamChanged payload

Two options:

**Option A (lightweight):**

- event only says “param changed”; UI reads latest value from a local cache updated by a separate `ParamValue` event or via a query.

**Option B (recommended): include new value**

```json
{ "kind": "ParamChanged", "param": 44, "value": 9100 }
```

Same for meta changes: include a patch:

```json
{ "kind": "MetaChanged", "node": 42, "patch": { "enabled": false } }
```

Structural events should carry enough info to update `children` arrays deterministically:

```json
{ "kind": "ChildAdded", "parent": 42, "child": 99, "index": 3 }
{ "kind": "ChildReplaced", "parent": 42, "old": 99, "new": 120, "index": 3 }
```

---

## 14.9 Edit intents (UI → engine)

### 14.9.1 Begin / End edit session

Used for Undo grouping and coalescing windows.

```json
{ "msg": "BeginEdit", "req_id": "c-01000", "payload": { "origin": "UI", "label": "Slider drag" } }
```

Reply:

```json
{ "msg": "BeginEditAck", "req_id": "c-01000", "payload": { "edit_session_id": "s-77" } }
```

End:

```json
{ "msg": "EndEdit", "req_id": "c-01001", "payload": { "edit_session_id": "s-77" } }
```

### 14.9.2 Set parameter

```json
{
  "msg": "SetParam",
  "req_id": "c-01002",
  "payload": {
    "edit_session_id": "s-77",
    "param_node_id": 44,
    "value": 9100,
    "propagation": "EndOfTick"
  }
}
```

Ack:

```json
{
  "msg": "Ack",
  "req_id": "c-01002",
  "payload": { "ok": true }
}
```

### 14.9.3 Patch meta

```json
{
  "msg": "PatchMeta",
  "req_id": "c-01003",
  "payload": {
    "edit_session_id": "s-77",
    "node_id": 42,
    "patch": { "label": "OSC Output A", "enabled": true },
    "propagation": "NextTick"
  }
}
```

### 14.9.4 Structural edits (create/move/delete)

Create:

```json
{
  "msg": "CreateNode",
  "req_id": "c-01004",
  "payload": {
    "edit_session_id": "s-77",
    "parent_id": 42,
    "type": "Folder",
    "meta": { "label": "New Folder" },
    "propagation": "EndOfTick"
  }
}
```

Move/reorder should use an explicit index:

```json
{
  "msg": "MoveNode",
  "req_id": "c-01005",
  "payload": {
    "edit_session_id": "s-77",
    "node_id": 99,
    "new_parent_id": 42,
    "new_index": 3,
    "propagation": "EndOfTick"
  }
}
```

Delete:

```json
{
  "msg": "DeleteNode",
  "req_id": "c-01006",
  "payload": {
    "edit_session_id": "s-77",
    "node_id": 99,
    "propagation": "EndOfTick"
  }
}
```

---

## 14.10 Error model

All request/response messages can reply with:

```json
{
  "msg": "Ack",
  "req_id": "c-01006",
  "payload": {
    "ok": false,
    "error": { "code": "ValidationFailed", "message": "Port must be <= 65535" }
  }
}
```

---

## 14.11 Re-sync and recovery

Clients can lose events (disconnect) or join late. Provide:

- `GetSnapshot` (again) for full resync
- `Subscribe { from: EventTime }` for incremental replay (if the engine keeps a bounded event log)

If replay is not supported, the server returns a structured error and the client performs a full snapshot.

---

# Chapter 15 — UI Client Patterns (Svelte Stores, Reducers, Reconnect, Optimistic UX)

This chapter describes a practical Svelte client architecture for Golden Core sync, usable in:

- **Tauri** (IPC + event stream), and
- **Web browsers** (WebSocket).

The goal is correctness first:

- the engine is authoritative,
- the UI applies ordered `EventBatch` updates,
- and local interactions remain responsive via controlled optimism and reconciliation.

---

## 15.1 Recommended architecture: “snapshot + reducer”

Use a single source of truth *inside the UI* for what it currently believes the engine state is:

1. **Snapshot loader** builds the initial cache.
2. **Event reducer** applies `EventBatch` messages in order.
3. UI components render from Svelte stores derived from that cache.
4. Edit intents are dispatched via a thin command layer.

The UI must not scatter state mutations across components.

---

## 15.2 Core stores (minimal, scalable)

Use a small set of stores; keep them normalised.

### 15.2.1 Graph store

- `nodesById: Map<NodeId, NodeDto>`
- `childrenById: Map<NodeId, NodeId[]>` (optional if included in NodeDto)
- `paramsById: Map<NodeId, ParamDto>` (if not inlined)

### 15.2.2 Schema store

- `nodeTypesByName`
- `enumsById`

### 15.2.3 Sync store

- `connected: boolean`
- `subscribedScope`
- `lastAppliedEventTime: EventTime`
- optional: `pendingRequests: Map<req_id, Pending>`

### 15.2.4 UI-only store

- selection state,
- expanded folders,
- panel layout,
- inspector focus.

These must never be sent back as model edits unless explicitly desired.

---

## 15.3 The event reducer (authoritative updates)

The reducer takes an `EventBatch` and mutates the cache.

Key rule:

- **Apply events strictly in stream order**, and track `lastAppliedEventTime`.

### 15.3.1 Ordering check

Maintain:

- `lastAppliedEventTime`

For each incoming event:

- if `event.time` is **<=** lastApplied, ignore (duplicate/replay)
- else apply and advance lastApplied.

(You can enforce strict monotonicity and treat out-of-order as a protocol error.)

### 15.3.2 Patch application patterns

Recommended: events carry enough payload to update without extra queries.

Examples:

- `ParamChanged { param, value }` → update `paramsById[param].value = value`
- `MetaChanged { node, patch }` → shallow merge into `nodesById[node].meta`
- `ChildAdded { parent, child, index }` → insert into `childrenById[parent]` and ensure `nodesById` contains the child (either already present, or delivered by a companion “NodeCreated/### Potential slots in the UI model

UI clients should be able to distinguish:

- “this child node does not exist” (slot absent),
- from “a node exists but is empty/default”.

For schema-reserved optional slots, represent them as an **optional child slot** keyed by `decl_id`, whose state is:

- `absent`, or
- `present` with a concrete node snapshot (including `type`, `uuid`, data/meta)

This prevents UI flicker during type switches and allows stable binding to a slot even when its concrete node kind changes.

NodeSnapshot” event)
- `NodeDeleted { node }` → remove from maps + remove from any parent’s children list

If you choose “lightweight events” (no new values), you must provide a query path (`GetNode`, `GetParamValue`). That is more complex and generally worse UX.

---

## 15.4 Edit dispatch layer (UI → engine)

Create a single module that exposes UI actions like:

- `setParam(nodeId, value, propagation)`
- `patchMeta(nodeId, patch, propagation)`
- `createNode(parentId, type, meta, propagation)`
- `moveNode(nodeId, newParentId, newIndex, propagation)`
- `deleteNode(nodeId, propagation)`

This module:

- optionally wraps edits in `BeginEdit`/`EndEdit`,
- assigns `req_id`,
- manages acks,
- and coordinates optimistic rendering.

---

## 15.5 Optimistic UI (recommended) with reconciliation

Optimism means:

- UI updates immediately when user interacts,
- but the engine remains authoritative,
- and the UI reconciles when events arrive.

### 15.5.1 How to do optimism safely

Maintain an overlay of local pending changes keyed by target:

- `pendingParam: Map<paramId, value>`
- `pendingMeta: Map<nodeId, patch>`
- `pendingStructure: …` (optional; structural optimism is harder)

Rendering reads:

- `pending overlay` if present,
- else authoritative cache.

When an event arrives from the engine:

- apply it to the authoritative cache,
- then clear matching pending entries that are now confirmed (or adjust if the engine disagreed).

### 15.5.2 Why you should be conservative with structural optimism

Optimistically reordering/moving nodes can conflict with:

- validation rejection,
- concurrent edits from other clients,
- auto-normalisation rules.

Recommendation:

- optimistic for **parameter values** and **simple meta**,
- “semi-optimistic” for structure (show a loading state or ghost placeholder until ack/event).

---

## 15.6 Edit sessions in UI (Undo grouping)

For user gestures:

- start a session on pointer-down / focus start
- end on pointer-up / blur / enter-key

Examples:

- slider drag: begin on `pointerdown`, end on `pointerup`
- text edit: begin on focus, end on blur/enter
- drag/drop move: begin at drag start, end at drop

This ensures one gesture = one undo step.

---

## 15.7 Coalescing strategy: UI and engine roles

- **UI coalescing** reduces bandwidth and IPC overhead:
    - throttle slider updates (e.g. requestAnimationFrame or 30–60 Hz cap)
    - debounce text input (e.g. 150–300 ms) unless you need per-keystroke previews
- **Engine coalescing** is mandatory:
    - engine may accept bursts but only commits meaningful changes
    - UI must not assume every intermediate value is applied

Best practice:

- make UI feel smooth locally via optimistic overlay,
- let engine coalescing dictate what becomes authoritative.

---

## 15.8 Reconnect and resync

### 15.8.1 Fast path: replay from EventTime

On reconnect:

1. send `Hello`
2. send `Subscribe { from: lastAppliedEventTime }`

If server can replay, client catches up.

### 15.8.2 Slow path: full snapshot

If server replies “cannot replay that far”:

1. `GetSnapshot`
2. replace authoritative cache completely
3. clear pending overlays (or reapply carefully if you have a robust pending queue)

Recommendation:

- clear pending overlays on full resync unless you explicitly implement “resubmit pending edits”.

---

## 15.9 Multi-client consistency rules

Assume:

- other clients can edit the same targets concurrently.

Therefore:

- always treat server events as truth,
- if an optimistic overlay conflicts with incoming events:
    - either snap to server state,
    - or show a conflict indicator (advanced).

A simple and acceptable rule:

- “last server event wins; pending overlay clears on mismatch”.

---

## 15.10 Tauri vs Web transport notes

### Tauri

- Use `invoke` for request/response.
- Use `listen` for pushed `EventBatch`.

Important: Tauri event delivery is per-window; ensure your UI layer handles duplicate listeners.

### Web (WebSocket)

- Use a single socket per tab.
- Implement heartbeat/ping to detect disconnect quickly.
- Queue outbound requests until `HelloAck` completes.

---

## Chapter 16 — Codebase Organization (Workspace, Crates, and Module Layout)

Golden Core’s guarantees (determinism, portability, stable persistence, UI sync) are easiest to keep if the codebase enforces them structurally. The repository should be a **Rust workspace** with small crates and strict dependency direction, so UI or OS glue can’t “leak” into the engine by accident.

This chapter defines the recommended workspace layout, crate responsibilities, dependency rules, and the internal module organisation that matches the concepts introduced in this document.

---

## 16.1 Workspace layout (high level)

Golden Core should be organised as:

- one minimal crate that defines the shared types, persistence DTOs, **and UI protocol DTOs** used everywhere,
- one headless engine crate that contains the deterministic runtime,
- optional feature crates (standard nodes, networking/transport),
- one top-level app crate (Tauri / OS integration).

Recommended workspace tree:

```
/Cargo.toml               # workspace
/crates
  /golden_schema          # shared types + persistence DTOs + UI protocol DTOs
  /golden_core            # engine runtime (headless, deterministic)
  /golden_macros          # proc-macros: #[derive(GoldenNode)], #[param], ...
  /golden_std             # standard node library (optional but recommended)
  /golden_net             # network transport + client sessions (optional)
  /golden_app             # Tauri app / OS integration (top)
```

**Change from previous structure:** `golden_ui_protocol` is merged into `golden_schema`.

There is now **one authoritative DTO layer** for both persistence and UI sync.

---

## 16.2 Crate responsibilities (what goes where)

### 16.2.1 `golden_schema` — shared language (no runtime)

Defines data types shared across subsystems:

- identifiers: `NodeId`, `NodeUuid`, `DeclId`, `NodeTypeId`
- metadata DTOs: `NodeMetaPatch`, semantics/presentation hints
- value domain DTOs: `Value` (incl. `ReferenceValue { uuid, cached_id }`)
- event DTOs: `EventKind`, `EventTime`, UI-facing event payload shapes
- persistence DTOs: Full/Delta node records, save/load DTO types
- **UI protocol DTOs**:
    - message types: `Hello`, `Snapshot`, `Subscribe`, `EventBatch`, `SetParam`, `PatchMeta`, `CreateNode`, etc.
    - UI projections: `NodeDto`, `ParamDto`, `EnumDef`, `NodeTypeDef`
    - codecs/helpers (optional): serde-friendly enums/structs, validation helpers that stay runtime-free

Hard constraints:

- no engine loop
- no OS glue (no `tauri`, no filesystem)
- no socket servers
- no node execution traits
- only foundational deps (`serde`, `smallvec`, `uuid`, etc.)

This crate is the “common language” layer. By merging UI protocol here, you ensure:

- one place to version DTOs,
- shared reuse of `EventTime` and identifiers,
- fewer cross-crate refactors when protocol evolves.

### 16.2.2 `golden_core` — deterministic engine runtime

Implements the headless engine:

- node storage (`NodeId` slotmap)
- intrusive hierarchy links (parent/child/siblings)
- event production + routing + inbox storage (incl. coalescing)
- execution surface (`ProcessCtx`, lifecycle hooks, scheduling)
- propagation handling (normal tick, end-of-tick stabilisation, `flushImmediate`)
- listener system (subscriptions + bubbling)
- persistence load/save logic (using DTOs from `golden_schema`)
- reference resolution (`uuid -> NodeId` cache filling)
- history (Undo/Redo + edit sessions) as engine services

Hard constraints:

- no UI framework deps
- no Tauri deps
- no network socket deps
- should remain portable (ideally WASM-friendly)

### 16.2.3 `golden_macros` — proc-macros / derive

Defines `#[derive(GoldenNode)]` and associated attributes used for declarations.

Responsibilities:

- parse struct fields annotated with `#[param]`, `#[child]`, `#[folder]`, `#[container]`
- generate:
    - schema descriptors (declared children/params/folders)
    - handle bindings (typed parameter handles, child list handles)
    - optional registration metadata for the node type

Hard constraint:

- proc-macro crate only; it depends on `syn`, `quote`, and **schema types**, not on the engine runtime.

### 16.2.4 `golden_std` — standard nodes (optional)

A library of built-in node types (OSC, MIDI, mapping primitives, etc.) built on the authoring surface.

Depends on:

- `golden_core` (for engine traits/handles)
- `golden_macros` (for derive/attributes)
- `golden_schema` (for shared ids/hints)

Should not contain OS glue.

### 16.2.5 `golden_net` — networking / transport (optional)

Owns:

- WebSocket server/client sessions (or other transports),
- mapping UI protocol messages to engine edit intents,
- subscription management per client scope,
- authentication/permissions (if any).

Depends on:

- `golden_schema` (UI protocol types, DTOs)
- `golden_core` (engine API: apply edits, subscribe to events, snapshot export)

Should not depend on `tauri`.

### 16.2.6 `golden_app` — Tauri + OS integration (top crate)

Owns:

- Tauri window/webview setup,
- filesystem access (open/save),
- OS-level device enumeration and permissions,
- wiring: engine + net + UI bridge.

Depends on:

- `golden_core`, `golden_schema`, `golden_net` (if used), and `tauri`.

---

## 16.3 Dependency direction (non-negotiable)

Keep dependencies unidirectional:

- `golden_schema` → (nothing in workspace)
- `golden_macros` → `golden_schema`
- `golden_core` → `golden_schema`
- `golden_std` → `golden_core` + `golden_macros` + `golden_schema`
- `golden_net` → `golden_core` + `golden_schema`
- `golden_app` → everything above

This prevents accidental coupling like “engine depends on OS glue” or “schema pulls in engine scheduling”.

---

## 16.4 Internal module layout (inside `golden_core`)

Organise by the concepts from the doc:

```
crates/golden_core/src
  lib.rs

  graph/
    node.rs
    hierarchy.rs
    queries.rs

  meta/
    mod.rs

  data/
    mod.rs
    container.rs
    parameter.rs
    custom.rs

  values/
    mod.rs
    reference.rs

  events/
    mod.rs          # EventTime + Event envelope + EventKind re-exports
    inbox.rs
    routing/
      subscriptions.rs
      bubbling.rs

  engine/
    mod.rs
    loop.rs         # tick / stabilisation / flushImmediate
    scheduling.rs
    process_ctx.rs

  edits/
    mod.rs
    apply.rs
    coalesce.rs

  history/
    mod.rs
    sessions.rs

  persistence/
    mod.rs
    save.rs
    load.rs
    migrate.rs       # optional
```

---

## 16.5 Internal module layout (inside `golden_schema`)

```
crates/golden_schema/src
  lib.rs

  ids.rs            # NodeId / NodeUuid / DeclId / NodeTypeId
  meta.rs           # NodeMetaPatch + hint DTOs
  values.rs         # Value DTO (incl. ReferenceValue)
  events.rs         # EventTime + EventKind (+ UI event payload DTOs if needed)

  persistence/
    mod.rs          # Full/Delta record DTOs
    file_format.rs  # root structure, versioning, etc.

  ui/
    mod.rs
    dtos.rs         # NodeDto / ParamDto / EnumDef / NodeTypeDef
    messages.rs     # Hello/Snapshot/Subscribe/EventBatch/SetParam/...
    codecs.rs       # optional: serde helpers, compact encodings
```

Guideline:

- keep `ui/` strictly DTO + protocol: no engine hooks, no transport code.
- transport-specific framing belongs in `golden_net` (websocket) or `golden_app` (Tauri).

---

## 16.6 “Class” organisation (Rust types and ownership boundaries)

Keep ownership crisp:

- `Engine` owns:
    - graph storage,
    - listener tables,
    - event queues/log,
    - history manager,
    - persistence services.
- `ProcessCtx` is ephemeral, created by the engine per pass and exposing only:
    - read views,
    - inbox view,
    - edit emission view.
- `NodeBehaviour` implementations never hold references to engine internals.
    
    They operate only through handles + `ProcessCtx`.
    

This ensures the engine remains testable and deterministic, and it prevents UI/network integration from mutating core state outside the edit pipeline.

---