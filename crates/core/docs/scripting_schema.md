# Scripting Schema (QuickJS)

Status: Draft v0.5
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
}

pub enum ScriptSource {
    Inline(String),
    ProjectFile(String),
}
```

Runtime behavior:

1. Runtime is auto-detected from file extension for file-backed scripts.
2. Inline scripts always run on QuickJS.
3. Script update rate is configured via `script.setUpdateRateHz(...)` (materialized as `update_rate_hz` in runtime state).
4. Auto-reload is always enabled.
5. Script execution enable/disable uses `node.meta.enabled` (no script-level enabled flag).

## 5. Template System

Script creation uses template files under:

`crates/core/src/script/templates/`

Template selection order when creating a script under host node type `X`:

1. `templates/x.{js|mjs|cjs}`
2. `templates/<normalized_x>.{js|mjs|cjs}`
3. `templates/default.{js|mjs|cjs}`

Include injection syntax:

`{{include:relative/path.js}}`

Rules:

1. Include paths are relative to `templates/`.
2. Absolute and parent (`..`) paths are rejected.
3. Include cycles are detected and rejected.

Default script nodes are initialized with inline source generated from the selected template.

## 6. Manifest Schema

Scripts are plain top-level JavaScript programs.
No `return { ... }` manifest object is required.
An empty script is valid.

```js
script.setApiVersion(1);
script.setUpdateRateHz(60);
script.addParameter("gain", { type: "float", default: 1.0 });
script.listen("@host", 2);
export function ping(value) { return value; }

function init() {}
function update(dt) {}
function event(ev) {}
function paramChanged(ev) {}
function destroy() {}
```

Hook functions are optional. Runtime detects and invokes only the hooks that exist.

Runtime method surface:

1. `script.setApiVersion(number)`
2. `script.setUpdateRateHz(number | null)`
3. `script.listen(nodeSelector, maxDepth)`
4. `script.unlisten(nodeSelector, maxDepth)`
5. `script.clearListeners()`
6. `script.addParameter(name, spec)`
7. `script.removeParameter(name)`

Global host helpers:

1. `log(message)` (info level)
2. `success(message)`
3. `warn(message)`
4. `error(message)`
5. `emit(topic, payload)`

Host-side parsed schema:

```rust
pub struct ScriptManifest {
    pub api_version: u32,
    pub update_rate_hz: Option<u32>,
    pub parameters: Vec<ScriptParameterSpec>,
    pub subscriptions: Vec<ScriptSubscriptionSpec>,
    pub exports: Vec<ScriptExportSpec>,
}
```

No UI contribution schema is part of this runtime manifest snapshot.

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

Scripts use global host helper functions (`log`, `success`, `warn`, `error`, `emit`).
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
2. standard node metadata and generated parameter children

Runtime VM state and live counters are runtime-only and never persisted.

## 12. Canonical Reference

Use this file as the canonical scripting reference unless superseded by a newer version in the same location.
