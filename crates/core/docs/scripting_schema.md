# Scripting Schema (Multi-runtime)

Status: Draft v0.2
Applies to: `golden_core` runtime and UI sync layers
Purpose: Canonical reference for script integration design and implementation

## 1. Goals

1. Keep script execution fast with lightweight host/script exchanges.
2. Keep script integration aligned with the node/edit/event model.
3. Keep scripting context-local to the host node by default.
4. Expose script exports to Rust and host APIs to scripts.
5. Allow scripts to define parameters and receive events.
6. Enforce strict runtime safety budgets.
7. Keep script UI/plugin extensibility out of this subsystem.

## 2. Core Architecture

1. Scripts are first-class nodes (`script` node type).
2. Scriptable nodes opt in with `ScriptHostPolicy`.
3. Script-defined parameters are regular `Parameter` child nodes.
4. Script mutations go through existing `Edit` queue semantics.

## 3. Script Host Policy

```rust
pub struct ScriptHostPolicy {
    pub enabled: bool,
    pub capabilities: ScriptCapabilitySet,
    pub allow_structural_mutation: bool,
}
```

Notes:

1. No `max_scripts` limit at policy level.
2. No `script_root_mode`; local references resolve from host/local context.
3. No script-driven UI contribution flag in this subsystem.

## 4. Script Node Config

```rust
pub struct ScriptNodeConfig {
    pub source: ScriptSource,
    pub runtime_hint: Option<ScriptRuntimeKind>,
    pub project_root: Option<PathBuf>,
}

pub enum ScriptSource {
    Inline(String),
    ProjectFile(String),
}
```

Runtime behavior:

1. Runtime is auto-detected from file extension for file-backed scripts.
2. `runtime_hint` is optional and only needed when extension cannot select runtime.
3. Inline scripts default to Luau when no hint is provided.
4. Script update rate is declared by manifest `update_rate_hz`.
5. Auto-reload is always enabled.
6. Script execution enable/disable uses `node.meta.enabled` (no script-level enabled flag).

## 5. Template System

Script creation uses template files under:

`crates/core/script/templates/`

Template selection order when creating a script under host node type `X`:

1. `templates/x.{lua|luau|js|mjs|cjs}`
2. `templates/<normalized_x>.{lua|luau|js|mjs|cjs}`
3. `templates/default.{lua|luau|js|mjs|cjs}`

Include injection syntax:

`{{include:relative/path.lua}}`

Rules:

1. Include paths are relative to `templates/`.
2. Absolute and parent (`..`) paths are rejected.
3. Include cycles are detected and rejected.

Default script nodes are initialized with inline source generated from the selected template.

## 6. Manifest Schema

Scripts return a manifest table/object at load time.

```lua
return {
  api_version = 1,
  update_rate_hz = 60,
  parameters = { ... },
  subscriptions = { ... },
  exports = { ... },
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
    pub requested_capabilities: ScriptCapabilitySet,
}
```

No UI contribution schema is part of this manifest.

## 7. Parameter Definition

Script-defined parameters map to regular `Parameter` nodes and existing `ParamValue`/constraints.

File parameter constraints supported:

1. `allowed_types`: `"audio" | "video" | "script"`
2. `allowed_extensions`: explicit extension allow-list

## 8. Events and Subscriptions

```rust
pub struct ScriptSubscriptionSpec {
    pub node: ScriptNodeSelector,
    pub max_depth: u32,
}
```

Supported selectors include path forms and anchors like `@host` and `@root`.

## 9. Host API

Scripts use capability-gated host functions through callback context.
All mutations enqueue edits and never mutate engine state directly.

## 10. Safety

```rust
pub struct ScriptBudgets {
    pub max_instructions_per_callback: u64,
    pub max_wall_time_us_per_callback: u64,
    pub max_memory_bytes: usize,
    pub max_host_calls_per_callback: u32,
    pub max_emitted_edits_per_tick: u32,
    pub max_emitted_events_per_tick: u32,
}
```

Budget violations abort the callback and surface node warnings/logs.

## 11. Persistence

Persisted script fields:

1. `source`
2. `runtime_hint`
3. standard node metadata and generated parameter children

Runtime VM state and live counters are runtime-only and never persisted.

## 12. Canonical Reference

Use this file as the canonical scripting reference unless superseded by a newer version in the same location.
