# Parameter Control Modes - Specification

For `golden_core` (host-agnostic)

---

## 1. Purpose

Define one coherent engine-level model for parameter control so the runtime and UI can support:

- context links selected from menus
- context links embedded as tokens in text fields
- one-line expressions that can read context
- proxying to another node/parameter of compatible type
- 2-way bindings between compatible parameters
- animations (local parameter drivers)

The design must scale to large graphs and remain deterministic.

---

## 2. Core Rule: One Runtime Control Plane

All control modes compile to one runtime abstraction. Do not implement separate engines per mode.

```rust
enum ControlMode {
    Manual,
    ContextLink,
    TemplateText,
    Expression,
    Proxy,
    Binding,
    Animation,
}

struct ParamControlState {
    mode: ControlMode,
    spec: ControlSpec,
    compiled: Option<CompiledControlPlan>,
    diagnostics: Vec<ControlDiagnostic>,
}
```

`ControlSpec` is persisted author intent.
`CompiledControlPlan` is rebuilt during resolve/reevaluate.

---

## 3. Mode Semantics

### 3.1 Manual

- Source is the parameter's own stored value.
- No external dependency edges.

### 3.2 ContextLink

- Source is a single resolved context endpoint.
- Uses lexical scope resolution from `node_contexts.md`.
- Optional view selector (`@current`, `@dim`, fixed index).

### 3.3 TemplateText

- Text is split into literal segments and dynamic token segments.
- Each token compiles to an endpoint plan (typically context or node metadata).
- Runtime evaluation concatenates segments in-order.

Example conceptual source:
- `"Track {trackName} - {sequence.$name}"`

### 3.4 Expression

- One-line expression compiled to AST/bytecode.
- Expression inputs are endpoint references (including context).
- Runtime evaluates expression with typed conversion rules.

### 3.5 Proxy

- Parameter reads/writes through a designated compatible target.
- Target is usually another parameter selected by type/facet constraints.
- Proxy cycles are invalid and must produce diagnostics.

### 3.6 Binding (2-way)

- Two parameters synchronize values bidirectionally via a binding mediator.
- Requires explicit compatibility and conversion policy.
- Loop prevention is mandatory (origin token + transaction id).

### 3.7 Animation

- Parameter value driven by local animation data/time.
- No context dependency required.
- Animation still participates in the same compile/runtime pipeline.

---

## 4. Unified Compiled Representation

```rust
enum CompiledControlPlan {
    Manual,
    Endpoint(EndpointRef),          // context link, proxy read path
    Template(CompiledTemplate),
    Expression(CompiledExpr),
    Binding(CompiledBinding),
    Animation(CompiledAnimation),
}
```

Notes:

- `EndpointRef` is shared with the context system.
- Proxy and context-link reuse the same endpoint infrastructure.
- Expression and template modes are endpoint consumers, not special-cased graph systems.

---

## 5. Type and Conversion Policy

Each control plan must declare input and output types.

```rust
struct TypeContract {
    source: ValueType,
    target: ValueType,
    conversion: ConversionPolicy,
}
```

Rules:

- No implicit lossy conversion unless explicitly allowed by policy.
- Binding requires forward and reverse conversion definitions.
- Invalid contracts produce diagnostics and disable only that control plan.

---

## 6. Resolve and Reevaluate

### 6.1 What "reevaluate" does

Resolve/reevaluate is the place to:

- compile `ControlSpec -> CompiledControlPlan`
- rebuild dependency edges for update scheduling
- resolve lexical context symbols
- resolve proxy/binding targets
- compute diagnostics

### 6.2 Triggers

Reevaluation must run (preferably incrementally) when:

- a node/parameter is moved or reparented
- context scope schema changes
- a control spec changes
- a target's type/availability changes
- blueprint/model migration changes field structure

### 6.3 Important boundary

Reevaluation rebinds references and dependencies. It does not mutate user-authored control specs unless explicitly requested by tooling.

---

## 7. UI Data Contract

UI must query control information per parameter without scanning the whole graph.

```rust
struct UiParamControlInfo {
    param: ParamAddr,
    active_mode: ControlMode,
    available_modes: Vec<ControlMode>,
    status: ControlStatus,
    diagnostics: Vec<ControlDiagnostic>,
    context_candidates: Vec<UiContextCandidate>,
    token_suggestions: Vec<UiTokenSuggestion>,
    proxy_candidates: Vec<UiParamCandidate>,
    binding_candidates: Vec<UiParamCandidate>,
}
```

`context_candidates` should include:

- symbol
- type
- scope owner node
- lexical depth
- compatibility score / exact-compatibility flag

`token_suggestions` should include symbols and metadata fields (`$name`, `$type`, `$id`, ...).

### 7.1 Query APIs

Recommended engine APIs:

```rust
fn ui_param_control_info(param: ParamAddr) -> UiParamControlInfo;
fn ui_context_candidates(param: ParamAddr, expected: ValueType) -> Vec<UiContextCandidate>;
fn ui_token_suggestions(param: ParamAddr, cursor: usize) -> Vec<UiTokenSuggestion>;
```

### 7.2 Caching and invalidation

Cache key should include generation counters:

- graph structure generation
- relevant context-scope generation
- control-schema generation

No UI request should trigger full-graph recomputation.

---

## 8. Integration with Contexts

Control modes that reference external values must use the context endpoint layer from `node_contexts.md`.

This includes:

- context link mode
- template tokens
- expression inputs
- proxy target metadata projections (when needed)

One resolver path keeps behavior consistent across modes.

---

## 9. Integration with Blueprints / Models

Control state is part of field behavior and must be model-aware.

Recommended keying:

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

Then override/propagation can treat value and control independently:

- model changes to value default propagate to non-overridden `Value`
- model changes to control default propagate to non-overridden `Control`
- instance-level customization can override either aspect

This avoids conflating "value override" with "control setup override".

---

## 10. Determinism and Safety

- no per-tick compilation
- binding cycles detected and rejected
- proxy cycles detected and rejected
- stable update ordering for bidirectional systems (binding mediator)
- unresolved sources degrade to explicit diagnostics, not silent fallback

---

## 11. Implementation Order (Recommended)

1. `ParamControlState` + `ControlMode` persistence schema
2. shared compiled control plan and resolver interfaces
3. context link mode (`EndpointRef` reuse)
4. template tokens and expression mode
5. proxy mode
6. binding mode with loop prevention
7. animation mode in the same pipeline
8. UI control info APIs + caches + invalidation
9. model/blueprint `FieldAspect` override integration
