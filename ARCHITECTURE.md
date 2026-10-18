# Architecture

Chataigne2 is split by ownership: reusable Golden foundations live under `crates/` and
`packages/`; product behavior lives under `apps/chataigne/`.

The product direction combines control, lighting, video/projection, and audio in one application.
Current implementation and qualification limits are in the single
[implementation plan](docs/plan/Chataigne2_Codex_Implementation_Plan.md) and
[status ledger](docs/progress/implementation-status.md); architecture references do not carry
independent task lists.

## Repository layers

### Golden Core

`crates/golden_core/` is the reusable backend framework and the public `golden_core` facade.
Its internal workspace crates are grouped by role:

- `engine/`: node tree, edit loop, scheduling, and UI read model.
- `foundation/`: application contracts, identities, values, parameters, and contexts.
- `runtime/`: control runtime, I/O workers, and scripting.
- `services/`: persistence and protocol boundaries.
- `hosts/`: desktop and transport hosts.
- `support/`: macros and code generation.

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
fixture/cue policy, projection models, and creative workflows belong under Chataigne systems/UI.
Generic processing facilities belong in reusable packages once exercised contracts justify them.
Production show-time coordination and project asset preparation remain readiness work; this
description does not claim a video compositor or projection editor already exists.

Use [the documentation index](docs/README.md) to choose a focused architecture page, guide,
operations runbook, or reference map. The exact filesystem map is in
[Repository Layout](docs/reference/repository-layout.md).
