# Golden / Chataigne2 Final Clean-Sheet Architecture Plan

## Executive decision

This plan replaces the current repository layout and the current runtime center. It deliberately provides no compatibility layer for the existing internal APIs, crate boundaries, Git submodules, runtime protocol, or project schema. The software is still pre-release and has no external users, so preserving flawed boundaries would be more expensive than replacing them.

The requirement is functional preservation, not architectural preservation. Every useful capability already present in Chataigne2 must exist in the final system, but it may be reimplemented behind new models, protocols, files, APIs, execution paths, and UI stores.

The final system is considered the best attainable foundation when:

- `golden-graph` is the sole owner of reusable graph concepts and graph editing infrastructure;
- Alchemist is strictly the formula domain built on an Alchemist-specific `golden-graph` domain;
- statecharts are a separate domain built on `golden-graph`, with no dependency on Alchemist;
- the editable project/graph model is never the steady-state runtime representation;
- project changes compile incrementally into immutable runtime generations;
- control, semantic execution, IO, effects, observation, and UI have explicit boundaries and queue semantics;
- steady-state value processing performs no tree snapshot, graph traversal, protocol serialization, or allocation proportional to project size;
- the UI receives bounded keyed deltas for visible data and remains responsive independently of semantic load;
- all former Git submodules are replaced by one coherent monorepo workspace;
- old implementations are removed after parity is proven rather than retained as compatibility paths.

## 1. Canonical terminology

The architecture must use the following terms consistently.

### Project document

The persistent authoring model of a Golden application. It contains hierarchical entities, parameters, graph documents, statecharts, formulas, module configuration, dashboards, and presentation metadata. It exists for editing, undo/redo, persistence, inspection, and compilation.

### Graph document

A reusable authored directed graph owned by `golden-graph`. It contains stable graph/node/port/edge identifiers, node payloads supplied by a graph domain, topology, graph-level metadata, comments/groups, and persisted layout. A graph document is not an executable runtime.

### Graph domain

The typed adapter that gives a generic graph meaning. A domain defines node payloads, port schemas, edge payloads, connection rules, validation, palette/catalog entries, inspectors, rendering metadata, and optional compilation entry points.

Examples:

- `AlchemistGraphDomain`: ANodes, typed formula sockets, formula-specific validation;
- `StatechartGraphDomain`: states, regions, transitions, guards, statechart-specific validation;
- future domains such as routing, cues, timelines, or shader graphs.

### Alchemist formula

A formula is a recipe authored as a `GraphDocument<AlchemistGraphDomain>` plus formula-specific properties, surface declarations, managed regions, metadata, and defaults. Alchemist does not own generic graph nodes, edges, groups, comments, layout, transactions, selection, or canvas behavior.

### Processor

A runtime-capable instance of an Alchemist formula. It owns property overrides, context bindings, managed-region instances, lifecycle configuration, condition bindings, and lane state. Identical processors share one compiled formula kernel.

### Context and multiplex

A context is inherited and accumulated authoring/runtime data. A context with multiple items along one or more axes is multiplexed. Multiplex is a dense lane layout and indexing concern, not a collection of dynamically discovered independent runtimes.

### Runtime generation

An immutable compiled representation of the currently valid project revision. It contains dense input routes, statecharts, processor kernels and instances, lane layouts, condition programs, dependency tables, state layouts, effect routes, and observation descriptors. It contains no editable node-tree traversal requirement.

### Semantic commit

An atomic publication of one completed runtime tick or event-driven evaluation boundary. Effects and observation reference its generation and tick identifiers.

## 2. Repository decision

### 2.1 Create one Golden monorepo

The recommended canonical repository is `Golden`, containing shared foundations and applications. Chataigne becomes an application inside it.

The current split across `Chataigne2`, `golden_core`, `golden_alchemist_core`, `golden_ui`, and `golden_alchemist_ui` is removed. The current root explicitly excludes core workspaces and consumes four Git submodules, which prevents atomic refactors across the boundaries that most frequently change together.

Recommended top-level layout:

```text
Golden/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── package.json
├── pnpm-workspace.yaml
├── pnpm-lock.yaml
├── AGENTS.md
├── README.md
├── docs/
│   ├── architecture/
│   ├── decisions/
│   ├── performance/
│   └── product/
├── crates/
│   ├── golden-model/
│   ├── golden-values/
│   ├── golden-parameters/
│   ├── golden-graph/
│   ├── golden-context/
│   ├── golden-alchemist/
│   ├── golden-condition/
│   ├── golden-statechart/
│   ├── golden-processor/
│   ├── golden-runtime/
│   ├── golden-protocol/
│   ├── golden-transport/
│   ├── golden-io/
│   ├── golden-persistence/
│   ├── golden-script/
│   ├── golden-host/
│   ├── golden-codegen/
│   ├── golden-testkit/
│   └── golden/
├── packages/
│   ├── golden-ui/
│   ├── golden-runtime-client/
│   ├── golden-graph-ui/
│   ├── golden-alchemist-ui/
│   └── golden-statechart-ui/
├── apps/
│   └── chataigne/
│       ├── backend/
│       ├── modules/
│       ├── ui/
│       ├── assets/
│       ├── formulas/
│       ├── fixtures/
│       └── tests/
├── tools/
├── benchmarks/
└── fuzz/
```

`pnpm` is recommended for the JavaScript workspace because strict workspace dependency declarations expose accidental imports. If the project prefers npm workspaces, the architectural requirement is still one root workspace and lockfile; package manager choice must not recreate embedded package copies.

### 2.2 Import history without preserving old boundaries

Use `git filter-repo` or subtree-based history import to retain useful history beneath the new directories. History preservation is not API compatibility and does not justify submodules.

After the monorepo passes the final cutover gate:

- archive the former repositories as read-only references;
- remove every `.gitmodules` entry;
- remove bootstrap logic dedicated to submodule initialization;
- prohibit vendored copies of Golden packages inside app directories;
- make one root commit atomically update Rust, TypeScript, generated protocol, tests, docs, and application consumers.

### 2.3 Workspace policies

- One root Rust workspace and one root JavaScript workspace.
- One pinned Rust toolchain and one pinned Node/package-manager toolchain.
- One generated-protocol drift gate.
- One dependency policy and advisory gate.
- Internal crates remain unpublished during hard development; publish only after boundaries stabilize.
- Applications depend on public crate/package APIs, never private filesystem paths.
- The `golden` Rust facade may provide the normal application-facing API, but every underlying crate must own real functionality and tests.
- No cyclic crate or package dependencies.
- No app-specific behavior inside reusable Golden crates or packages.

## 3. Dependency direction

The intended Rust dependency graph is acyclic and mostly top-down:

```text
golden-model
    ├── golden-values
    │   └── golden-parameters
    ├── golden-graph
    └── golden-context

golden-graph + golden-values
    ├── golden-alchemist
    └── golden-statechart

golden-values + golden-context + golden-alchemist
    ├── golden-condition
    └── golden-processor

golden-statechart + golden-processor + golden-condition
    └── golden-runtime

golden-model + golden-protocol
    ├── golden-persistence
    ├── golden-transport
    └── golden-host

apps/chataigne
    └── composes all public layers
```

Precise rules:

- `golden-model` knows nothing about graphs, formulas, UI, networking, Chataigne, or Tauri.
- `golden-values` knows nothing about Alchemist. It owns the canonical value and value-type system used by parameters, contexts, formulas, conditions, protocol DTOs, and module IO.
- `golden-graph` knows nothing about formula evaluation, state transitions, Chataigne modules, or runtime scheduling.
- `golden-alchemist` depends on `golden-graph`; `golden-graph` never depends on Alchemist.
- `golden-statechart` depends on `golden-graph`; it never depends on Alchemist.
- `golden-runtime` executes compiled artifacts and generic runtime systems; it does not own app node declarations or module protocols.
- `golden-ui` knows nothing about Chataigne or Alchemist.
- `golden-graph-ui` knows graph editor mechanics, not formula semantics.
- `golden-alchemist-ui` adapts Alchemist to `golden-graph-ui`.
- Chataigne-specific modules, policies, built-in formula catalogs, panels, icons, and script templates live under `apps/chataigne`.

## 4. Foundational crates

### 4.1 `golden-model`

Owns only reusable authoring/document foundations:

- stable entity IDs and generational runtime handles;
- immutable revision identifiers;
- transactions and transaction metadata;
- change sets and typed invalidation reasons;
- hierarchical entity/document storage where needed;
- labels, tags, enabled state, ownership metadata;
- undo/redo command journal primitives;
- schema-version primitives;
- diagnostic source locations that do not depend on a domain.

It must not become another god crate. Graph topology, values, parameters, persistence, transport, and runtime stay outside it.

### 4.2 `golden-values`

Move the generic value system out of Alchemist and unify the current `ParamValue`/`RuntimeValue` split.

Owns:

- `Value` and typed storage variants;
- `ValueTypeId` and value-type descriptors;
- bool, integer, float, string, file, enum, CSS value, vec2, vec3, color, duration, trigger, array, reference, and extension values;
- typed projections and component paths;
- explicit conversion rules;
- equality/change semantics;
- `ValueSet`, lane keys, trigger edge IDs, and stable value references;
- compact runtime storage descriptors;
- protocol-safe validation for numeric finiteness, sizes, and extension payloads.

There must be one canonical conversion implementation. Parameters, Alchemist, conditions, module IO, scripting, persistence, and protocol must not maintain parallel value enums.

### 4.3 `golden-parameters`

Owns authoring and control semantics layered on `golden-values`:

- constraints;
- control modes;
- context links;
- template text;
- expressions/scripts;
- automation/animation binding metadata;
- change behavior and coalescing policy;
- UI hints that are domain-neutral;
- parameter declarations and resolved parameter state.

Parameter controls compile into runtime input routes. They are not interpreted by walking parameter nodes each tick.

### 4.4 `golden-context`

Owns contexts and dimensions independently of Alchemist and Chataigne UI nodes:

- context scope IDs;
- inherited/accumulated scope resolution;
- axes, items, lists, item metadata;
- context keys and mixed-radix lane indexing;
- checked cardinality and configured budgets;
- context-value paths;
- bindings and templates;
- context-delta reconciliation plans;
- deterministic ordering;
- state migration keys across context resize.

Multiplex lists in the project document are adapters onto this model. Runtime consumers receive compiled dense lane layouts.

## 5. `golden-graph`: sole graph foundation

### 5.1 Ownership

All reusable graph-related concepts move here from Alchemist, statechart, UI packages, and app code:

- `GraphId`, `GraphNodeId`, `GraphPortId`, `GraphEdgeId`;
- graph documents and revisions;
- graph topology and indexes;
- node/edge insertion, removal, replacement, connection, disconnection, move, duplicate, group, comment;
- graph transactions and coherent deltas;
- selection-independent graph validation infrastructure;
- deterministic traversal, SCC and stable topological utilities;
- generic graph schema/domain registry interfaces;
- graph metadata;
- graph presentation document: node position/size/collapse, comments, groups, viewport bookmarks;
- connection constraints supplied by the domain;
- graph serialization envelope and migrations for the generic envelope;
- generic graph protocol DTOs/deltas;
- graph testkit and mutation/property tests.

The current `AlchemistGraph`, `AEdge`, graph comments/groups/layout, graph edit errors, and generic node/edge mechanics are therefore removed from Alchemist and reexpressed through `golden-graph`.

### 5.2 Domain model

Avoid both extremes: do not make `golden-graph` depend on formula types, and do not erase all domain data into unvalidated JSON.

Use a typed domain contract conceptually equivalent to:

```rust
pub trait GraphDomain: Send + Sync + 'static {
    type GraphData: Clone + Send + Sync;
    type NodeData: Clone + Send + Sync;
    type PortData: Clone + Send + Sync;
    type EdgeData: Clone + Send + Sync;

    fn node_ports(
        &self,
        node: &Self::NodeData,
        graph: &GraphDocument<Self>,
    ) -> PortSchema<Self::PortData>;

    fn validate_connection(
        &self,
        graph: &GraphDocument<Self>,
        from: PortRef,
        to: PortRef,
    ) -> Result<(), GraphDiagnostic>;

    fn validate_graph(
        &self,
        graph: &GraphDocument<Self>,
        changes: &GraphChangeSet,
    ) -> Vec<GraphDiagnostic>;
}
```

Exact API shape may change, but these invariants may not:

- topology and transactions are generic;
- payload and semantic validation are typed by the domain;
- graph operations produce precise change sets and revisions;
- the graph editor consumes a domain adapter rather than importing Alchemist;
- graph persistence can serialize domain payloads without making the generic graph crate know their meaning.

### 5.3 Internal structure

Keep one crate initially, with cohesive modules rather than premature microcrates:

```text
golden-graph/src/
├── document/
├── topology/
├── transaction/
├── revision/
├── domain/
├── validation/
├── traversal/
├── presentation/
├── protocol/
├── persistence/
└── testkit/
```

Split into multiple crates only when compile times, feature isolation, or independent reuse provide measured value.

## 6. `golden-graph-ui`: one reusable graph editor

The current reusable graph canvas and Alchemist editor mechanics become one generic Svelte 5 package.

It owns:

- revision-partitioned graph stores;
- viewport, selection, focus, drag, box selection, pan, zoom;
- generic node/port/edge presentation contracts;
- connection preview and validation feedback;
- comments and groups;
- undoable graph commands expressed as intents;
- spatial indexing and visible-entity queries;
- edge-routing caches keyed by endpoint geometry revision;
- virtualization and level-of-detail;
- keyboard navigation and accessibility hooks;
- canvas performance instrumentation;
- generic context menus and extension registries.

It does not own:

- ANodes;
- formula types;
- statechart transition semantics;
- Chataigne processor policy;
- module icons or inspectors;
- backend mutation semantics.

Use a hybrid renderer:

- Svelte/DOM for panels, accessible controls, inspectors, and a bounded number of visible node shells;
- retained Canvas/WebGL/WebGPU rendering for dense edges, backgrounds, minimaps, scopes, and very large graph views where profiling justifies it;
- spatial indexes so pointer work and rendering scale with visible/near-visible entities rather than total graph size.

The frontend domain adapter supplies node components, socket styles, palette entries, inspectors, connection labels, and domain-specific commands.

## 7. Alchemist: formula domain only

### 7.1 What stays in Alchemist

- formulas and formula identity;
- `AlchemistGraphDomain`;
- ANode declarations and registries;
- formula-specific typed port schemas;
- value-type constraints and type solving used by formulas;
- formula properties and defaults;
- exposed inputs, outputs, parameters, actions, and formula surface sections;
- managed-region definitions and formula-owned managed-region schema;
- formula analysis and diagnostics;
- formula compiler and source map;
- compiled formula kernels;
- formula runtime state layout and evaluator;
- formula catalog metadata and formula-file codec;
- debug/observation descriptors that do not own functional output.

### 7.2 What leaves Alchemist

- generic graph IDs and topology;
- generic graph node/edge structs;
- comments, groups, viewport, layout;
- generic graph mutation errors;
- graph selection and graph editor state;
- reusable canvas behavior;
- generic runtime values and value-type registry;
- generic contexts, axes, context keys, and multiplex enumeration;
- statecharts;
- processor lifecycle and instance scheduling;
- application conditions;
- transport and UI protocol publication.

### 7.3 Formula model

Conceptually:

```rust
pub struct AlchemistFormula {
    pub id: FormulaId,
    pub schema: FormulaSchema,
    pub graph: GraphDocument<AlchemistGraphDomain>,
    pub properties: FormulaPropertySchema,
    pub surface: FormulaSurface,
    pub metadata: FormulaMetadata,
    pub defaults: FormulaDefaults,
}
```

`ANodeId` is replaced by `GraphNodeId` at the authored graph boundary. Formula compilation maps authored graph IDs to dense `ExecNodeId`s. Runtime IDs remain Alchemist-specific because they belong to compiled formula execution rather than generic graph editing.

### 7.4 Compiler/runtime requirements

- Incremental compile keys derive from graph revision, formula schema revision, ANode registry revision, and value-type registry revision.
- Identical formula definitions compile once and are shared by all processor instances.
- The compiler emits dense typed slots, dependency bitsets, state layout, effect routes, observation source maps, liveness information, and batch-capable operations.
- Runtime evaluation uses reusable typed buffers and performs zero proportional allocation after warm-up.
- Functional output is read directly from output slots; debug capture is an optional observer.
- The IR is batch- and SIMD-ready without requiring GPU execution.
- Custom ANode evaluators must declare purity, state layout, time dependence, effects, input-change behavior, and deterministic/thread-safety capabilities.

### 7.5 `golden-alchemist-ui`

This package is a domain plugin for `golden-graph-ui`.

It owns:

- ANode rendering and socket styles;
- type/conversion visualization;
- Alchemist palette and insertion/autowire rules;
- formula surface editor;
- managed-region editor;
- formula diagnostics;
- output preview chips and formula-specific inspection;
- formula library UI primitives.

It does not own another canvas, another graph store, another selection model, or graph mutation behavior.

## 8. Statecharts are separate from Alchemist

Create `golden-statechart` around `StatechartGraphDomain`.

It owns:

- hierarchical states and parallel/compound regions;
- transition topology;
- entry/exit/history policies;
- active configuration;
- transition priority and deterministic resolution;
- statechart compiler/runtime representation;
- statechart diagnostics;
- statechart-specific graph validation;
- persisted statechart UI metadata through the generic graph presentation model.

`golden-statechart-ui` adapts it to `golden-graph-ui` and owns state/transition rendering and editing.

It must not depend on Alchemist. Chataigne composes states with processor managers at a higher layer.

State-machine transitions retain one global truth and are not multiplied by processor context dimensions. Context/multiplex applies to processors and their value lanes unless a future feature explicitly defines otherwise.

## 9. Conditions, processors, and state-machine composition

### 9.1 `golden-condition`

Owns reusable condition authoring and compiled condition semantics:

- Input Value Condition;
- Input Node Condition through registered typed condition providers;
- Condition Group with deterministic all/any/none policies;
- Script Condition through the generic script host;
- typed comparators for scalar, vector magnitude/component, color luminance/component, strings, booleans, triggers, duration, and extension types;
- transient, toggle, edge, speed, and previous-value state;
- condition diagnostics and observation descriptors;
- compiled condition IR and dense state layout.

Conditions remain a state-machine/processor concern, not an Alchemist formula example. They may compile alongside a formula kernel, but Alchemist does not own them.

### 9.2 `golden-processor`

Owns reusable formula instantiation:

- formula reference and version;
- property overrides;
- managed-region instances;
- context bindings and lane layout;
- inherited/accumulated contexts;
- lifecycle policy;
- condition program binding;
- instance state layout;
- compiled processor kernel composition;
- deterministic effect descriptors;
- processor catalog DTOs that are static by revision.

The compiled processor combines a compiled condition program with a shared Alchemist formula kernel without merging the two authoring domains.

Conceptually:

```rust
pub struct CompiledProcessorKernel {
    pub condition: Arc<CompiledConditionProgram>,
    pub formula: Arc<CompiledFormulaKernel>,
    pub dependencies: ProcessorDependencyMap,
    pub state_layout: ProcessorStateLayout,
    pub effect_layout: EffectLayout,
    pub observation: ProcessorObservationCatalog,
}
```

### 9.3 Chataigne state-machine layer

Chataigne owns product composition:

- a state contains a processor manager;
- processor folders/groups;
- Chataigne-specific default lifecycle policy;
- state activation/deactivation feeding processor activation;
- built-in Action and Mapping formula recipes;
- user formula selection and catalog policy;
- Chataigne-specific state-machine panels and inspectors;
- transition/processor commands exposed to scripts and modules.

Action and Mapping remain formulas, not hardcoded processor classes. Built-in formulas are shipped immutable, hidden from the ordinary user formula library, and exposed through the processor creation catalog. Mapping is one user-facing choice regardless of input count or conditioned output. Context and dimensions supply multiplex behavior.

## 10. Runtime architecture

### 10.1 Explicit planes

The final runtime contains six explicit planes.

#### Control plane

- sole owner of the editable project document;
- graph/document transactions;
- undo/redo;
- persistence revision coordination;
- structural UI intents;
- compile requests and diagnostic publication.

It is actor-owned. Transport never locks the control engine directly.

#### Compilation plane

- consumes immutable project revisions and precise change sets;
- incrementally compiles affected graph domains, statecharts, processors, conditions, contexts, and routes;
- continues running the previous valid generation during compilation;
- atomically swaps a new generation at a semantic boundary;
- migrates compatible state through stable typed state keys.

#### Input/IO plane

- module/device/network connection state;
- autoreconnect and recovery;
- parsing and timestamping outside semantic execution;
- typed input updates;
- bounded stream policies;
- output transmission outside semantic execution.

Use async IO facilities here, not as the CPU execution scheduler.

#### Semantic data plane

- immutable current runtime generation;
- dense input/state/output arenas;
- event-driven and scheduled rate domains;
- dirty bitsets;
- batch execution;
- deterministic semantic commit;
- no project-tree or graph-document access.

#### Effect plane

- stages commands/effects from workers;
- commits them in deterministic `(state, processor, lane, effect)` order;
- routes them to module/IO adapters;
- preserves lossless triggers/commands independently of preview pressure.

#### Observation plane

- per-client/per-view interest registry;
- reads immutable semantic commits;
- projects visible/selected values only;
- builds DTO/binary deltas and serializes off semantic/control threads;
- uses bounded latest-wins queues;
- never owns functional result computation.

### 10.2 No shared engine mutex

Replace external `Arc<Mutex<Engine>>` access with handles and typed channels:

- `ControlHandle` for intents and transaction completion;
- `RuntimeInputHandle` for values/events;
- `ObservationHandle` for subscriptions;
- `ReadModelHandle` for immutable snapshots/deltas;
- `HostHandle` for lifecycle and shutdown.

Acknowledgements distinguish received, accepted, applied, and rejected. Increasing intent timeouts is not a fix.

### 10.3 State, event, and observation semantics

| Data class | Delivery policy |
|---|---|
| Continuous state value | Latest value with monotonic revision; explicitly coalescible |
| Trigger/edge/command | Lossless and ordered within its declared scope |
| Structural intent | Lossless transaction or explicit rejection before admission |
| Drag edit values | Coalescible within one edit session; final value and session boundaries lossless |
| External sample stream | Explicit capacity and overflow policy declared by the adapter |
| Preview value | Latest-wins per client/view/key |
| Diagnostic | Bounded and deduplicated with suppression counts |

No generic event queue may silently apply one policy to all these classes.

### 10.4 Runtime generation

```rust
pub struct RuntimeGeneration {
    pub id: RuntimeGenerationId,
    pub project_revision: ProjectRevision,
    pub statecharts: Arc<[CompiledStatechart]>,
    pub processor_kernels: Arc<[CompiledProcessorKernel]>,
    pub processor_instances: Arc<[ProcessorInstanceLayout]>,
    pub contexts: Arc<CompiledContextCatalog>,
    pub input_routes: Arc<InputRoutingTable>,
    pub schedule: Arc<RuntimeSchedule>,
    pub effects: Arc<EffectRoutingTable>,
    pub observation: Arc<ObservationCatalog>,
}
```

The runtime uses stable dense slots such as `InputSlot`, `StateSlot`, `ValueSlot`, `EffectSlot`, and `LaneIndex`. Authoring UUIDs survive in source maps and catalogs but are not hash-looked-up per value.

### 10.5 Scheduler

- persistent worker pool initialized once;
- group work by shared compiled kernel;
- batch many single-lane processor instances together;
- split large lane arrays into cache-friendly contiguous chunks;
- sparse execution for low dirty density;
- dense/vectorized execution for widespread change;
- preassigned output/state ranges so completion requires no result sorting;
- deterministic effect commit independent of worker completion order;
- configurable rate domains rather than forcing all work through one global frequency;
- deadline and backlog metrics without silently skipping semantics.

Steady-state hot paths forbid JSON, strings, graph traversal, hash maps keyed by authored IDs, DTO construction, and allocations proportional to processor/lane count.

## 11. Protocol and transport

### 11.1 One source of truth

Rust owns protocol schemas. Generate TypeScript bindings in the same commit. CI regenerates and fails on drift.

### 11.2 Separate protocol planes

- reliable control/transaction messages;
- reliable structural graph/document deltas;
- coalesced value-plane deltas;
- lossless triggers/events where subscribed;
- bounded runtime observation deltas;
- diagnostics and metrics;
- snapshot/resync messages scoped to the affected plane.

Do not transport runtime preview as a generic reliable custom graph event.

### 11.3 Encoding

- Human-readable structured encoding is acceptable for low-rate intents, metadata, diagnostics, and development tools.
- Use compact binary frames for high-rate keyed values and runtime observation.
- Every frame includes protocol version, session, plane, generation/revision, sequence, and size limits.
- Local Tauri UI and remote browser clients use the same public protocol.

### 11.4 Open Studio networking

Retain the product decision: no account, password, role, token, pairing, or client approval flow by default.

Keep invisible protections:

- same-origin and configurable browser-origin policy;
- Host validation against bound/advertised interfaces;
- native clients without `Origin` remain accepted;
- bounded clients, frames, queues, subscriptions, and batches;
- slow-client isolation and scoped resync;
- mDNS discovery and copyable connection information;
- structured connection/queue metrics;
- no wildcard unbounded resource behavior.

## 12. UI architecture

### 12.1 Package ownership

#### `golden-ui`

- workbench shell;
- docking/panels;
- command routing;
- generic inspectors/outliner/dashboard primitives;
- theming and reusable controls;
- session facade over transport interfaces;
- no app/domain policy.

#### `golden-runtime-client`

- generated protocol adapters;
- connection/reconnect/resync state;
- control intent lifecycle;
- revisioned graph/document stores;
- keyed runtime value/preview stores;
- frame staging and one `requestAnimationFrame` commit;
- transport interface independent of WebSocket implementation.

#### `golden-graph-ui`

- generic graph editing described above.

#### `golden-alchemist-ui`

- Alchemist domain adapter and formula-specific UI.

#### `golden-statechart-ui`

- statechart domain adapter and statechart-specific UI.

#### Chataigne UI

- module panels/inspectors;
- State Machine composition;
- processor inspector and catalog policy;
- dashboards and Spatializer panels;
- product icons and menus;
- app-specific panel registration through public Golden hooks.

### 12.2 Reactive data rules

- Svelte 5 runes only.
- Stores are keyed and revision-partitioned by topology, geometry, selection, values, diagnostics, and viewport.
- A one-value update must not replace whole node/edge/processor collections.
- Runtime deltas stage outside reactive state and commit once per animation frame.
- One frame displays one coherent semantic revision.
- Hidden views have no observation interest.
- Lists and inspectors are virtualized.
- Graph pan/zoom/pointer work queries spatial indexes, never all nodes/edges.
- UI read functions are pure; lifecycle effects own subscriptions and intents.
- UI never decides labels, domain defaults, graph semantics, processor validity, or internal mutations.

### 12.3 Tauri

Use Tauri only for desktop shell responsibilities:

- windows;
- menus;
- native dialogs;
- OS lifecycle and packaging;
- tightly scoped capabilities.

Do not use general Tauri events for high-rate engine data. The embedded UI connects through the same versioned local API as a remote UI.

## 13. Persistence

Reset the project schema to a clean version 1. No compatibility code is required for unreleased historical schemas.

The new project file stores:

- project document entities and hierarchy;
- parameter/control configuration;
- graph documents with domain IDs and domain schema versions;
- Alchemist formulas and formula references;
- statecharts;
- conditions/processors/context configuration;
- module configuration;
- dashboards and presentation state;
- explicit app/schema versions.

It does not store compiled runtime generations as authoritative data. Optional compiled caches are disposable and keyed by exact source/toolchain/schema hashes.

Requirements:

- ordered migration registry beginning at the new v1;
- atomic temp-write, flush, and replacement;
- rolling backup and recovery journal;
- immutable save snapshot produced without holding the control plane during serialization/IO;
- limits and validation before document application;
- corruption diagnostics and recovery UX;
- formula files have their own explicit schema and migration path;
- built-in formula assets are immutable and versioned with the application.

If existing development fixtures are valuable, write one disposable offline converter to the new v1, convert all fixtures, verify them, then delete the converter. Do not ship a permanent legacy loader.

## 14. Modules, scripts, dashboards, and specialized systems

### 14.1 Chataigne modules

All concrete module behavior remains app-owned under `apps/chataigne/modules`:

- OSC;
- MIDI;
- Art-Net/DMX/sACN;
- serial;
- MQTT;
- HTTP/TCP/UDP/WebSocket;
- HID/game controllers/Stream Deck/Leap and other device integrations;
- Signal and internal utility modules;
- App Control and Node module functionality;
- future protocol-specific modules.

`golden-io` supplies reusable connection/task/recovery primitives, typed ingress/egress, bounded queues, timestamps, diagnostics, and lifecycle APIs. It contains no Chataigne module catalog.

Each endpoint integration must define:

- connection state;
- autoreconnect/device recovery;
- input timestamp and coalescing/event policy;
- output effect ordering;
- shutdown behavior;
- queue/resource limits;
- command nodes;
- script-callable methods;
- script callbacks;
- app-owned script snippets/templates.

### 14.2 Scripting

`golden-script` owns the language-neutral host contract, compiled-script cache, budgets, diagnostics, callbacks, and typed value bridge. App/module script APIs are registered from Chataigne.

Script execution must declare whether it is:

- deterministic semantic work;
- time-dependent work;
- effectful work;
- asynchronous IO;
- externally hosted work.

Long or blocking script/IO work never runs on the semantic scheduler.

### 14.3 Dashboards

Keep dashboard authoring in the project document and reusable dashboard UI primitives in `golden-ui`. Dashboard targets compile to stable value/command routes. Runtime dashboard updates use the keyed value plane rather than graph snapshots.

### 14.4 Spatializer

Retain the feature but rebuild it as a specialized compiled subsystem:

- target topology cache;
- incremental source updates;
- proven Delaunay/Voronoi implementation;
- identical backend/UI fixtures and tolerance rules;
- worker/off-main-thread preview projection;
- explicit supported scale and performance gates;
- no brute-force O(S × T²) or O(T³) steady-state path at supported counts.

## 15. Functional preservation contract

Before deleting an old capability, map it to an owner and characterization test.

| Existing capability | Final owner/path |
|---|---|
| Project node hierarchy and metadata | `golden-model` + Chataigne project schema |
| Parameters, constraints, controls, expressions | `golden-values` + `golden-parameters` |
| Context inheritance and multiplex lists | `golden-context` |
| Generic graph topology/layout/editing | `golden-graph` + `golden-graph-ui` |
| ANodes, formula typing/compilation/runtime | `golden-alchemist` |
| Formula editor/previews/surface | `golden-alchemist-ui` |
| Formula library and built-ins | Alchemist assets/catalog + Chataigne policy |
| Statechart model and runtime | `golden-statechart` |
| Statechart editor | `golden-statechart-ui` |
| Conditions | `golden-condition` |
| Processor instances, context lanes, lifecycle | `golden-processor` |
| State contains Processor Manager | Chataigne state-machine composition |
| Action and Mapping formulas | Chataigne built-in formula assets |
| Multi-input Mapping | Same Mapping formula/catalog path |
| ConditionGate filter | Alchemist ANode/filter declaration |
| ValueSet/pipelines | `golden-values` + Alchemist compiled operations |
| Module managers and concrete protocols | `apps/chataigne/modules` |
| Connection recovery | `golden-io` primitives + module adapters |
| Script APIs/callbacks/templates | `golden-script` + app/module registrations |
| Dashboards | Chataigne document + `golden-ui` primitives |
| Spatializer | Chataigne specialized compiled subsystem |
| Logs/diagnostics | runtime/transport diagnostics + Golden UI |
| Open LAN UI/API | `golden-transport` + Chataigne host configuration |
| Headless runtime | `golden-host` |
| Desktop runtime | Tauri adapter in `golden-host` |
| Save/load/recovery | `golden-persistence` + Chataigne schema |
| Runtime ANodes output previews | observation catalog + Alchemist UI keyed store |

Parity means equivalent user-observable behavior and semantics, not preservation of old paths, JSON shapes, IDs, or source APIs.

## 16. Performance qualification

### 16.1 Canonical reported fixtures

- `P50-L1`: 50 active non-multiplexed input-only actions sharing one changing Signal.
- `P5-L127`: five active actions with 127 multiplex lanes and lane-specific comparison values.

Both fixtures use deterministic threshold-crossing inputs and semantic result digests so work cannot be optimized away incorrectly.

### 16.2 Required release gates on named reference hardware

| Metric | Gate |
|---|---:|
| `P50-L1` semantic p95/p99 | ≤ 2/4 ms |
| `P5-L127` semantic p95/p99 | ≤ 4/6 ms |
| Both real debug fixtures | sustained ≥ 59.5 Hz |
| Intent accepted p95/p99 | ≤ 2/5 ms |
| Intent applied p95/p99 | ≤ 16.67/33 ms |
| Visible input-to-paint p95/p99 | ≤ 33/50 ms |
| Browser main-thread work p95/p99 | ≤ 4/8 ms per frame |
| Tasks over 50 ms | zero during a ten-minute fixture |
| Intent timeouts | zero |
| Proportional semantic allocations after warm-up | zero |
| Tree snapshots on value-only tick | zero |
| Graph/document node visits on value-only tick | zero |

### 16.3 100,000-value claim

Define the first claim precisely: 100,000 active scalar comparisons using changing shared values and precompiled references.

Test processor/lane partitions independently:

- 100 × 1,000;
- 1,000 × 100;
- 10,000 × 10;
- 100,000 stored with 1% dirty;
- 100,000 stored with none dirty.

Required release gates:

- dense p95 ≤ 8 ms and p99 ≤ 12 ms;
- 1% sparse p95 ≤ 2 ms with work proportional to dirtiness;
- no-dirty p95 ≤ 0.5 ms and zero lane evaluation;
- missed 16.67 ms deadlines below 0.1%;
- deterministic digests across 1/2/4/8 workers;
- queues and memory remain bounded;
- 200 visible values retain 60 Hz UI and p95 input-to-paint ≤ 33 ms;
- no full 100,000-value payload reaches the browser.

Complex formulas, scripts, IO, and specialized algorithms receive operation-specific budgets rather than being hidden beneath the scalar claim.

## 17. Testing and quality system

### Every pull request

- formatting, clippy with no new debt, workspace tests;
- Svelte check/lint/unit tests/build;
- generated protocol drift;
- dependency/advisory gates;
- architecture dependency-rule checks;
- semantic characterization suite;
- short processor/lane matrix;
- allocation/work-counter regression;
- project save/load round trip;
- absolute canonical performance gates on dedicated runners where stable;
- failure on meaningful regression above 5% outside noise.

### Nightly

- full processor/lane/dirty/visibility matrix;
- real browser Playwright traces;
- 100,000 dense/sparse/idle fixtures;
- slow-client, reconnect, resync, hidden-tab cases;
- continuous manipulation;
- 1/2/4/8 worker determinism;
- fuzz corpora for graph transactions, protocols, project files, and generation swaps;
- Linux, Windows, and macOS reference machines.

### Release candidate

- randomized repeated performance runs;
- ten-minute p99 distributions;
- at least eight-hour engine/network/UI soak;
- stable memory plateau and bounded queues;
- atomic-save interruption tests;
- every functionality-parity row signed off;
- no deadlock, panic, lost non-coalescible message, semantic mismatch, or intent timeout.

## 18. Clean-break implementation progression

The work should happen on a dedicated architecture branch or new canonical monorepo. The old repositories remain read-only references during implementation. Do not build permanent adapters between old and new systems.

Each phase ends with:

1. progress document updated with exact completed work;
2. characterization/performance evidence committed in machine-readable form;
3. all scoped formatting/tests passing;
4. one focused monorepo supercommit;
5. no phase marked complete without its exit criteria.

### Phase 0 — Freeze behavior and establish the new contract

> **Implementation status (2026-07-10): in progress on
> `rewrite/golden-architecture`.** The architecture decisions and dependency rules are
> being frozen under `docs/architecture`, and the functional preservation contract is
> being expanded into a machine-readable parity ledger. Phase 0 will remain uncommitted
> and will not be marked complete until every ledger entry has automated evidence or an
> explicit manual acceptance scenario, both canonical failures reproduce with semantic
> digests, and the current end-to-end baseline is recorded.

Deliver:

- exhaustive functionality inventory and parity matrix;
- canonical `P50-L1` and `P5-L127` fixtures;
- semantic digests for conditions, formulas, statecharts, contexts, effects;
- module connection/script/dashboard/Spatializer characterization tests;
- current end-to-end baseline;
- ADRs for monorepo, graph boundary, runtime planes, value unification, and no legacy.

Exit:

- every existing capability has a test or an explicitly documented manual acceptance scenario;
- the reported performance failures reproduce;
- the new dependency rules are frozen.

### Phase 1 — Create the monorepo and foundational types

> **Implementation status (2026-07-10): complete.** The four former Golden repositories
> are imported with history under `legacy/repositories`, their gitlinks are removed, and the
> old Chataigne application is a read-only characterization reference. One root Cargo
> workspace and one root npm workspace now own pinned toolchains, locks, CI, and deterministic
> codegen support. The active workspace contains `golden-model`, `golden-values`,
> `golden-parameters`, `golden-context`, clean UI package boundaries, and thin Chataigne
> composition packages. CI rejects submodules, legacy workspace members, forbidden internal
> dependencies, malformed evidence, formatting drift, clippy warnings, test failures, and
> TypeScript failures. The foundational characterization suite passes without initializing
> any submodule.

Deliver:

- import repository histories under final paths;
- root Rust and JavaScript workspaces;
- unified toolchains, locks, CI, codegen, docs;
- `golden-model`, `golden-values`, `golden-parameters`, `golden-context`;
- one canonical `Value` replacing parallel parameter/runtime value models;
- no submodule-based build path in the new workspace.

Exit:

- the workspace builds from one clone without submodule initialization;
- foundational crates contain no Alchemist/Chataigne/UI imports;
- value/context/parameter characterization tests pass.

### Phase 2 — Build `golden-graph` and `golden-graph-ui`

> **Implementation status (2026-07-10): complete.** `golden-graph` owns typed domain
> contracts, stable graph identities, indexed topology, rollback-backed atomic transactions,
> revisions, precise changes, validation, deterministic traversal/SCC utilities, presentation,
> and a domain-adapted protocol envelope. Alchemist and statechart crates prove two independent
> real domains on the same foundation. `golden-graph-ui` owns stable revision-partitioned maps,
> comments/groups, a grid spatial index, visible/hit-test queries, domain adapters, and an
> accessible Svelte 5 canvas shell. Tests prove zero existing-payload clones for a one-node edit
> in a 10,000-node graph, stable untouched UI record identity, and viewport work bounded to
> nearby cells. CI enforces the Rust and JavaScript dependency boundaries.

Deliver:

- typed graph-domain contract;
- transactions, revisions, precise deltas, topology indexes, presentation model;
- generic graph protocol;
- revision-driven Svelte graph store;
- generic editor/canvas with spatial index and domain adapters;
- property/mutation tests;
- remove generic graph concepts from the new Alchemist code.

Exit:

- a test graph domain and at least two real domains can use the same graph/document/editor stack;
- one-node changes do not copy whole graph maps;
- viewport/pointer work scales with visible entities;
- no Alchemist-specific import exists in graph packages.

### Phase 3 — Rebuild Alchemist on `golden-graph`

Implementation status (2026-07-10): complete on `rewrite/golden-architecture`. Formula authoring, v1 file conversion, compilation, shared immutable kernels, dense dirty-slot evaluation, opt-in observation, batching, catalog protection, and the composed Svelte editor/store are implemented. Automated evidence is recorded in `benchmarks/phase3/alchemist-foundation.v1.json`; ownership guidance lives in `docs/architecture/alchemist.md`.

Deliver:

- `AlchemistGraphDomain`;
- formula model, properties, surface, managed regions, registries;
- new formula file schema;
- type solver and compiler targeting dense batch-capable IR;
- direct functional output slots and optional observation;
- Alchemist UI as a graph-domain adapter;
- built-in/user formula catalog primitives;
- conversion of existing formula fixtures.

Exit:

- all formula functionality and characterization tests pass;
- identical formulas compile once;
- Alchemist owns no generic graph/layout/canvas/value/context/statechart concept;
- unchanged pure evaluation performs no work.

### Phase 4 — Rebuild statechart, conditions, contexts, and processors

Implementation status (2026-07-10): complete for the clean rewritten workspace on `rewrite/golden-architecture`. Statecharts and predicates compile to immutable runtime forms; contexts compose before bounded lane compilation; processor lanes share formula kernels; Action, Mapping, and ConditionGate are wired through public boundaries; the app shell composes state transitions with processors; and the Svelte statechart adapter uses keyed runtime deltas. Automated evidence is recorded in `benchmarks/phase4/control-foundation.v1.json`; ownership guidance lives in `docs/architecture/statechart-processors.md`.

Deliver:

- statechart model/runtime/UI on `golden-graph`;
- compiled condition IR;
- processor instance model;
- context/multiplex lane compiler;
- Action and Mapping built-in formula assets;
- inherited/accumulated contexts;
- one Mapping creation path for single/multi-input/conditioned output;
- ConditionGate as an Alchemist filter ANode;
- state/processor composition and semantic parity.

Exit:

- statechart truth is non-multiplexed;
- conditions are not interpreted node walks;
- processor formulas share kernels;
- all existing state-machine features pass characterization tests.

### Phase 5 — Build the control/compiler/data/effect runtime

Implementation status (2026-07-11): complete for the clean rewritten workspace on `rewrite/golden-architecture`. The single-owner control plane compiles immutable graph-free generations in the background, migrates stable state, atomically publishes successful candidates, and keeps the prior valid generation active on rejected work. The semantic plane uses generation-stamped direct inputs, dense typed arenas, flattened routes, sparse/dense scheduling, and deterministic staged effects. Debug and release workload evidence is recorded in `benchmarks/phase5/runtime-foundation.v1.json`; ownership guidance lives in `docs/architecture/runtime.md`.

Deliver:

- actor-owned control plane;
- compilation service and immutable runtime generations;
- direct input slots and dependency routes;
- state migration and atomic generation swap;
- dense typed runtime arenas;
- deterministic batch scheduler;
- sparse/dense execution;
- deterministic effect commit;
- no editable graph access in the semantic plane.

Exit:

- value-only ticks show zero project snapshot, topology traversal, binding reconstruction, and proportional allocation;
- `P50-L1` and `P5-L127` pass release and debug gates;
- graph edits compile asynchronously while the previous valid generation continues.

### Phase 6 — Build protocol, transport, observation, and runtime UI stores

Deliver:

- generated multi-plane protocol;
- engine/control handles with no transport mutex access;
- per-client/view observation interests;
- static catalogs and keyed preview deltas;
- binary high-rate value frames;
- bounded latest-wins preview queues and reliable control queues;
- Svelte frame staging and coherent rAF commit;
- slow-client isolation;
- open-network safeguards and observability.

Exit:

- zero intent timeouts under both canonical workloads;
- no full runtime bundle on value changes;
- UI frame/input-to-paint gates pass in a real browser;
- slow clients do not affect semantic or healthy-client performance.

### Phase 7 — Port the Chataigne product

Deliver:

- thin Chataigne composition shell;
- every module family and connection recovery;
- commands, script methods/callbacks/templates;
- dashboards;
- Spatializer replacement;
- state-machine product panels and inspectors;
- built-in formula assets and catalog policy;
- headless and Tauri hosts;
- actual open LAN workflow.

Exit:

- every functional preservation row passes;
- app-specific behavior exists only under `apps/chataigne` or through explicit registrations;
- Golden packages remain app-agnostic.

### Phase 8 — New persistence and recovery

Deliver:

- clean v1 project schema;
- atomic save, backups, recovery journal;
- immutable save snapshots;
- new formula asset schema;
- converted development fixtures through a disposable offline tool;
- corruption, interruption, large-project, and round-trip tests.

Exit:

- all final fixtures load/save/reload identically;
- interrupted save cannot destroy the last good project;
- no permanent legacy loader remains.

### Phase 9 — Scale qualification and deletion

Deliver:

- 100,000-value dense/sparse/idle qualification;
- graph editor scale fixtures;
- module/network/client soak;
- cross-platform release tests;
- delete every old runtime, old protocol, old graph store, old Alchemist graph type, compatibility adapter, submodule, and obsolete doc;
- archive old repositories;
- final architecture and contributor documentation.

Exit:

- all performance, UI, persistence, networking, and parity gates pass;
- production has one path for each responsibility;
- no legacy feature flag or dual runtime remains;
- a new contributor can understand ownership and dependency direction from the root docs in minutes.

## 19. Explicit non-solutions

The implementation must reject these shortcuts:

- increasing intent timeouts;
- lowering preview frequency below the required visible feedback rate;
- disabling preview to qualify semantic performance;
- adding more parallel iterators around interpreted manager logic;
- copying the full project graph to a worker each tick;
- creating `golden-graph` as a renamed folder while leaving graph concepts duplicated elsewhere;
- making `golden-graph` depend on Alchemist;
- hiding domain payloads in unvalidated JSON everywhere;
- introducing a generic god `golden-core` crate again;
- retaining submodules for packages that require atomic changes;
- shipping both old and new protocols/runtimes;
- preserving unreleased project formats at the cost of the new model;
- moving work to unbounded queues and calling the engine faster;
- claiming 100,000-item scale from a headless benchmark while the real UI remains unresponsive.

## 20. Final acceptance statement

The architecture is complete only when the following statement is true:

> Golden provides one reusable project/value/parameter/context foundation, one generic graph document and editor system, separate Alchemist formula and statechart domains built on that graph foundation, an incremental compiler producing immutable data-oriented runtime generations, isolated IO/effect/observation planes, a responsive keyed Svelte UI, one generated public protocol, and one coherent monorepo. Chataigne preserves its current creative functionality while containing only product-specific composition, modules, assets, and UI. Steady-state execution and visible feedback meet published scale gates, and no legacy architecture remains in production.

That is the target against which every implementation choice should be judged.
