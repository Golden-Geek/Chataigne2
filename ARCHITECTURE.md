# Architecture

The workspace is split by ownership: reusable Golden foundations live under `crates/` and
`packages/`; Chataigne product behavior lives under `apps/chataigne/`. The planned sibling
video mapping app, MapGyver, owns its product behavior under `apps/mapgyver/` and consumes the
same public Golden contracts. See [MapGyver architecture](docs/architecture/mapgyver.md).

Chataigne's current implementation and qualification limits are in its
[implementation plan](docs/plan/Chataigne2_Codex_Implementation_Plan.md) and
[status ledger](docs/progress/implementation-status.md); architecture references do not carry
independent task lists.
MapGyver has a separately selected [plan](docs/plan/MapGyver_Implementation_Plan.md) and
[ledger](docs/progress/mapgyver-status.md). Its proposed runtime and editor paths are design,
not implemented workspace members.

## Repository layers

### Golden Core

`crates/golden_core/` is the reusable backend framework and the public `golden_core` facade.
Its internal workspace crates are grouped by role:

- `engine/`: node tree, edit loop, scheduling, and UI read model.
- `foundation/`: application contracts, identities, values, parameters, and contexts.
- `runtime/`: control runtime, bounded I/O workers, readiness/deadline notifications,
  payload retirement, and scripting.
- `services/`: persistence and protocol boundaries.
- `hosts/`: desktop and transport hosts.
- `support/`: macros and code generation.

The engine edit queue applies an explicit prefix on failure and always finalizes committed
history, references, contexts, and publication. Structural mutation and fallible lifecycle
completion are distinct: a committed insertion remains undoable if its callback fails.
The shared ordered edit journal also retains nested lifecycle changes and complete authored
callback effects. Callback history captures declared persisted state, including defaults, while
preserving runtime-only resources; replay restores recorded values without invoking the callback.
Compound user intents own their validation and atomic commit/rollback above this queue;
grouping edits in a history session alone does not make them atomic. Prepared subtree rollback
uses an ordered journal for structured lifecycle edits to the existing tree, together with
candidate cleanup, captured parameters/listeners, project dirty tracking, and publication state.
`engine/transaction.rs` owns the outer checkpoint shared by compound UI operations and nested
prepared-subtree operations; a rejection preserves any enclosing session's accepted history.
Device effects already dispatched by lifecycle callbacks cannot be rolled back by restoring
authored state.

Catalog creation uses the prepared-subtree boundary in `engine/src/engine/persistence/`:
detached structure and available initializer values are validated first. Fields that depend on
the live parent and reference targets are validated in a provisional subtree before ready,
publication, and history commit, behind one rollback checkpoint.
`persistence/lifecycle.rs` owns reconstruction and ready ordering; explicit initializers win over
init defaults before ready callbacks configure resources. Detached staging never trusts cached
runtime reference ids from the live graph.

History replay lives in `engine/history/`: `atomic.rs` retains exact live scalar state and
reversible payload transitions around the batched replay paths. Rejection restores publication
and indexes through the shared edit checkpoint and keeps the transaction on its original stack.
Restoration uses `NodeCreationContext::HistoryRestore` for ready activation. Runtime callbacks
still run, but authored self-state is preserved and immediate callback edits are limited to
subscriptions. Replay notifications settle inside this boundary; earlier pending work keeps its
ordinary dispatch semantics.

`engine/runtime/` owns the revision of each successfully resolved schedule. Production compiler
snapshots and installed generations retain that same revision. `runtime_center/compilation.rs`
owns admission, completion fencing, and generation installation; compiler-request sequence and
project epochs have separate purposes. UI admission and runtime ticks request compilation from
engine schedule changes. While compilation is pending, tick ordering uses the engine's current
resolved dependency order. Stale generations cannot append newly scheduled prerequisites after
their dependents, and stale completions cannot install a replaced schedule.

The production facade reports actor admission/completion failures through `ApplicationError`.
`application/transactions.rs` separates admitted batches from authoritative receipts, so transports
can publish `Accepted` only after admission. `ControlErrorKind::Cancelled` means queued work never
started; loss of a started operation's completion remains a disconnection. HTTP and WebSocket
share Rust-owned control error categories in protocol 0.7.0, generated into Golden UI bindings.
`ui_server/http.rs`, `hub.rs`, `delivery.rs`, and `websocket.rs` own the transport paths;
`application_error.rs` maps application causes into wire errors. Clients keep those categories in
`ControlRequestError` and receive an engine acknowledgement only when one actually exists.

Applications normally depend on the facade at `crates/golden_core/`, not on those implementation
crates directly.

### Golden Audio

`crates/golden_audio/` is the reusable, app-agnostic audio engine. It owns device discovery,
backend supervision, render plans, routing, playback, analysis, observations, and deterministic
null/mock/offline qualification. Native backends and codec/DSP dependencies stay private.

Chataigne's persistent Sound Card model, defaults, commands, scripts, and product policy live under
`apps/chataigne/src/module/modules/audio/sound_card/`. Reusable device/status UI and Rust-generated
audio contracts live in `packages/golden-audio-ui/`. See
[Golden Audio Architecture](docs/architecture/golden-audio.md) for the realtime and recovery
boundaries.

### Golden Graph

`crates/golden_graph/` is the app-agnostic graph document and transaction system.
`packages/golden-graph-ui/` is its reusable canvas. Neither package knows about Alchemist,
conditions, processors, or Chataigne state machines.

### MapGyver and proposed Golden Video

`apps/mapgyver/` will own Media/Screen managers, composition authoring, Matrix column/app policy,
projection surfaces and four specialized editors. Proposed `crates/golden_video/` owns reusable
GPU resources, render passes, Spout and output/recovery mechanics, following Golden Audio's
independent runtime boundary. Proposed `packages/golden-video-ui/` owns native preview bindings;
the shared Golden desktop host owns native window/WebView integration. None of these reusable
boundaries imports MapGyver policy. See [native rendering research](docs/architecture/mapgyver-native-rendering.md)
for the D3D11-first recommendation and transparent-WebView feasibility gate.

### Chataigne systems

`apps/chataigne/systems/alchemist/` owns the whole Alchemist processing domain: formulas, ANodes,
conditions, processors, inputs, filters, outputs, lane/value-set execution, and their reusable
Chataigne-owned kernels. Engine-node integration for those concepts is grouped in
`apps/chataigne/systems/alchemist/integration/`.

`apps/chataigne/systems/state_machine/` owns Chataigne's state/transition model, runtime,
arbitration, and generated protocol. Its engine-node integration is grouped in
`apps/chataigne/systems/state_machine/integration/`. It consumes Alchemist processors; it does not define
a second condition or processing system.

The dependency direction is:

`golden_core / golden_graph → Alchemist → Chataigne state machine → app shell`.

### UI

`packages/golden-ui/` provides the reusable workbench and `packages/golden-graph-ui/` provides
the generic graph canvas. Chataigne Formula and State Machine UI, including the state-machine
document projection, stays under `apps/chataigne/ui/src/lib/systems/` in matching Alchemist and
state-machine folders.

Rust owns transport DTOs and generates TypeScript bindings. UI-local adapters may normalize those
bindings but must not duplicate the protocol.

### App shell and hosts

`apps/chataigne/src/app/` only composes product nodes, lifecycle hooks, and the default project.
Desktop startup, browser/headless serving, native dialogs, transports, and persistence remain
provided by Golden Core.

## Where to continue

The intended media boundary keeps authored state, validation, history, and coordination in the
control engine. Audio rendering, future video/GPU execution, and lighting output use specialized
runtimes with shared time, resource identity, and generation contracts. App-owned show transport,
fixture/cue policy and Chataigne workflows belong under Chataigne systems/UI. MapGyver's media,
projection and Matrix models/workflows belong under its own systems/UI.
Generic processing facilities belong in reusable packages once exercised contracts justify them.
Production show-time coordination and project asset preparation remain readiness work; this
description does not claim a video compositor or projection editor already exists.

Use [the documentation index](docs/README.md) to choose a focused architecture page, guide,
operations runbook, or reference map. The exact filesystem map is in
[Repository Layout](docs/reference/repository-layout.md).
