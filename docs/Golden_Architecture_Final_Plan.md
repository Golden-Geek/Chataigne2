# Golden / Chataigne2 Final Product-Preserving Architecture and Migration Plan

## Executive decision

This plan starts from `Golden-Geek/Chataigne2` `main` and replaces its repository layout and runtime center without replacing Chataigne with a foundation demo. At the start of execution, resolve and record the exact `origin/main` commit; the presently observed working baseline is `f0a7c2fe4d192a076c7649c6fe3e6a5ab193a435` (`before aaa implementation`). That application is the product specification, executable reference, and continuous acceptance harness throughout the migration.

Internal APIs, crate boundaries, Git submodules, runtime protocols, and the unreleased project schema may be replaced. Existing UX, panels, graph-editing behavior, inspectors, modules, formulas, state-machine workflows, scripting, dashboards, Spatializer behavior, desktop hosting, and open-LAN operation may not disappear merely because their present implementation is coupled to the old architecture.

The requirement is functional and experiential preservation, not source-level preservation. Existing code may be moved, split, refactored, or replaced, but a capability is removed only through an explicit product decision by the project owner. Architectural purity is never implicit authorization to delete product behavior.

Temporary migration adapters, shadow execution, dual reads, fixture converters, and development-only feature switches are allowed when they keep the real application usable while a vertical slice moves. They must have named owners, deletion criteria, and tests. “No legacy” describes the final production state; it does not prohibit safe migration machinery.

The final system is considered the best attainable foundation when:

- `golden-graph` is the sole owner of reusable graph concepts and graph editing infrastructure;
- Alchemist is a Chataigne-owned formula domain built as a plugin over app-agnostic `golden-graph` contracts;
- statecharts are a separate domain built on `golden-graph`, with no dependency on Alchemist;
- the editable project/graph model is never the steady-state runtime representation;
- project changes compile incrementally into immutable runtime generations;
- control, semantic execution, IO, effects, observation, and UI have explicit boundaries and queue semantics;
- steady-state value processing performs no tree snapshot, graph traversal, protocol serialization, or allocation proportional to project size;
- the UI receives bounded keyed deltas for visible data and remains responsive independently of semantic load;
- all former Git submodules are replaced by one coherent monorepo workspace;
- the full Chataigne UI and module catalog are present and connected to a real engine after every completed migration phase;
- a developer can launch the desktop app, open a representative project, manipulate it, and observe live runtime feedback at every named runnable checkpoint;
- old implementations may be replaced directly during a declared construction interval and are absent once checkpoint parity is proven.

## Product-preservation doctrine

The following rules override any later wording that could be read as permission to build an empty replacement application.

1. **The existing application is the oracle.** Preserve a named working baseline commit and executable build for behavior, interaction, visual, module, and project-fixture comparison.
2. **Import first; reorganize second.** The first monorepo milestone imports the complete Rust application, Svelte UI, assets, built-in formulas, modules, scripts, fixtures, and desktop host. It must run before foundational extraction begins.
3. **Keep one real product checkpoint.** The last runnable Chataigne checkpoint remains immutable while construction proceeds. At the next checkpoint, the actual Chataigne UI must talk to an actual engine; a headless runtime, test graph, protocol demo, or placeholder workbench does not satisfy that gate.
4. **Migrate vertical slices.** Each slice includes authoring model, runtime/compiler behavior, transport, UI store, panels/inspectors, persistence, diagnostics, and tests. Do not build six backend layers and defer the product to a late “port” phase.
5. **Lift and refactor existing UX.** Start generic UI packages by moving and adapting the working components. Rewriting a component is allowed only with side-by-side interaction parity; recreating the UI from memory is not.
6. **Modules are product features.** A registered name, empty adapter, mock-only backend, or disabled menu item is not module parity. Each module retains creation, configuration, connection/recovery, values, commands, script surface, diagnostics, and relevant custom UI.
7. **Prefer direct replacement during construction.** Old code may be deleted as part of a coordinated in-scope replacement once the immutable baseline/checkpoint, affected parity rows, and recovery path are recorded. Do not create dual paths solely to keep intermediate commits runnable.
8. **Every accepted checkpoint is demoable.** If the full app does not build, launch, connect, load a representative project, and expose the expected panels/modules, the checkpoint is incomplete regardless of crate-level tests. Construction intervals are never described as product-validated.
9. **Preserve testing access.** Existing development projects must remain openable or receive verified converted equivalents. One documented command must launch the complete app with the canonical performance/UX fixture.
10. **Deliberate UX changes are reviewed as product changes.** Pixel identity is not mandatory, but missing controls, altered workflows, degraded feedback, changed shortcuts, lost docking/layout behavior, or simplified inspectors require explicit approval and evidence that the result is equal or better.

## Immediate recovery from an unusable rewrite

If an implementation has already produced a product-empty or non-runnable rewrite:

- freeze and tag both current `main` and the rewrite head; destroy neither;
- create a fresh `architecture/aaa-product-rewrite` branch or worktree from the recorded `origin/main` commit;
- do not promote or merge the rewrite branch into that new trunk merely because its new crates are cleaner;
- treat the rewrite as a donor for individually reviewable foundations, tests, and algorithms;
- transplant those pieces behind the running product one vertical slice at a time;
- restore the complete existing UI and module sources before further architectural milestones are accepted;
- fix the default developer build and launch path before performance work continues;
- maintain a rollback point for every subsystem cutover until its parity and soak gates pass.

The goal is not to bolt the old UI onto an unrelated new demo at the end. The goal is to evolve `main` into the target architecture while preserving a continuous, testable Chataigne application. The final UI and engine code may be entirely different; the final product capability and UX must be equal or better.

## Failed-rewrite donor policy

The published failed rewrite is `rewrite/golden-architecture` at `174fc5096ac7ab4546b3acc76569bca6a1c9e01d`, 493 commits ahead of the recorded working baseline. It is not the migration base.

Its current state proves why architectural nouns and unit tests are not product parity:

- it deletes roughly 140,000 lines of working product code and assets while replacing them with small foundation sketches;
- the baseline app plus UI packages contain roughly 106 Svelte components and 1.44 MB of Svelte source, while the rewrite retains only three tiny Svelte files totaling about 4 KB;
- the root `Cargo.toml` is a virtual workspace with no default runnable Chataigne binary, so ordinary root `cargo run` is lost;
- `apps/chataigne/ui/src/index.ts` registers a handful of panel descriptors but supplies no actual workbench application or replacement panel components;
- the complete `src-ui` application, inspectors, panels, graph/formula/state-machine UI, icons, assets, and browser tooling were deleted;
- `apps/chataigne/backend/src/modules.rs` lists module descriptors and script-shape placeholders but does not implement the former protocol/device modules;
- `golden-host` describes a launch plan and shell trait but does not provide the former runnable Tauri application;
- no Tauri/HTTP/WebSocket application server or frontend bundler produces a usable product, and the nominal npm build only type-checks packages;
- nominal browser tests mount isolated data structures into synthetic page content rather than mounting and operating the Svelte application;
- the Chataigne backend is not wired to the new runtime/graph/protocol crates as a real compiled project path; project/statechart/processor data remain placeholders and the foundation runtime is effectively self-tested in isolation;
- the real built-in `Action` and `Mapping` formula assets were deleted;
- broad entries in `functional-parity.v1.json` point to foundation unit tests and call them complete without reproducing corresponding user workflows.

Useful work from that branch may still be salvaged, but only through a versioned donor ledger:

| Donor item | Required review before import |
|---|---|
| Crate/package boundary | Re-evaluate dependency direction, cohesion, public API, and whether the split has real ownership rather than plan-shaped scaffolding |
| Value/graph/context types | Compare exhaustively with `main` semantics and project/UI needs; add conversion and characterization tests |
| Compiler/runtime algorithm | Prove semantic digest, effect behavior, state migration, allocation, and canonical real-app performance before cutover |
| Transport/queue code | Prove delivery/backpressure policies and integrate with real server/client/host paths |
| UI store/component | Require actual rendered component and interaction parity; registry metadata or store tests alone do not qualify |
| Module/host/persistence code | Require operational implementation, platform integration, and end-to-end tests; descriptors and traits do not qualify |
| Benchmark/parity evidence | Re-run against `main` workflows and reject topology-only, pending, ignored, mock-only, or assertion-free evidence |

Do not merge or cherry-pick the donor branch wholesale. Import or reimplement one reviewed unit at a time into the `main`-based migration branch. A donor test may accompany code, but it never replaces the baseline characterization and continuous product gates.

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
│   ├── golden-condition/
│   ├── golden-statechart/
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
│   └── golden-statechart-ui/
├── apps/
│   └── chataigne/
│       ├── backend/
│       ├── alchemist/
│       ├── processor/
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
    └── golden-statechart

golden-values + golden-context
    └── golden-condition

golden-statechart + golden-condition
    └── golden-runtime

golden-model + golden-protocol
    ├── golden-persistence
    ├── golden-transport
    └── golden-host

apps/chataigne/alchemist
    └── depends on golden-graph, golden-values, and domain-neutral runtime contracts

apps/chataigne/processor
    └── composes Alchemist with golden-condition, golden-context, and golden-runtime contracts

apps/chataigne
    └── composes Alchemist with all required public Golden layers
```

Precise rules:

- `golden-model` knows nothing about graphs, formulas, UI, networking, Chataigne, or Tauri.
- `golden-values` knows nothing about Alchemist. It owns the canonical value and value-type system used by parameters, contexts, formulas, conditions, protocol DTOs, and module IO.
- `golden-graph` knows nothing about formula evaluation, state transitions, Chataigne modules, or runtime scheduling.
- Chataigne Alchemist depends on `golden-graph`; no reusable Golden crate depends on Alchemist.
- `golden-statechart` depends on `golden-graph`; it never depends on Alchemist.
- `golden-runtime` executes compiled artifacts and generic runtime systems; it does not own app node declarations or module protocols.
- `golden-ui` knows nothing about Chataigne or Alchemist.
- `golden-graph-ui` knows graph editor mechanics, not formula semantics.
- Chataigne's Alchemist UI adapts its app-owned domain to `golden-graph-ui`.
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

## 7. Chataigne-owned Alchemist: formula domain only

Alchemist is not a reusable Golden package. It is a Chataigne bounded context that implements the
public `golden-graph` domain contracts and consumes app-agnostic value/runtime services. Phase 4
relocates its imported implementation to `apps/chataigne/alchemist` before changing the authoring
or runtime model.

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

### 7.5 Chataigne Alchemist UI

This app-owned UI is a domain plugin for `golden-graph-ui` and lives under
`apps/chataigne/ui/src/lib/alchemist`.

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

### 9.2 Chataigne processor composition

Because processor instances select and instantiate Alchemist formulas, this layer is Chataigne-owned
under `apps/chataigne/processor`. It owns:

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

The compiled processor combines a reusable compiled condition program with a shared app-owned
Alchemist formula kernel without merging the two authoring domains. `golden-runtime` executes the
result through domain-neutral kernel/effect contracts and never imports the Chataigne processor or
Alchemist implementation.

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

#### Chataigne Alchemist UI

- app-owned Alchemist domain adapter and formula-specific UI built on `golden-graph-ui`.

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

All concrete module behavior remains app-owned under `apps/chataigne/modules`.

The operational `main` baseline contains 23 real modules:

- protocol: Generic OSC, MIDI, MQTT, HTTP, Serial, UDP, TCP Client, TCP Server, WebSocket Client, and WebSocket Server;
- controllers/devices: Buttplug, Gamepad, Joy-Con, Keyboard, Mouse, Kinect 2, Stream Deck, and Ultraleap;
- generators: Signals, Metronomes, and Spatializer;
- system: App Control and OS.

Art-Net/sACN/DMX and Node have appeared as dependencies, icons, designs, or rewrite catalog names but are not complete operational modules in the recorded `main` baseline. Track them as separate new-feature requirements after baseline parity; never use their names to inflate the restored-module count. The same rule applies to every future protocol-specific module.

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

Preserve the actual dashboard product: authoring and viewer modes, pages/tabs, layouts, widgets, bindings/routes, drag/drop, resize, selection, inspectors, persistence, and live values/commands. Backend dashboard structs or route tests without the authoring UI are not parity.

### 14.4 Spatializer

Retain the feature but rebuild it as a specialized compiled subsystem:

- preserve all baseline modes and controls, including Voronoi behavior, target/source radius, overlap and freeze-radius rules, 2D/3D layouts, value-layout selection, visual debugging, and the complete editor;
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
| ANodes, formula typing/compilation/runtime | `apps/chataigne/alchemist` |
| Formula editor/previews/surface | `apps/chataigne/ui/src/lib/alchemist` + `golden-graph-ui` |
| Formula library and built-ins | Chataigne Alchemist assets/catalog and product policy |
| Statechart model and runtime | `golden-statechart` |
| Statechart editor | `golden-statechart-ui` |
| Conditions | `golden-condition` |
| Processor instances, context lanes, lifecycle | `apps/chataigne/processor` |
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

### 15.1 Mandatory parity ledger

Phase 0 creates a versioned ledger from the working application. One broad row such as “modules” or “UI” is insufficient. Each independently observable capability receives a row with:

| Field | Requirement |
|---|---|
| Stable capability ID | Never reused; suitable for CI and progress tracking |
| Product area | Workbench, graph, formula, state machine, module, script, dashboard, Spatializer, persistence, networking, host, diagnostics |
| Working-baseline source | Repository/ref and relevant source or asset paths |
| User workflow | Exact steps a user performs and expected feedback |
| Runtime semantics | Inputs, outputs, state, ordering, timing, errors, and recovery behavior |
| Final owner | Target crate/package/app directory |
| Executable evidence ID | Stable test/scenario ID invoked by the parity runner; unit, semantic digest, integration, Playwright, screenshot, protocol, simulator, or hardware scenario |
| Last passing result | Commit SHA, toolchain/target/features, timestamp, result artifact ID/hash, and relevant measured digest/screenshot/trace |
| Manual evidence | Required only where hardware or visual judgment cannot be automated |
| Migration state | Baseline, adapted, shadowing, cut over, old path removed |
| Temporary adapter | Owner, scope, expiry phase, and deletion issue |
| Approval | Explicit sign-off for any intentional behavioral or UX change |

Unknown and untested rows are blockers. “Not yet ported” is honest progress; “complete” is not.

The parity checker executes or consumes the signed machine result for every declared evidence ID. It never marks a row complete merely because a source/test file path exists, a catalog contains a name, or a unit test elsewhere passed. A result becomes stale when its capability implementation, transitive runtime path, UI workflow, fixture, toolchain, or evidence test changes; CI then requires re-execution.

### 15.2 UX surface inventory

The ledger must enumerate at least:

- application startup, loading feedback, empty/default project, save/open/reload, recent files, recovery, and error presentation;
- the workbench shell, docking, panel creation/removal, layout persistence, menus, context menus, commands, keyboard shortcuts, focus, and theming;
- outliner/tree navigation, selection and multi-selection, inspector routing, breadcrumbs, search/reference popups, duplication, drag/drop, and undo/redo;
- parameter editors for every value type, constraints, control modes, references, templates, expressions, animation/automation affordances, and live feedback;
- graph pan/zoom, node creation, connection creation/removal, reconnection, box selection, comments/groups, clipboard/duplication, navigation, validation, and previews;
- Alchemist formula creation/open/edit, formula surface, managed sections, built-in/external/shared formulas, read-only behavior, export/removal guards, ANode catalog, diagnostics, and output previews;
- statechart editing, state activation, transitions, conditions, processor managers/groups, Action and Mapping creation, contexts/multiplex controls, lane selection, inspectors, and live lane/output feedback;
- module manager workflows plus every custom module panel and inspector;
- dashboards, Spatializer, logs, performance/diagnostic views, script editing, and app/module script affordances;
- local Tauri, remote browser, reconnect/resync, open-LAN discovery/addressing, and multiple-client behavior.

For each major baseline screen and interaction path, capture a reference screenshot or short deterministic trace. Visual tests detect missing structure; Playwright interaction tests prove the controls actually work. A screenshot of a static shell is not UX parity.

### 15.3 Module parity contract

Create a generated module manifest from the working registry and compare it in CI with the migrating application. The initial parity manifest must contain the 23 operational baseline modules listed in Section 14.1 and any additional runtime-backed entry discovered from the exact recorded gitlinks. Planned Art-Net/sACN/DMX, Node, and other new modules use separate feature rows and cannot be marked “restored.” For every concrete module, the ledger covers:

- creation and deletion;
- complete declared parameter/value/command hierarchy;
- defaults and visibility rules;
- connection state, reconnect, device enumeration, and shutdown;
- input parsing/timestamp/coalescing or event policy;
- output ordering and error behavior;
- command nodes and triggers;
- concrete command execution semantics and parameters—not only command-name strings—including MIDI message variants/raw data, stream string/bytes/hex/value/JSON sends, HTTP request/upload, OSC custom messages, controller outputs, App Control/OS operations, generic logging, and output-group delay/stagger/nesting/cancel-pending behavior discovered from the baseline manifest;
- script methods, callbacks, snippets, and templates;
- persistence round trip;
- generic and custom inspectors/panels;
- diagnostics and unavailable-hardware behavior.

Protocol modules use loopback or deterministic simulator tests. Hardware modules use an injectable transport/device boundary plus recorded fixtures; a small named manual hardware matrix supplements those tests. Feature-gated hardware may report “unavailable” cleanly, but it may not prevent the normal application and the rest of the module catalog from building and launching.

### 15.3.1 Formula, condition, processor, and state-machine parity

The same anti-placeholder standard applies to the creative runtime:

- generate and compare the complete baseline ANode registry, currently spanning dozens of primitive families such as Math, Function, Remap, smoothing/filtering, Speed, Counter, LFO, Noise, Metronome, geometry, color, strings, logic, triggers, delay/debug, and Chataigne module/manager/command bridges;
- support every canonical baseline value type, default, conversion, socket behavior, managed region, persistent state, side effect, and diagnostic; implementing only bool/integer/float/string or a handful of arithmetic operations is not Alchemist parity;
- restore the real shipped `Action` and `Mapping` formula files and icons first, then compile them through the new system. A one-node pass-through or bool gate is not an acceptable replacement;
- preserve Action's trigger/filter/command workflow and Mapping's input/filter/output model, including multiple inputs, `ValueSet` behavior, conditioned output, contexts, and multiplex lanes;
- preserve project/shared/external Formula Library behavior, built-in read-only inspection, editable-copy/export/removal flows, file watching, rename/relink/delete handling, formula source selection, and live previews/history;
- preserve processor property mirroring/overrides, managed-region instances/lowering, active-state continuous evaluation, lifecycle, inherited contexts, deterministic command arbitration/effect dispatch, selected-lane inspection, and output previews;
- characterize every Input Value condition comparator and state mode: equality/inequality, ranges/outside, string contains/not/starts/ends/regex, `value_changed`, numeric/vector magnitude, speed/absolute speed, color luminance/alpha, component projection, typed bool/string/vector/color references, transient and toggle behavior;
- preserve condition-group reductions including all/any/none/at-least/exactly where present, plus Input Node and Script Condition authoring even where `main` runtime behavior is incomplete. Baseline stubs are recorded honestly as unfinished capability and completed to the agreed final specification rather than mislabeled as operational parity;
- verify states and transitions, processor groups/folders, contexts/multiplex dimensions, formulas, conditions, outputs, commands, and module effects together through a loaded real project—not only isolated crate tests.

The Phase 0 ledger distinguishes three states: operational baseline behavior to preserve, baseline UI/persistence scaffolding whose intended behavior still needs implementation, and newly planned functionality. Only the first category can be called “restored”; all three remain explicit final-product work.

### 15.4 Local iteration and cross-platform qualification gates

Migration validation uses two profiles so the normal edit loop stays fast without weakening the
portable product contract:

- **Win-x64 iteration profile.** Every focused `RUNNABLE` migration supercommit must pass the
  applicable checks locally on `x86_64-pc-windows-msvc`, the active development platform. Narrow
  checks may be used while editing, but the complete local product gate runs before the
  supercommit is handed off or used as the base for another architectural slice.
- **Cross-platform qualification profile.** The same gate expands to Windows MSVC, macOS, Linux,
  and the supported compatibility targets at the end of Phases 1B, 3, 6, 8, and 9; before a merge
  to `main`, release, packaging cutover, or old-path deletion that depends on platform behavior;
  and whenever a slice changes host startup, native dependencies, target selection, packaging, or
  platform-specific code. A qualification may be run earlier when useful, but routine pushes to a
  long-lived migration branch do not require online CI.

The local and qualification profiles share one product-gate contract. The applicable profile must:

1. builds and tests the complete Rust workspace;
2. checks, tests, and production-builds the complete Svelte workspace;
3. builds the real Chataigne backend and desktop host, not only libraries;
4. starts the real backend, connects the real frontend, and waits for an explicit ready/connected state;
5. loads the canonical converted-or-current project fixture;
6. exercises graph selection/edit, inspector mutation, formula/state-machine interaction, live value feedback, and save/reload through Playwright;
7. compares registered panel, command, node-type, ANode, formula, module, and script-surface manifests with the approved ledger;
8. runs a representative loopback module input/output test through the real engine;
9. captures screenshots and browser console/network failures;
10. connects a browser through a real non-loopback LAN address to prove the client does not hardcode `localhost` and the advertised/open-network workflow works;
11. build the selected validation matrix with platform-appropriate optional integrations.

The gate fails on a blank UI, disconnected UI, placeholder-only catalog, missing panel registration, console exception, intent timeout, unusable fixture, or omitted default application binary. A green library test suite cannot override a red product gate.

Deferred cross-platform evidence is recorded as `NOT_RUN`, never inferred from the Windows result.
It does not block intermediate work on the canonical migration branch, but it blocks the named
qualification point and any final merge, release, or deletion that requires it. Portability remains
an implementation constraint between qualifications: prefer cross-platform libraries and explicit
target boundaries, and do not knowingly accumulate a platform-specific design that merely happens
to pass on Windows.

### 15.5 Toolchain and native dependency gate

The default build must not depend on accidental local linker tools or an undocumented native SDK. In particular:

- Windows uses the declared MSVC toolchain unless a GNU target is an explicit separately tested deliverable;
- no build script may assume `dlltool` exists on an MSVC-only development machine;
- native SDK discovery is explicit and diagnostic;
- optional hardware integrations are isolated behind features or dynamically loaded adapters when practical;
- lack of one optional device SDK does not erase the UI or make engine-only/product-core development impossible;
- CI verifies both the normal developer configuration and feature-complete platform builds on appropriate runners;
- one root bootstrap/check command validates Rust, Node, package manager, native prerequisites, generated sources, UI-engine connectivity, and the application launch path.

One committed root toolchain manifest is canonical. Standard consumer files such as `rust-toolchain.toml`, `.node-version`, `package.json` `packageManager`/`engines`, CI setup, bootstrap scripts, and editor configuration are generated from it or verified byte/semantically against it. Scripts may not override the pin with ad hoc `stable`, `stable-msvc`, or machine-local defaults.

Every check report records `rustc -vV`, `cargo -vV`, target host, Node version, package-manager version, OS, and enabled feature set. Define two explicit feature matrices:

- **normal developer/product-core:** full UI, engine, network protocols that have portable dependencies, simulated hardware boundaries, and no optional vendor SDK requirement;
- **feature-complete platform:** all integrations supported on that OS, run on appropriately provisioned CI/hardware runners.

Both matrices build real binaries. “Engine-only” cannot be the normal developer substitute for a broken Chataigne application.

### 15.6 Stable root developer commands

Repository/package/folder refactors must update the toolchain in the same change. From the monorepo root, these user-facing entry points remain stable throughout the migration and in the final system:

| Entry point | Required contract |
|---|---|
| `cargo run` | Build and launch the normal complete Chataigne application with its real backend and bundled/default UI path |
| `watch` | Run the established live-development workflow from the root. Phase 0 records what the current command means; the final implementation is owned by a checked-in `cargo xtask watch` command with thin root `watch`, `watch.cmd`, and `watch.ps1` wrappers so no globally installed watcher defines project behavior |
| `cargo run -- --dev` | Launch the complete application in development mode with the live frontend/dev-server workflow, engine connection, generated protocol, and hot-development feedback intact |

Also maintain root commands for complete checks, UI-only development, headless operation, code generation, benchmarks, product E2E, and packaging, but those may be renamed once documented. The three commands above are compatibility contracts and may not silently disappear.

Every path move updates, in the same supercommit:

- Cargo workspace members, package paths, build scripts, generated-code inputs/outputs, asset embedding, and Tauri configuration;
- JavaScript workspace members, package dependencies, scripts, lockfiles, TypeScript aliases, Vite/Svelte configuration, and generated imports;
- `watch` orchestration, process shutdown/restart behavior, readiness detection, ports, and source globs;
- root bootstrap/check scripts, CI working directories, cache keys, platform setup, VS Code tasks, contributor docs, and agent repository-routing instructions;
- test/fixture/asset/formula/module discovery paths.

A folder architecture change is incomplete if the new crates compile only through ad hoc `--manifest-path` commands while any required root entry point is broken.

The watch contract is explicit:

- one orchestrator owns every child process and writes one machine-readable readiness line containing backend, frontend, engine-connection, project-revision, and port state;
- fixed default ports remain compatible with the current workflow; overrides are explicit, and an occupied required port produces an actionable failure rather than silently connecting to an unrelated process;
- generated protocol/assets are checked before startup and regenerated/restarted when their source schemas change;
- Svelte/Vite uses HMR for UI changes; Rust changes rebuild/restart the backend/application without leaving duplicate servers; workspace/config changes trigger the appropriate full restart;
- frontend readiness, backend readiness, and UI-to-engine session readiness are distinct and bounded;
- Ctrl-C, terminal close, child failure, or orchestrator failure terminates the complete process tree and releases ports/files;
- nonzero child/orchestration failures propagate a nonzero exit status; successful deliberate shutdown returns zero;
- logs are labeled by process, startup timeouts are configurable and bounded, and stale generated output cannot be treated as ready;
- all watcher/runtime dependencies are workspace-pinned and bootstrap-validated.

CI command smokes execute literal bare `cargo run` and literal `cargo run -- --dev` from the root as child processes. The harness:

1. asserts Cargo resolves exactly one default runnable Chataigne package/binary;
2. waits within a fixed timeout for backend, frontend, and engine-connected readiness;
3. loads and mutates a named fixture through the real client;
4. verifies visible/runtime feedback and save/reload where applicable;
5. requests documented graceful shutdown and verifies a clean exit with no child or bound port remaining.

Run separate mounted remote-browser and Tauri-shell smokes. Headless HTTP/WebSocket hello checks do not substitute for either.

### 15.7 Honest checkpoint and construction states

Every migration interval declares one state before implementation:

| State | Rules |
|---|---|
| `CHECKPOINT_RUNNABLE` | The complete applicable build, test, product, UI-engine, fixture, and root-command matrix for the required validation profile must run and pass before the checkpoint is accepted |
| `CONSTRUCTION` | Permitted on the canonical migration branch between named checkpoints; the objective, affected layers, expected breakages, focused checks, next checkpoint, and immutable last runnable checkpoint are documented before work |

For a `CHECKPOINT_RUNNABLE` interval:

- never claim validation from source inspection alone;
- never substitute `cargo check` for binaries/tests that can actually compile and run;
- run every relevant test after code generation and the real build succeeds;
- run the real UI against the real backend when the phase affects either side;
- report skipped/ignored/platform-gated tests separately with reasons;
- a failed prerequisite is a failed phase, not evidence that downstream checks “would pass.”

Unless Section 15.4 requires a cross-platform qualification for that phase or change, the required
profile is the Win-x64 iteration profile. A phase that is a named qualification point is incomplete
until the full declared platform matrix passes.

For a `CONSTRUCTION` interval:

- the progress document prominently records that full application launchability is not guaranteed;
- no full-app, UI, performance, or product-parity validation is claimed;
- focused compile, unit, contract, serialization, migration, and performance checks run wherever
  their dependencies are available, and every skipped or broken check has an exact reason;
- coordinated API/schema replacement and deletion are allowed within the declared scope, but the
  interval is not a subsystem cutover, release, merge-to-`main`, or completed phase;
- the recorded baseline, last runnable checkpoint ref/report, product manifests, modules, assets,
  fixtures, and recovery instructions remain available even when their former implementation path
  is removed from the working tree;
- unrelated feature or optimization work waits until the named checkpoint restores full-product
  validation; construction commits remain reviewable through explicit WIP commits or tags.

The canonical migration branch may therefore contain `CONSTRUCTION` heads. Only named checkpoints
are required to be fully runnable, and only those checkpoints may claim application parity.

All phase reports record `PASS`, `FAIL`, `NOT_RUN`, or `BLOCKED` for each exact command together with commit SHA, toolchain fingerprint, target/features, exit code, ignored tests, manual checks, and artifact IDs. A compilation failure is `FAIL`; dependent tests are `BLOCKED`, never silently “not applicable.” “Tests pass” without a successfully compiled applicable target is forbidden.

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

### Every focused migration supercommit

- validation-state declaration and exact executed-command report;
- formatting, clippy with no new debt, workspace tests;
- Svelte check/lint/unit tests/build;
- `cargo run`, `watch`, and `cargo run -- --dev` smoke gates for every `RUNNABLE` phase/change affecting build or layout;
- real Chataigne binary/package, real bundled frontend artifact, and real UI-engine readiness/connectivity checks;
- generated protocol drift;
- dependency/advisory gates;
- architecture dependency-rule checks;
- semantic characterization suite;
- short processor/lane matrix;
- allocation/work-counter regression;
- project save/load round trip;
- panel/module/command/ANode/formula/script manifest parity;
- Playwright interaction through the mounted Chataigne app rather than synthetic `page.setContent` component/data-structure tests;
- absolute canonical performance gates on dedicated runners where stable;
- failure on meaningful regression above 5% outside noise.

Run this list locally on Win-x64 during ordinary migration work. A pull request is a review and
qualification vehicle, not a prerequisite for keeping a long-lived migration branch runnable:
open a focused PR when a named cross-platform qualification or merge point is ready. The
cross-platform profile adds the platform matrix and platform-appropriate integrations required by
Section 15.4.

### Nightly

- full processor/lane/dirty/visibility matrix;
- real browser Playwright traces;
- 100,000 dense/sparse/idle fixtures;
- slow-client, reconnect, resync, hidden-tab cases;
- non-loopback LAN browser connection, discovery/address display, and multi-client control/observation;
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

## 18. Product-preserving implementation progression

The work starts on a fresh migration branch/worktree created from the recorded `origin/main` commit. That branch is transformed into the canonical monorepo by importing the complete dependency repositories and product sources; an empty replacement repository is not an alternative path. The last working repositories, commits, and named runnable checkpoints remain immutable comparison points. After the initial import checkpoint, the migration branch may enter declared construction intervals under Section 15.7.

Temporary adapters are optional tools for persisted-data migration or checkpoint risk reduction, not a default migration technique. Every adapter is recorded in the parity ledger with an owner and deletion phase. Permanent compatibility architecture is still forbidden.

Every phase listed below ends with a `CHECKPOINT_RUNNABLE` gate. Its internal implementation may use one or more declared `CONSTRUCTION` intervals on the canonical migration branch. Never describe a failed checkpoint as an accepted construction result after the fact.

The implementation updates this table in every phase-closing supercommit:

| Phase | Validation | Initial status |
|---|---|---|
| 0 — `main` baseline and contract | `CHECKPOINT_RUNNABLE` | Pending |
| 1A — complete-product monorepo import | `CHECKPOINT_RUNNABLE` | Pending |
| 1B — toolchain modernization | `CHECKPOINT_RUNNABLE` | Pending |
| 2 — product seams and shadowing | `CHECKPOINT_RUNNABLE` | Pending |
| 3 — foundations and graph extraction | `CHECKPOINT_RUNNABLE` | Pending |
| 4 — Alchemist vertical migration | `CHECKPOINT_RUNNABLE` | Pending |
| 5 — statechart/condition/context/processor migration | `CHECKPOINT_RUNNABLE` | Pending |
| 6 — runtime-center cutover | `CHECKPOINT_RUNNABLE` | Pending |
| 7 — protocol/observation/UI migration | `CHECKPOINT_RUNNABLE` | Pending |
| 8 — concrete modules and specialized systems | `CHECKPOINT_RUNNABLE` | Pending |
| 9 — final qualification and deletion | `CHECKPOINT_RUNNABLE` | Pending |

Each phase ends with:

1. its final validation state is `CHECKPOINT_RUNNABLE`, with every preceding construction interval and its focused evidence recorded honestly;
2. the progress and parity ledgers updated with exact completed, shadowing, cut-over, and remaining work;
3. only genuinely executed characterization, visual, connectivity, and performance evidence committed in machine-readable form;
4. every applicable compile/build command executed first and recorded; any failure marked `FAIL`, with dependent tests explicitly `BLOCKED`, otherwise all applicable checks executed;
5. the required Section 15.4 product-gate profile and `cargo run`, `watch`, and `cargo run -- --dev` contracts passing against the real backend and real Svelte/Tauri application; the Win-x64 profile is sufficient between named qualification points;
6. an immutable checkpoint ref/report for the independently buildable and launchable complete product;
7. no phase marked complete without its honest exit criteria;
8. no unrelated feature or optimization work carried across an unfinished construction interval.

### Phase 0 — Branch from `main`, prove the product, and freeze the contract

Deliver:

- fetch and record the exact `origin/main` SHA, verify it is the intended working product, and create `architecture/aaa-product-rewrite` from that SHA;
- record every submodule gitlink SHA and recursively nested dependency revision from that exact `main` tree; baseline source, UI, formula, test, and behavior inventories are taken from those gitlinks, never from whichever repository heads are newest;
- immutable tags/refs for the recorded `main` baseline and the failed/current rewrite head;
- a clean migration worktree based on `main`, with the rewrite retained only as a donor and comparison source;
- a reproducible Windows MSVC, macOS, and Linux bootstrap/build matrix;
- the complete mandatory parity ledger and generated manifests for panels, commands, node types, ANodes, formulas, modules, script surfaces, and fixtures;
- reference screenshots and deterministic Playwright traces for the principal workflows;
- canonical `P50-L1` and `P5-L127` fixtures exercised through the real application;
- semantic digests for values, conditions, formulas, statecharts, contexts, effects, module loopback, save/reload, and UI-observed results;
- ADRs for monorepo, graph boundary, runtime planes, value unification, product-preserving migration, temporary adapters, and final legacy deletion;
- update `AGENTS.md`/contributor instructions so “thin app shell,” “no legacy,” and “no compatibility shims” cannot be interpreted as permission to discard Chataigne product code; temporary adapters required by this migration are explicitly authorized and governed;
- one root command that proves backend build, UI build, backend readiness, frontend connection, representative project load, and live feedback;
- executable characterization of `cargo run`, the current `watch` workflow, and `cargo run -- --dev`, including processes, ports, readiness, generated-code prerequisites, restart/shutdown behavior, and expected UI-engine connection.

Exit:

- the full `main` product builds and launches from a documented clean environment;
- the UI is connected to the engine and the canonical project is visibly testable;
- every existing capability is represented by an automated test or explicit manual scenario;
- optional native integrations cannot break the normal developer build merely because an unrelated SDK or GNU `dlltool` is absent;
- `cargo run`, `watch`, and `cargo run -- --dev` work from the baseline root and have automated smoke coverage;
- the reported performance failures reproduce end to end;
- no destructive rewrite work proceeds until this gate is green.

**Supercommit:** `main` baseline, manifests, fixtures, product gate, and migration ADRs only; no failed-rewrite merge.

### Phase 1A — Form the monorepo by importing the complete working product

Deliver:

- import the histories and complete contents of `Chataigne2`, `golden_core`, `golden_alchemist_core`, `golden_ui`, and `golden_alchemist_ui` under their intended monorepo locations;
- import the exact baseline gitlink revisions recorded in Phase 0 before replaying any later donor work;
- import all Svelte components, styles, icons, assets, built-in formulas, module implementations, scripts/templates, fixtures, Tauri configuration, platform resources, and packaging metadata;
- one root Rust workspace, one root JavaScript workspace, pinned toolchains, root locks, CI, codegen, and bootstrap commands;
- the existing Chataigne app wired as the monorepo application entry point before any subsystem replacement;
- a real Chataigne binary target with root workspace `default-members` selecting it and package `default-run` set where needed, so the workspace never regresses into a library-only virtual manifest;
- mechanical path/package moves separated from semantic refactors so source history and regressions remain reviewable;
- correct optional-native-feature boundaries and platform toolchain selection;
- root-compatible `cargo run`, `watch`, and `cargo run -- --dev` orchestration updated for the new workspace paths;
- updated agent/code-navigation workspace routing, editor tasks, contributor docs, and architecture maps for the monorepo;
- no submodule initialization required by the new workspace.

Phase 1A is mechanical: retain the baseline Rust/Node/package-manager/framework/dependency versions and lock contents except for unavoidable path/source rewrites. Toolchain, dependency, package-manager, and framework modernization belongs only to Phase 1B runnable supercommits.

Do not create empty future package folders and call the product “ported.” A future `golden-graph-ui` package initially may contain moved working graph UI code that is not yet perfectly generic; preserving behavior comes before completing that extraction.

Exit:

- one clone builds and launches the same full Chataigne product;
- the workbench, panels, inspectors, graph/formula/state-machine editors, modules, dashboards, Spatializer, logs, and settings are present;
- approved panel/module/command/catalog manifests match the working baseline;
- representative projects load, run, save, and reload;
- local Tauri and remote browser workflows both work;
- all three stable developer entry points run from the monorepo root without submodule-era paths;
- the old split repositories are still retained as references, but the monorepo is now the only migration worktree.

**Supercommit:** repository integration with zero intentional product behavior change.

### Phase 1B — Modernize and unify the toolchain without changing the product

Deliver:

- audit and deliberately select current supported Rust, Cargo, target, Node, package-manager, Svelte 5, TypeScript, Vite, Tauri, test-runner, code-generation, formatting, linting, and native build tool versions;
- pin them in the canonical root toolchain manifest and locks; generate/verify standard tool-consumed version files and use Corepack or the package manager's official pinning mechanism where applicable;
- update dependencies in coherent groups with release-note/migration review, rather than one opaque lockfile churn;
- retain Svelte 5 runes and modernize existing UI code in place where dependency upgrades require it;
- make Windows MSVC the normal Windows build, add correct macOS/Linux prerequisites, and document separately tested optional target/toolchain variants;
- isolate native/hardware libraries behind explicit Cargo features or dynamic adapters where practical, with clear unavailable-device diagnostics;
- replace submodule-era and folder-specific codegen/build paths with public workspace packages and root-relative configuration;
- establish formatter, clippy, dependency/advisory, license, unused-dependency, duplicate-version, generated-drift, TypeScript, Svelte, and package-audit gates;
- install required Linux Tauri/WebKit system dependencies in CI instead of allowing the job to fail before source validation;
- make the frontend production build produce and verify the actual deployable application artifact, not only run type checks;
- update root `cargo run`, `watch`, `cargo run -- --dev`, headless, check, benchmark, E2E, and packaging orchestration;
- update editor tasks, debug launch configurations, environment diagnostics, cache keys, and contributor onboarding.

Use separate runnable supercommits for Rust/native tooling, JavaScript/UI tooling, and developer orchestration when needed. Do not combine a package-manager migration, framework upgrade, monorepo move, and runtime rewrite into an unreviewable single diff.

Exit:

- a clean supported machine can bootstrap from documented prerequisites;
- the complete application builds and runs on Windows MSVC, macOS, and Linux in the supported configuration;
- `cargo run`, `watch`, and `cargo run -- --dev` pass their real UI-engine smoke contracts from the root;
- production UI output is embedded/served by the normal app and remote browser path;
- all existing projects/fixtures, panels, modules, formulas, and workflows remain present;
- CI reaches and executes source tests rather than failing on missing environment packages;
- tool versions and upgrade policies are documented and reproducible.

**Supercommits:** coherent dependency/toolchain groups, each `RUNNABLE` and product-gated.

### Phase 2 — Establish stable product seams and shadow infrastructure

Deliver:

- explicit application-facing facades for project transactions, graph editing, runtime values, observation, module IO, persistence, and host lifecycle;
- current backend and UI connected through those seams without changing workflows;
- typed temporary adapters where the current protocol/model cannot yet implement the final interface;
- dual-run/shadow hooks for pure deterministic subsystems, with semantic digest comparison and zero user-visible double effects;
- injectable IO/device boundaries and deterministic protocol/hardware recordings;
- revisioned parity dashboards showing row-by-row baseline, adapted, shadowing, cut-over, and removed status;
- contract tests preventing reusable Golden packages from importing Chataigne policy.

Exit:

- the working application still uses real production implementations through the new seams;
- replacements can be selected per subsystem or document without changing UI registration;
- shadow execution cannot send duplicate outputs, commands, triggers, or device traffic;
- temporary adapters are bounded, tested, and scheduled for deletion;
- the product gate remains identical or better than Phase 1B.

**Supercommit:** migration seams and shadow harness, no user-facing replacement yet.

### Phase 3 — Extract foundations and `golden-graph` through the live product

Deliver in small vertical slices:

- `golden-model`, `golden-values`, `golden-parameters`, and `golden-context` extracted from working behavior;
- canonical `Value` conversions with current parameter controls, module values, scripts, persistence, and UI all covered;
- typed `golden-graph` domain contract, transactions, revisions, deltas, topology indexes, presentation model, protocol, and persistence envelope;
- move the existing graph canvas/store/components into `golden-graph-ui`, then progressively remove app/Alchemist assumptions while preserving interaction and visuals;
- first adapt a test graph domain, then the real Alchemist domain, then the real statechart domain;
- spatial indexing and revision-partitioned stores introduced behind the existing graph UX;
- converted fixtures plus temporary old/new graph-document adapter where required.

Cut over one concern at a time: identifiers, values, transactions, topology, presentation, then store/protocol. Do not replace the entire graph/backend/UI stack in one commit.

Exit:

- every baseline graph interaction and inspector route passes against the real Chataigne UI;
- a test domain and two real domains use the common graph foundation;
- no Alchemist-specific import remains in final graph packages;
- one-node changes do not clone whole graph maps and viewport work scales with visible entities;
- all modules and non-graph UI remain operational;
- old graph ownership is removed only for the cut-over slices.

**Supercommits:** one per cut-over slice, each product-gated; a phase-closing supercommit removes only proven duplicate graph paths.

### Phase 4 — Migrate Alchemist as a complete authoring-to-runtime slice

Deliver:

- relocate the complete Alchemist Rust and UI implementations from their imported `golden_*`
  locations into the Chataigne app boundary, preserving public behavior and recorded provenance;
- keep `AlchemistGraphDomain` app-owned on the public `golden-graph` contract, with architecture
  checks proving that neither `golden-graph` nor `golden-graph-ui` imports Alchemist;
- formula model, surface, properties/defaults, managed regions, registries, type solver, compiler, dense IR, state layout, direct functional outputs, and optional observation;
- existing ANode catalog and behaviors, including scripts, value operations, effects, managed structures, and ConditionGate;
- move/adapt the complete current formula UI: graph editing, formula surface, properties, built-in/external/shared formulas, catalog, export/removal guards, diagnostics, preview modes, lane selection, and ANode output previews;
- built-in Action and Mapping formula assets and the user-facing catalog policy;
- formula/fixture converter with verified semantic and visual results;
- shadow evaluation of deterministic formulas before processor/runtime cutover.

Exit:

- all existing formula workflows are available in the same full application;
- all ANode and formula manifest entries have functional tests or explicit manual hardware/effect scenarios;
- identical formulas compile once and unchanged pure evaluation performs no work;
- preview/debug capture is observational only and the visible feedback remains smooth;
- the old Alchemist graph/runtime/UI path is removed only after all rows are signed off.

**Supercommits:** ownership relocation, authoring model, compiler/runtime, UI adapter, fixtures, then
cutover/removal; each remains runnable. The relocated Rust crate uses the Chataigne-owned
`chataigne_alchemist` name; no reusable Golden package alias remains in production.

### Phase 5 — Migrate statecharts, conditions, contexts, and processors vertically

Deliver:

- statechart domain/runtime/UI on `golden-graph` while preserving current state-machine panels and interaction model;
- compiled condition IR for Input Value, Input Node, Condition Group, and Script Condition, including all comparator/projection/transient/toggle/speed behavior;
- processor instance model, groups/folders, property overrides, formula selection, context bindings, inherited/accumulated contexts, lifecycle, and lane state;
- context/multiplex lane compiler and state migration;
- Action and the single user-facing Mapping path, including multi-input and conditioned behavior;
- preserved state/transition/condition/processor/context inspectors, catalogs, context menus, validation, previews, and live output/lane UI;
- shadow semantic comparison with the old implementation without emitting duplicate effects.

Exit:

- statechart truth remains non-multiplexed;
- conditions no longer walk editable nodes during steady state;
- identical processor formulas share kernels;
- all existing state-machine and context workflows pass UI and semantic characterization;
- `P50-L1` and `P5-L127` can be created, run, observed, and manipulated from the actual UI;
- no module, dashboard, script, or persistence regression is present.

**Supercommits:** statechart authoring/UI, conditions, contexts/processors, product composition, then cutover/removal.

### Phase 6 — Replace the runtime center behind the continuously working app

Deliver:

- actor-owned control plane;
- incremental compiler and immutable runtime generations;
- direct input slots, dependency routes, dense typed arenas, and stable lane/state/output layouts;
- state migration and atomic generation swap;
- persistent deterministic batch scheduler with sparse/dense execution;
- isolated deterministic effect commit;
- module inputs and outputs connected to the new runtime through production adapters;
- a temporary compatibility observation/control adapter so the complete current UI remains usable while protocol stores migrate;
- safe shadow mode for semantic comparison and effect suppression;
- runtime metrics surfaced in the existing diagnostics/performance UI.

Exit:

- value-only ticks show zero project snapshot, topology traversal, binding reconstruction, and proportional allocation;
- `P50-L1` and `P5-L127` pass release and real debug-application gates;
- graph edits compile asynchronously while the previous valid generation continues;
- actual module inputs drive the runtime and actual module outputs/effects preserve ordering;
- the full UI remains responsive, connected, and testable; headless success alone is insufficient;
- rollback to the old runtime remains possible until the following observation/protocol cutover passes.

**Supercommits:** control/compiler, input/data plane, scheduler, effects/modules, shadow qualification, then runtime cutover.

### Phase 7 — Migrate protocol, observation, and UI stores panel by panel

Use separately gated `RUNNABLE` subphases so the existing roughly hundred-component UI is migrated as product code, not replaced by registry metadata:

1. **7A — Session and workbench:** connection/reconnect/resync, application shell, docking/layout persistence, menus/commands, theme, startup/loading/error states.
2. **7B — Core authoring UI:** outliner, selection/multi-selection, generic inspectors and parameter controls, dashboard, logger/diagnostics, dialogs, reference popups, context menus, and undo/redo feedback.
3. **7C — Generic graph and Alchemist:** graph canvas interactions, formula editor/surface/library, managed items, ANode editors, validation, previews, clipboard, toolbar, and catalog workflows.
4. **7D — State machine:** statechart canvas, states/transitions, conditions, processor managers/groups, context/multiplex controls, inspectors, lane selectors, and live previews.
5. **7E — Modules and specialized panels:** Modules panel, module inspectors/commands/indicators, Spatializer editor, custom per-module UI, icons, and traffic/connection feedback.
6. **7F — Packaging and remote client:** production bundle, embedded Tauri assets, browser entry point, LAN client, accessibility/keyboard pass, performance traces, and old-store/protocol adapter deletion.

Deliver:

- generated multi-plane protocol and typed runtime client;
- control handles with received/accepted/applied/rejected lifecycle and no transport engine mutex;
- reliable structural deltas, coalesced values, lossless triggers, bounded observation deltas, and scoped resync;
- per-client/per-view interest registry, static catalogs, keyed preview deltas, and binary high-rate frames where measured;
- bounded latest-wins observation queues and slow-client isolation;
- Svelte frame staging and coherent `requestAnimationFrame` commits;
- migrate existing UI stores and components panel by panel, retaining all components and interactions unless an approved improvement replaces them;
- local Tauri and remote browsers use the same public protocol;
- explicit removal checklist for each temporary old-protocol adapter.

Exit:

- every existing panel and inspector uses the final client/store path or an explicitly approved non-runtime path;
- no full runtime bundle is sent for value-only changes;
- no intent timeout occurs under canonical workloads;
- UI frame and input-to-paint gates pass in a real browser with the full interface visible;
- slow or disconnected clients do not affect the engine or healthy clients;
- the temporary protocol/observation compatibility path is deleted only after complete panel manifest parity.

**Supercommits:** control client, graph/document stores, value/preview stores, panel migrations by product area, then adapter removal.

### Phase 8 — Migrate every module and specialized product subsystem

The implementations and UIs were imported and kept functional in Phase 1A. This phase changes their foundations one family at a time; it does not recreate the product late.

Execute this as separately gated `RUNNABLE` subphases, refined by the authoritative Phase 0 registry manifest:

1. **8A — Module framework:** module manager, base Connection/Parameters/Values/Commands structure, traffic/logging, reference filtering, paging, script/context hosting, diagnostics, recovery contracts, and test transports.
2. **8B — Generators first:** Signal and Metronome provide deterministic end-to-end value/event fixtures before external protocols move.
3. **8C — Message/control protocols:** OSC and MIDI, preserving dynamic received-value structure, complete commands, parsing, connection/device behavior, and UI.
4. **8D — Network and stream protocols:** Serial, MQTT, HTTP, TCP client/server, UDP, and WebSocket client/server, including recovery, server/client multiplicity, ordering, and backpressure.
5. **8E — Controllers and hardware:** gamepad, Joy-Con, keyboard, mouse, Kinect2, Stream Deck, Ultraleap, Buttplug, and every additional baseline registry entry through injectable device adapters and named hardware checks.
6. **8F — System integrations:** App Control and OS, including background polling isolation, process lifecycle, commands, callbacks, and platform behavior.
7. **8G — Specialized authoring:** Spatializer backend/editor and dashboards, retaining their complete existing panels and live workflows while replacing only measured algorithmic bottlenecks.
8. **8H — Script and asset surface:** shared script host plus every per-module method, callback, snippet/template, formula asset, icon, and product registration.
9. **8I — Persistence and hosts:** project/formula schema conversion, atomic recovery, desktop/headless/open-LAN hosts, discovery, packaging, signing/notarization hooks, and release assets.
10. **8J — Approved module expansion after parity:** implement planned Art-Net/sACN/DMX and Node modules, plus any newly approved modules, to the same full creation/configuration/IO/recovery/command/script/persistence/UI/diagnostic standard. These are new features, not evidence for baseline restoration.

Deliver:

- move shared connection/task/recovery/queue primitives to `golden-io`;
- migrate protocol modules family by family while preserving their node trees, controls, commands, scripts, diagnostics, and custom UI;
- migrate hardware modules through injectable device adapters, feature-complete platform builds, recordings, and named hardware checks;
- migrate scripting host/cache/budgets while retaining all app/module APIs, callbacks, snippets, and templates;
- migrate dashboards to compiled stable routes while preserving authoring UI;
- replace Spatializer internals with cached/proven geometry while preserving its editor, controls, live behavior, and fixtures;
- migrate logging, diagnostics, settings, discovery, headless host, Tauri host, and packaging;
- clean v1 project/formula schemas, verified fixture conversion, atomic save, backups, recovery journal, corruption UX, and large-project round trips.

Exit for each family before moving to the next:

- all module-manifest rows exist and are functional, not stubs;
- loopback/simulator and applicable hardware scenarios pass;
- custom panels and inspectors remain present and interactive;
- scripts and callbacks match semantics;
- projects persist and reload the subsystem;
- connection failure/recovery is observable and bounded;
- the whole application product gate remains green.

Phase exit:

- every functional preservation row is signed off;
- final fixtures load, run, save, and reload equivalently;
- interrupted save cannot destroy the last good project;
- no optional native dependency prevents the standard product from building;
- app-specific behavior exists under `apps/chataigne` or explicit registrations while Golden packages remain app-agnostic.

**Supercommits:** one per coherent module family or specialized subsystem, never one mass “port all modules” commit.

### Phase 9 — Final qualification, approved UX improvements, and deletion

Deliver:

- complete functionality and UX parity review in the real app;
- deliberate UI improvements only as separate product-reviewed commits after parity is established;
- 100,000-value dense/sparse/idle qualification;
- graph editor scale fixtures with the full workbench present;
- module/network/multi-client/hardware soak;
- cross-platform desktop and browser release tests;
- install/package smoke from produced Windows installer/archive, macOS bundle/package, and Linux artifact on clean test environments, followed by launch, UI-engine connection, fixture mutation, save/reload, and uninstall/cleanup where applicable;
- remove every old runtime, protocol, graph store, superseded Alchemist graph type and former
  `golden_alchemist*` package path, temporary adapter, dual-run switch, converter not needed for the
  new v1, submodule, and obsolete doc;
- archive old repositories only after the monorepo is self-contained and the working baseline remains tagged;
- final architecture, module-authoring, UI-extension, performance, troubleshooting, and contributor documentation.

Exit:

- all performance, UI, persistence, networking, module, packaging, and parity gates pass;
- the application is recognizably and functionally Chataigne, with equal or better UX;
- production has one path for each responsibility;
- no temporary migration adapter, old runtime flag, or dual implementation remains;
- a clean machine can build, launch, connect, load the canonical project, and test the engine through the full UI using documented commands;
- a new contributor can understand ownership and dependency direction from root documentation in minutes.

**Supercommit:** final cutover/deletion only after every gate is green; UX improvements remain separately reviewable.

## 19. Explicit non-solutions

The implementation must reject these shortcuts:

- starting the accepted migration from the failed rewrite instead of `main`;
- replacing Chataigne with a crate showcase, protocol demo, test graph, blank workbench, or skeleton UI;
- deleting or postponing the current panels, inspectors, editors, modules, assets, or workflows with a promise to recreate them in a late product-port phase;
- claiming module parity from catalog names, traits, mocks, or stubs without real configuration, IO/recovery, commands, scripts, persistence, diagnostics, and UI;
- claiming a phase is complete because libraries compile while the real app does not launch or the UI cannot connect to the engine;
- treating a virtual Cargo workspace, interface-only host/transport, or typecheck-only npm script as a product build;
- marking parity complete because an evidence path or descriptor name exists without executing the user workflow and consuming its result;
- calling synthetic `page.setContent` tests of stores/spatial indexes “browser UI tests” when the Svelte product is never mounted;
- converting compilation failure into “tests not applicable” instead of `FAIL` plus explicitly `BLOCKED` dependent checks;
- letting bootstrap/CI scripts override the pinned toolchain with `stable`, `stable-msvc`, or local defaults;
- rewriting the UI from memory instead of inventorying, importing, and validating the working interface;
- treating “no legacy in the final result” as a prohibition on temporary, bounded, tested migration adapters;
- accepting an undocumented platform linker/SDK dependency such as accidental `dlltool` availability as the normal developer build path;
- increasing intent timeouts;
- lowering preview frequency below the required visible feedback rate;
- disabling preview to qualify semantic performance;
- adding more parallel iterators around interpreted manager logic;
- copying the full project graph to a worker each tick;
- creating `golden-graph` as a renamed folder while leaving graph concepts duplicated elsewhere;
- making `golden-graph` depend on Alchemist;
- treating Alchemist as a reusable Golden package instead of a Chataigne-owned graph domain;
- hiding domain payloads in unvalidated JSON everywhere;
- introducing a generic god `golden-core` crate again;
- retaining submodules for packages that require atomic changes;
- shipping both old and new protocols/runtimes;
- preserving unreleased project formats at the cost of the new model;
- moving work to unbounded queues and calling the engine faster;
- claiming 100,000-item scale from a headless benchmark while the real UI remains unresponsive.

## 20. Final acceptance statement

The architecture is complete only when the following statement is true:

> Golden provides reusable project/value/parameter/context foundations, one app-agnostic graph
> document and editor system, an incremental domain-neutral runtime foundation, isolated
> IO/effect/observation planes, a responsive keyed Svelte UI, one generated public protocol, and one
> coherent monorepo. Chataigne owns Alchemist as its formula-domain plugin over those public
> contracts and preserves its current creative functionality through product-specific composition,
> modules, assets, and UI. Steady-state execution and visible feedback meet published scale gates,
> and no legacy architecture remains in production.

And the product statement is simultaneously true:

> From the repository root, a developer can use `cargo run`, `watch`, or `cargo run -- --dev` as appropriate; launch the complete Chataigne interface; recover the workbench, docking layout, panels, outliner, inspectors, graph and formula editors, state machine, dashboards, Spatializer, logs, icons, and live previews; create and use the full module catalog; author states, transitions, conditions, processors, inherited contexts, multiplex dimensions, Action/Mapping/custom formulas and ANodes; connect real or simulated IO; run scripts and commands; save/reopen projects; and use the same application locally through Tauri or remotely on the LAN. The implementation underneath may be entirely new, but no unapproved product capability has disappeared.

That is the target against which every implementation choice should be judged.
