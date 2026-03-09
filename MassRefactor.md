This is a mass refactor to clean and make the overall architecture better.
Read this, and start by rewriting AGENTS.md to incorporate the rules that will help you throughout all tasks current and future. 
Then get to it !

## Target end state

You want a repo where:

* `Chataigne2` stays a thin app shell.
* `golden_core` is actually “core,” not “core + desktop host + file dialogs + transport server.”
* Rust and TypeScript do not manually mirror the same protocol types.
* UI state is split into small Svelte runes stores with one façade, not a 2k-line orchestration file.
* build/codegen boundaries are public and stable instead of path-importing internal files.
* docs tell a new dev where to start in 10 minutes instead of forcing them to reverse-engineer the tree. The current structure already hints at that direction: engine internals are reasonably decomposed, while the main friction is boundary leakage and readability debt. ([GitHub][2])

---

## Phase 0 — Put guardrails around readability first

This is the first thing I would land because it improves every later diff.

Right now the repo is carrying extremely wide formatting via `rustfmt.toml` (`max_width = 300`), and several key files are effectively committed in one-line or near-one-line form, including the app `Cargo.toml`, `build.rs`, `golden_core` `Cargo.toml`, `lib.rs`, `ui_sync.rs`, `desktop.rs`, and `golden_ui` protocol/store files. That makes review, blame, and onboarding much harder than necessary. ([GitHub][3])

### Tasks

1. Reduce Rust line width to something sane, ideally `100` or `120`.
2. Reformat the whole Rust tree in one dedicated PR.
3. Reformat `golden_ui` TypeScript/Svelte files consistently with Prettier.
4. Add a root `CONTRIBUTING.md` with:

   * repo map
   * formatting commands
   * “do not import submodule internals by path”
   * “do not duplicate protocol declarations”
5. Add a root `ARCHITECTURE.md` with one page explaining:

   * app shell
   * core engine
   * UI protocol
   * UI client/store layers
   * desktop/browser host split

### Acceptance criteria

* No new one-line source files.
* `cargo fmt --all` and UI formatting pass locally.
* New contributors can identify the top-level layers from docs without opening implementation files.

### Codex/Copilot prompt

```text
Refactor goal: establish repo-wide readability guardrails without changing runtime behavior.

Scope:
- Do not touch src/main.rs.
- Reduce Rust formatting width from 300 to a sane value.
- Reformat Rust and TypeScript/Svelte sources consistently.
- Add CONTRIBUTING.md and ARCHITECTURE.md at repo root.
- Document: app shell, golden_core, golden_ui, protocol boundary, build/codegen rule.

Constraints:
- No behavior changes.
- No compatibility shims.
- Keep docs concise and architecture-focused.
```

---

## Phase 1 — Remove the private build/codegen dependency leak

The current app `build.rs` imports `submodules/golden_core/crates/core/node/node_codegen.rs` directly, while `golden_core` also has its own `crates/core/build.rs`, which is currently empty. That is a brittle boundary: app code is reaching into a submodule’s private filesystem instead of consuming a supported interface. ([GitHub][4])

### Tasks

1. Move node codegen behind a supported boundary in `golden_core`.
2. Pick one of these designs:

   * **Preferred:** create a small workspace crate like `golden_codegen_support`.
   * **Acceptable:** expose codegen through a dedicated public module/crate specifically meant for build scripts.
3. Update `Chataigne2/build.rs` to consume the public API only.
4. Ban `#[path = "...submodule internal file..."]` imports from app crates.

### Suggested destination

* New crate: `golden_core/crates/codegen_support`
* Public API example:

  * `generate_app_nodes(src_root: &Path, out_file: &Path) -> Result<()>`

### Acceptance criteria

* `Chataigne2/build.rs` no longer references internal submodule paths.
* `node_codegen.rs` can move internally without breaking app build scripts.
* One public entrypoint owns code generation.

### Codex/Copilot prompt

```text
Refactor goal: eliminate the app build.rs dependency on golden_core private file paths.

Scope:
- Do not touch src/main.rs.
- Introduce a supported public codegen boundary in golden_core.
- Update Chataigne2/build.rs to use that boundary.
- Remove #[path = ".../node_codegen.rs"] from the app.

Constraints:
- Preserve current generated output behavior.
- Prefer a dedicated codegen-support crate over exposing private modules.
- No runtime behavior changes.
```

---

## Phase 2 — Make `golden_core` actually core

Today the `golden_core` crate root exports engine, node, events, app helpers, UI sync DTOs, and scripting, and the crate depends directly on `tauri`, `rquickjs`, and `rfd`. The `app/` directory contains `desktop.rs`, `mod.rs`, and `ui_server.rs`, while the engine itself is already more cleanly segmented into files like `engine_apply`, `engine_runtime`, `engine_history`, `engine_ui`, and `node_store`. That means the repo already has a good internal engine split; the next step is to move host concerns out of the “core” crate. ([GitHub][5])

### Proposed crate split

Keep one workspace, but split responsibilities:

* `golden_engine`

  * engine
  * node
  * events
  * process context
  * blueprints
  * contexts
  * logger
  * animation curve
* `golden_protocol`

  * UI DTOs
  * protocol version
  * transport-agnostic request/response/event types
* `golden_script`

  * QuickJS integration and scripting schema/runtime
* `golden_persistence`

  * project serialization/deserialization types and helpers
* `golden_host_desktop`

  * Tauri window boot
  * native dialogs
  * desktop-only startup glue
* `golden_ui_server` or `golden_transport_server`

  * HTTP/WebSocket server
  * subscription/replay endpoints
* optional façade crate `golden_core`

  * thin re-export layer if you still want a top-level import

### Tasks

1. Extract `app/desktop.rs` into a host crate.
2. Extract `app/ui_server.rs` into a transport/server crate.
3. Move `ui_sync.rs` into `golden_protocol`.
4. Move `persistence/persistence.rs` into `golden_persistence`.
5. Keep engine internals together in the pure engine crate.
6. Remove `tauri` and `rfd` from the pure core/engine dependency graph.
7. Revisit whether `rquickjs` belongs in the core crate or a script crate.

### Acceptance criteria

* A pure engine build has no desktop/UI host dependencies.
* Desktop host code can evolve without polluting engine APIs.
* Browser/headless/server usage can consume protocol + engine without Tauri.

### Codex/Copilot prompt

```text
Refactor goal: split golden_core into clean workspace crates so “core” no longer depends on desktop host concerns.

Scope:
- Do not touch src/main.rs.
- Extract app/desktop.rs and app/ui_server.rs into host/server crates.
- Extract ui_sync.rs into a protocol crate.
- Extract persistence.rs into a persistence crate.
- Keep engine/node/events/process_ctx/logger in a pure engine crate.

Constraints:
- Prefer moving code over rewriting behavior.
- Break compatibility if needed; optimize for architecture.
- Final result must allow a pure engine build without tauri or rfd.
```

---

## Phase 3 — Replace duplicated Rust/TS protocol definitions with one source of truth

This is the most important structural cleanup after crate splitting.

Right now `golden_core/ui/ui_sync.rs` defines the Rust-side UI protocol, while `golden_ui/types.ts` manually defines matching TS unions and DTOs for snapshots, events, intents, logs, history, script state, and subscription scope. That duplication is exactly the kind of drift your `AGENTS.md` says to avoid. There is already a concrete mismatch too: `AGENTS.md` says Svelte sizing should use only relative units, but `golden_ui/types.ts` still includes `CssUnit = 'px' | 'rem' | 'em' | 'percent' | 'vw' | 'vh'`. ([GitHub][6])

### Tasks

1. Choose one source of truth:

   * **Preferred:** Rust DTOs generate TS types.
   * **Alternative:** schema files generate both Rust and TS.
2. Move all protocol DTOs into a dedicated module/crate.
3. Generate:

   * `types.ts`
   * protocol version constant
   * request/response/event DTOs
4. Add a protocol conformance test:

   * Rust snapshot serializes
   * TS client accepts and parses it
5. Remove hand-maintained TS protocol declarations once generation is in place.
6. Resolve the `CssUnit` policy mismatch explicitly:

   * either remove `px`
   * or revise AGENTS/rules if fixed px is actually needed for some canonical UI surfaces

### Acceptance criteria

* No manual duplication of snapshot/event/intent DTOs between Rust and TS.
* Protocol versioning is defined once.
* Any DTO change fails generation/tests until both sides align.

### Codex/Copilot prompt

```text
Refactor goal: eliminate manual duplication between Rust ui_sync DTOs and golden_ui/types.ts.

Scope:
- Do not touch src/main.rs.
- Introduce a single source of truth for the UI protocol.
- Generate TypeScript types from Rust/schema.
- Add protocol conformance tests.
- Remove hand-maintained duplicate TS declarations where possible.

Constraints:
- Preserve current JSON shape unless changing it is architecturally necessary.
- If shape changes, update both generator and consumers in the same PR.
- Resolve the CssUnit policy mismatch explicitly.
```

---

## Phase 4 — Decompose `golden_ui` session/workbench state into focused stores

`golden_ui` already has separate folders for `components`, `dockview`, `store`, `transport`, `utils`, and `style`, and the `store/` folder already contains multiple focused files. But `store/workbench.svelte.ts` is still doing too much at once: transport wiring, snapshot/bootstrap, replay/resync, selection, toasts, logger UI state, history state, footer hover, warning aggregation, edit intent queueing, keyboard command registration, and project file workflow. It is 1,963 lines / 51.4 KB. ([GitHub][7])

### Target split

Keep `createWorkbenchSession()` as the façade, but move internals into focused stores/services:

* `session-connection.svelte.ts`

  * connection state
  * bootstrap/retry
  * replay/resync
* `session-selection.svelte.ts`

  * selected nodes
  * persisted selection restore/save
* `session-history.svelte.ts`

  * undo/redo state
  * edit session lifecycle
* `session-logger.svelte.ts`

  * log buffering
  * UI log frequency
  * toast generation
* `session-warnings.svelte.ts`

  * normalized warning model
  * visible warning cache
* `session-hover.svelte.ts`

  * footer hover stack
  * description resolution
* `session-intents.svelte.ts`

  * batching
  * queue draining
  * ack handling
* `session-project-files.svelte.ts`

  * save/load/reopen hooks
* `session-transport.ts`

  * transport adapter interface

### Transport boundary cleanup

The transport folder is already split into `client-instance.ts`, `http.ts`, and `ws.ts`, but `workbench.svelte.ts` currently imports the websocket client directly. Push it behind an interface so workbench/session logic depends on a generic `UiClientTransport`, not a specific ws implementation. ([GitHub][8])

### Acceptance criteria

* `workbench.svelte.ts` becomes a thin composition façade.
* Each extracted store owns one job.
* Session logic is testable without a real websocket.
* Transport swaps do not require changes in selection/history/logger state code.

### Codex/Copilot prompt

```text
Refactor goal: break golden_ui/store/workbench.svelte.ts into focused Svelte 5 runes stores and a thin façade.

Scope:
- Do not touch src/main.rs.
- Keep createWorkbenchSession() public.
- Extract connection, selection, history, logger, warnings, hover, intents, and project-file concerns into separate files.
- Introduce a transport adapter so session logic does not import websocket implementation directly.

Constraints:
- Preserve behavior and public session API where practical.
- Prefer composition over inheritance or utility god-files.
- Use Svelte 5 runes idiomatically; avoid legacy patterns.
```

---

## Phase 5 — Normalize persistence boundaries

`golden_core` has a `persistence/` directory, but it currently contains only a single `persistence.rs` file, while app-level project codec types are also defined under `app/mod.rs`. That split is not clean: persistence concepts are spread across app/runtime glue and the persistence area itself. ([GitHub][9])

### Tasks

1. Define a clean persistence boundary:

   * persisted graph/project schema
   * import/export API
   * migration/import compatibility if kept
2. Move project codec types out of the app host layer.
3. Split persistence into:

   * `project_schema.rs`
   * `project_codec.rs`
   * `migrations.rs` or `imports.rs`
   * `engine_persistence_bridge.rs` if needed
4. Keep desktop file dialogs and project open/save UI out of persistence.

### Acceptance criteria

* Serialization contracts live in persistence/protocol crates, not host/bootstrap modules.
* App host code only calls persistence APIs.
* Persistence changes do not require touching desktop/window code.

### Codex/Copilot prompt

```text
Refactor goal: cleanly separate persistence concerns from app host/runtime glue.

Scope:
- Do not touch src/main.rs.
- Move project codec types and persistence logic out of app modules into a dedicated persistence area/crate.
- Split persistence into schema, codec, and migration/import modules.

Constraints:
- Preserve existing serialized project behavior unless a cleaner format is explicitly introduced.
- Keep desktop file dialogs out of persistence.
```

---

## Phase 6 — Normalize filesystem/module layout for newcomer legibility

`golden_core/crates/core/src/lib.rs` currently uses `#[path = "../..."]` to wire in modules from sibling directories like `engine/`, `node/`, `script/`, and `ui/`. It works, but it is harder to navigate than a standard `src/` hierarchy because the crate root and filesystem layout do not visually line up. ([GitHub][10])

### Tasks

1. Decide whether to keep one crate per concern or normalize folder layout inside each crate.
2. Preferred outcome:

   * each crate has a conventional `src/` tree
   * internal modules live under that crate’s `src/`
3. Avoid `#[path]` unless there is a compelling generation reason.

### Acceptance criteria

* A new dev can navigate modules from `src/lib.rs` by path intuition.
* IDE “go to file” and symbol ownership become more obvious.
* Crate boundaries are mirrored by filesystem layout.

### Codex/Copilot prompt

```text
Refactor goal: normalize crate filesystem layout so module paths match crate structure.

Scope:
- Do not touch src/main.rs.
- Reduce or eliminate #[path = "../..."] module wiring in golden_core crates.
- Move modules into conventional src/ layouts where practical.

Constraints:
- Favor conventional Rust crate organization.
- Avoid behavior changes.
- Preserve docs/comments while moving files.
```

---

## Phase 7 — Fix docs and onboarding debt

The current docs are uneven. The root `README.md` is only “Chataigne2 comes back.” The `src-ui/README.md` is still mostly the default Svelte starter README. `golden_core/README.md` is also minimal, though `golden_core/crates/core/docs/` does contain useful design docs such as `dashboard_system.md`, `node_blueprints.md`, `node_contexts.md`, `parameters_control_modes.md`, and `scripting_schema.md`. ([GitHub][11])

### Tasks

1. Replace root README with:

   * what Chataigne2 is
   * repo layout
   * how submodules fit
   * where app code ends and framework code begins
2. Replace `src-ui/README.md` with actual UI architecture docs.
3. Expand `golden_core/README.md` to summarize existing deeper docs.
4. Add:

   * `docs/repo-map.md`
   * `docs/adding-a-node.md`
   * `docs/ui-protocol.md`
   * `docs/desktop-vs-browser-host.md`

### Acceptance criteria

* A dev can answer “where do I change X?” from docs.
* Default template docs are gone.
* Existing deep docs are linked from top-level docs instead of hidden.

### Codex/Copilot prompt

```text
Refactor goal: replace placeholder docs with project-specific onboarding docs.

Scope:
- Do not touch src/main.rs.
- Rewrite root README.md and src-ui/README.md.
- Expand golden_core README.md.
- Add repo-map and onboarding docs for nodes, UI protocol, and host layers.

Constraints:
- Keep docs architectural and actionable.
- Link to existing design docs instead of duplicating them.
```

---

## Phase 8 — Reconcile metadata, licensing, and version-policy drift

There is metadata drift that should be fixed deliberately. `golden_core`’s workspace `Cargo.toml` declares `license = "MIT"`, while the GitHub org/repo listing shows `golden_core` as `AGPL-3.0`. `golden_ui`’s repo page shows `GPL-3.0`. `Chataigne2` is on Rust edition 2021 while `golden_core`’s workspace is on edition 2024. These may be intentional, but right now they read as unresolved policy drift rather than deliberate architecture. ([GitHub][12])

### Tasks

1. Choose the actual intended license model for:

   * `Chataigne2`
   * `golden_core`
   * `golden_ui`
2. Make repo metadata, Cargo manifests, and LICENSE files agree.
3. Align Rust editions unless divergence is intentional and documented.
4. Add a short policy note on:

   * breaking changes allowed
   * compatibility expectations
   * generated types policy
   * repo/submodule versioning policy

### Acceptance criteria

* No repo/license ambiguity.
* Edition differences are either removed or documented.
* Contributors do not have to infer policy from conflicting metadata.

### Codex/Copilot prompt

```text
Refactor goal: reconcile repo metadata and policy drift.

Scope:
- Do not touch src/main.rs.
- Align LICENSE files, GitHub metadata, and Cargo.toml license fields.
- Review Rust edition mismatch between app and golden_core.
- Add a short policy doc for compatibility, versioning, and generated code.

Constraints:
- Prefer explicit consistency over silent assumptions.
```

---

## Recommended PR order

Do this as a sequence of small, reviewable PRs instead of one giant rewrite.

1. Phase 0 — formatting + docs scaffolding
2. Phase 1 — build/codegen boundary
3. Phase 2 — crate split skeleton
4. Phase 3 — protocol single-source generation
5. Phase 4 — UI store/workbench decomposition
6. Phase 5 — persistence cleanup
7. Phase 6 — filesystem/module normalization
8. Phase 7 — onboarding docs
9. Phase 8 — metadata/license/version cleanup

That order minimizes churn because formatting and docs reduce review friction first, build/codegen removes a brittle dependency early, then crate boundaries and protocol generation stabilize the architecture before you split UI state aggressively. The current repo layout supports that sequence well: the app shell is thin, the engine is already internally segmented, and the UI already has a store/transport split that can be further refined. ([GitHub][1])

---

## Hard rules I would give Codex/Copilot for every PR

```text
Project rules:
- Do not edit src/main.rs in this task.
- Prefer moving responsibilities to the right layer over adding glue.
- Do not add compatibility shims unless explicitly requested.
- Do not duplicate protocol definitions across Rust and TypeScript.
- Do not import private submodule files by filesystem path.
- Keep Chataigne2 thin; put reusable logic in golden_core/golden_ui workspaces.
- Preserve behavior unless the refactor explicitly targets design cleanup.
- Leave the tree cleaner than you found it.
```

---

## Best single starting ticket

If you want one first ticket to hand to Codex right now, make it this:

```text
Title: Eliminate private build/codegen coupling and establish repo guardrails

Deliverables:
1. Reformat Rust/TS code to sane widths and consistent style.
2. Add CONTRIBUTING.md + ARCHITECTURE.md.
3. Replace app build.rs private import of golden_core/node_codegen.rs with a supported public codegen boundary.
4. Do not touch src/main.rs.

Definition of done:
- No private #[path] import from Chataigne2 into golden_core internals.
- rustfmt/prettier configs are sane and applied.
- Root docs explain repo layers and contribution rules.
```

That gives you immediate payoff without waiting for the larger crate split. It also makes every later Codex run safer and easier to review. ([GitHub][4])

I can also turn this into a **copy-paste issue backlog** with one GitHub issue per phase, each with checklist items and “files likely touched.”

[1]: https://github.com/Golden-Geek/Chataigne2/tree/main/src
[2]: https://github.com/Golden-Geek/golden_core/tree/7d42f0e7520a1bd585ccb81a10ed79a269acf31b/crates/core/engine
[3]: https://raw.githubusercontent.com/Golden-Geek/Chataigne2/main/rustfmt.toml
[4]: https://raw.githubusercontent.com/Golden-Geek/Chataigne2/main/build.rs
[5]: https://raw.githubusercontent.com/Golden-Geek/golden_core/7d42f0e7520a1bd585ccb81a10ed79a269acf31b/crates/core/Cargo.toml
[6]: https://raw.githubusercontent.com/Golden-Geek/golden_core/7d42f0e7520a1bd585ccb81a10ed79a269acf31b/crates/core/ui/ui_sync.rs
[7]: https://github.com/Golden-Geek/golden_ui
[8]: https://github.com/Golden-Geek/golden_ui/tree/main/transport
[9]: https://github.com/Golden-Geek/golden_core/tree/7d42f0e7520a1bd585ccb81a10ed79a269acf31b/crates/core/persistence
[10]: https://raw.githubusercontent.com/Golden-Geek/golden_core/7d42f0e7520a1bd585ccb81a10ed79a269acf31b/crates/core/src/lib.rs
[11]: https://raw.githubusercontent.com/Golden-Geek/Chataigne2/main/README.md
[12]: https://raw.githubusercontent.com/Golden-Geek/golden_core/main/Cargo.toml
