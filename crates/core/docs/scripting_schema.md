# Scripting Schema (Multi-runtime)

Status: Draft v0.1  
Applies to: `golden_core` runtime and UI sync layers  
Purpose: Canonical reference for script integration design and implementation

## 1. Goals

1. Support fast runtime scripting with frequent host calls.
2. Keep script integration fully compatible with the existing node/edit/event model.
3. Keep data exchange lightweight and typed in hot paths.
4. Allow context-local scripting with clear scope boundaries.
5. Expose script exports to Rust and host APIs to scripts.
6. Allow scripts to define parameters and receive parameter/structure events.
7. Allow controlled UI contributions.
8. Enforce strong safety limits.

## 2. Core Architecture

1. Scripts are first-class nodes (`script` node type).
2. Script execution is handled by a centralized runtime service, not by ad-hoc per-node engines.
3. Scriptable host nodes opt in explicitly via policy.
4. Script parameters are materialized as regular `Parameter` child nodes.
5. Script-produced mutations always go through existing `Edit` queue semantics.

## 3. Node Integration Model

### 3.1 Script Host Policy

Each node type may expose script hosting capabilities with a policy contract.

```rust
pub struct ScriptHostPolicy {
    pub enabled: bool,
    pub max_scripts: u16,
    pub script_root_mode: ScriptRootMode,
    pub capabilities: ScriptCapabilitySet,
    pub allow_structural_mutation: bool,
    pub allow_ui_contributions: bool,
}

pub enum ScriptRootMode {
    EngineRoot,
    HostNode,
    RelativeDeclPath(Vec<String>),
}
```

Nodes that do not expose this policy are not scriptable.

### 3.2 Tree Layout

Preferred layout:

1. `HostNode`
2. `HostNode / script`
3. `HostNode / script / params / <generated_parameter_nodes>`

`ScriptManager` can exist as an optional grouping node, but it is lazy and never mandatory.

## 4. Script Node Schema

`script` is a standard runtime node that stores script configuration and runtime status.

```rust
pub struct ScriptNodeConfig {
    pub source: ScriptSource,
    pub runtime_hint: Option<ScriptRuntimeKind>,
    pub auto_reload: bool,
    pub enabled: bool,
    pub requested_update_rate_hz: Option<u32>,
}

pub enum ScriptSource {
    Inline(String),
    ProjectFile(String), // project-relative path
}
```

Runtime selection rules:

1. File-backed sources choose runtime by extension (`.lua/.luau` => Luau, `.js/.mjs/.cjs` => QuickJS).
2. `runtime_hint` is used when extension does not resolve or for inline source.

Project file paths must be relative to the project root.

## 5. Manifest Schema

Each script must return a manifest table at load time.

```lua
return {
  api_version = 1,
  update_rate_hz = 60,
  parameters = { ... },
  subscriptions = { ... },
  exports = { ... },
  ui = { ... },
  on_init = function(ctx) end,
  on_update = function(ctx, dt) end,
  on_event = function(ctx, ev) end,
  on_destroy = function(ctx) end
}
```

Host-side parsed schema:

```rust
pub struct ScriptManifest {
    pub api_version: u32,
    pub update_rate_hz: Option<u32>,
    pub parameters: Vec<ScriptParameterSpec>,
    pub subscriptions: Vec<ScriptSubscriptionSpec>,
    pub exports: Vec<ScriptExportSpec>,
    pub ui: ScriptUiSpec,
    pub requested_capabilities: ScriptCapabilitySet,
}
```

## 6. Parameter Definition Schema

Script-defined parameters map to real `Parameter` nodes and existing `ParamValue` and `ParameterConstraints`.

```rust
pub struct ScriptParameterSpec {
    pub name: String,                 // stable id used in script API
    pub label: Option<String>,
    pub value_type: ScriptValueType,  // trigger/int/float/str/file/enum/bool/vec2/vec3/color/reference
    pub default_value: crate::parameter::ParamValue,
    pub read_only: bool,
    pub constraints: crate::parameter::ParameterConstraints,
    pub ui_hints: crate::parameter::ParameterUiHints,
}
```

Rules:

1. Parameter names are unique per script.
2. Manifest diff on reload adds/removes/patches generated parameter nodes.
3. Existing parameter values are retained on compatible schema updates.
4. Incompatible schema changes fall back to default value.
5. `file` parameters can declare:
   - `allowed_types`: `["audio", "video", "script"]`
   - `allowed_extensions`: explicit extension allow-list like `["wav", ".mp3"]`

## 7. Event and Subscription Schema

Scripts receive events through the same engine event model.

```rust
pub struct ScriptSubscriptionSpec {
    pub node: ScriptNodeSelector,
    pub max_depth: u32,
}
```

`ScriptNodeSelector` supports:

1. Absolute `NodeId` (runtime-only use).
2. Decl-path relative to script root (`"."`, `".."`, `"foo/bar"`).
3. Special anchors (`@host`, `@root`).

Subscriptions compile into existing `EventSubscription` registrations.

## 8. Exports and Rust Calls

Scripts can declare callable exports.

```rust
pub struct ScriptExportSpec {
    pub name: String,
    pub signature: ScriptFnSignature,
}
```

Rust call contract:

```rust
pub fn call_script_export(
    script_node: NodeId,
    export_name: &str,
    args: &[ScriptValue],
) -> Result<ScriptValue, ScriptCallError>;
```

Export lookup is resolved from the validated manifest. Missing exports are a typed error.

## 9. Host API for Scripts

Scripts call host functions through a capability-gated `ctx`.

Core API groups:

1. `ctx.param_get(name_or_path)`
2. `ctx.param_set(name_or_path, value)`
3. `ctx.node_find(path)`
4. `ctx.node_add(spec)`
5. `ctx.node_remove(path_or_id)`
6. `ctx.node_move(path_or_id, new_parent)`
7. `ctx.node_patch_meta(path_or_id, patch)`
8. `ctx.subscribe(selector, max_depth)`
9. `ctx.emit_custom(topic, payload)`
10. `ctx.log(level, message)`
11. `ctx.ui_publish(contribution)`

All mutation APIs enqueue edits and never mutate engine state directly.

## 10. UI Contribution Schema

Scripts can submit declarative UI contributions only.

```rust
pub struct ScriptUiSpec {
    pub menus: Vec<ScriptMenuContribution>,
    pub panels: Vec<ScriptPanelContribution>,
    pub drawings: Vec<ScriptDrawContribution>,
}
```

Rules:

1. No arbitrary JS execution in UI.
2. UI payloads are validated and size-limited.
3. UI actions route back through explicit intents/events.

## 11. Safety Schema

Each script instance has hard budgets.

```rust
pub struct ScriptBudgets {
    pub max_instructions_per_callback: u64,
    pub max_wall_time_us_per_callback: u64,
    pub max_memory_bytes: usize,
    pub max_host_calls_per_callback: u32,
    pub max_emitted_edits_per_tick: u32,
    pub max_emitted_events_per_tick: u32,
    pub max_ui_payload_bytes_per_tick: usize,
}
```

Enforcement:

1. Budget violations abort current callback.
2. Repeated violations mark script as faulted and disabled.
3. Fault state is surfaced as node warning and logger record.
4. Capability violations are hard errors and never partially apply.

## 12. Hot Reload Schema

Reload flow:

1. Detect source change and mark instance dirty.
2. Reload at tick boundary.
3. Compile and validate manifest.
4. Diff parameter schema and apply edits.
5. Swap runtime instance atomically.

Failure flow:

1. Keep previous valid instance running.
2. Attach warning to script node.
3. Emit structured error log.

Optional migration hooks:

1. `on_serialize_state() -> table`
2. `on_restore_state(state)`

## 13. Persistence Schema

Persisted fields on script nodes:

1. `source` (`Inline` or `ProjectFile`)
2. `auto_reload`
3. script-level metadata (label, tags, policy overrides where allowed)
4. generated parameter child nodes as standard persisted nodes

Runtime-only fields are not persisted:

1. compiled bytecode
2. runtime VM handles
3. instruction counters and live perf stats

## 14. Scope and Path Semantics

Canonical path format uses existing decl-path style:

1. `.` current script node
2. `..` parent
3. `foo/bar` decl-id chain
4. `@host/...` from script host node
5. `@root/...` from resolved script root

This aligns with UI "Script Control Path" behavior and keeps references context-local.

## 15. Versioning Rules

1. `api_version` in manifest is mandatory.
2. Runtime supports a declared list of schema versions.
3. Unsupported versions fail validation with explicit error.
4. Backward compatibility is optional during current foundation phase and can be broken when architecture improves.

## 16. Implementation Phases

1. Phase 1: Load/validate Luau+QuickJS manifest, params materialization, update/event callbacks, safety limits.
2. Phase 2: Rust-to-script exports and script subscriptions.
3. Phase 3: Graph mutation APIs and stricter capability matrix.
4. Phase 4: Declarative UI contributions.
5. Phase 5: Performance tuning and profiling instrumentation.

## 17. Canonical Reference

When implementing scripting features, use this document as the primary source of truth unless superseded by a newer schema revision in the same location.
