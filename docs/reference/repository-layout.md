# Repository Layout

The root `Cargo.toml`, `Cargo.lock`, `package.json`, and `package-lock.json` define one Rust
and npm workspace. `apps/chataigne/` and `apps/mapgyver/` are sibling product shells using the
shared Golden runtime and workbench. Root Cargo defaults and unqualified app commands continue
to select Chataigne; MapGyver has explicit run, watch, UI and package commands.

## Backend

`crates/golden_audio/` is a sibling reusable foundation: it owns the app-agnostic audio engine,
native host adapters, playback, analysis, and qualification harnesses. It does not depend on
Golden Core or Chataigne.

```text
crates/
├── golden_core/                 public facade and reusable backend framework
│   ├── engine/                  node tree, engine loop, scheduling, UI read model
│   ├── foundation/
│   │   ├── application/         application contracts
│   │   ├── context/             context declarations and state
│   │   ├── model/               cross-layer identities
│   │   ├── parameters/          parameter contracts and projections
│   │   └── values/              canonical runtime values
│   ├── runtime/
│   │   ├── control/             compiled control/runtime primitives
│   │   ├── io/                  workers, queues, and recovery
│   │   └── script/              generic scripting integration
│   ├── services/
│   │   ├── persistence/         project/file persistence
│   │   └── protocol/            public UI protocol
│   ├── hosts/
│   │   ├── desktop/             Tauri and native dialogs
│   │   └── transport/           HTTP/WebSocket host
│   └── support/
│       ├── codegen/             supported build/codegen API
│       └── macros/              proc macros
├── golden_graph/                app-agnostic graph documents and transactions
├── golden_time/                 shared coordinates, rational mapping and time/activation authority
└── golden_video/                compiler, bounded worker/null runtime and optional Windows D3D11/output backend
```

App metadata comes from the engine's pure `ProjectLifecycle` hooks; shared desktop listener and
frontend defaults come from `DesktopHostConfig`. The desktop host's `native_presentation/`
owns the optional typed UI/worker/retirement boundary, while `desktop_window.rs` owns Tauri
window construction and its event-loop adapter. `services/protocol/` owns discovery and UI DTOs,
generated into `packages/golden-ui/generated/rust_protocol/`; the current protocol is `0.13.0`
and the discovery route is `/.well-known/golden-ui`. `services/persistence/` owns the required
application-ID envelope for full app projects, while generic graph, subtree and Preferences
formats stay independent. See [Application Contracts](../architecture/application-contracts.md)
for rejection, restoration and host ownership rules.

## Chataigne

```text
apps/chataigne/
├── src/
│   ├── app/                     thin lifecycle and composition shell
│   └── module/                  concrete product modules
├── tests/
│   └── samples/                 crate-level project fixtures
├── systems/
│   ├── alchemist/
│   │   ├── src/                 Formula/ANode graph, compiler, and runtime
│   │   ├── condition/           condition authoring/compiler/runtime
│   │   ├── processor/           Inputs, Filters, Outputs, lanes, and ValueSet
│   │   └── integration/         Golden Core nodes for the Alchemist surface
│   └── state_machine/
│       ├── model/               graph-backed state/transition model
│       ├── runtime/             execution, arbitration, and protocol
│       └── integration/         Golden Core state/transition nodes
├── resources/
│   └── formulas/builtin/        shipped Action and Mapping formula assets
└── ui/                          Svelte 5 product UI
```

`apps/chataigne/build.rs` generates the app node registry and embeds bundled UI/formula assets.
Desktop capabilities, icons, permissions, and Tauri configuration remain directly under the app.
The persistent Sound Card model and product policy live under
`apps/chataigne/src/module/modules/audio/sound_card/`; reusable device/status presentation lives in
`packages/golden-audio-ui/`.

Within every Rust crate and UI source area, unit tests live in a local `tests/` directory directly
under the feature they exercise. A crate's top-level `tests/` directory is reserved for crate-wide
integration tests and their fixtures. See [Development Workflows](../guides/development.md#test-placement)
for the exact convention.

## UI packages

- `packages/golden-ui/`: reusable workbench, stores, transport adapters, and generated Rust
  protocol bindings.
- `packages/golden-audio-ui/`: reusable audio device selection/status UI and generated
  `golden_audio` contracts.
- `packages/golden-graph-ui/`: domain-neutral graph canvas.
- `apps/chataigne/ui/src/lib/systems/alchemist/`: Formula, condition, processor, input, filter, and
  output UI.
- `apps/chataigne/ui/src/lib/systems/state_machine/`: State Machine panel, local graph projection,
  and generated protocol bundle.

## MapGyver

`apps/mapgyver/` is an existing Rust/npm workspace member. Its `src/app/` declares the product
identity, `.mapgyver` project specification and shared-host defaults; new projects contain the
standard Dashboard and Media Manager authoring container. Its `ui/` mounts the reusable workbench
with MapGyver branding and a typed media-intent adapter. App-owned
Tauri configuration, icons, permissions and build script remain under that directory; UI bundling
uses the public `golden_codegen_support::ui_bundle` boundary shared with Chataigne.

MapGyver defaults to backend `127.0.0.1:7011` and development UI `127.0.0.1:5174`, separate from
Chataigne's 7010/5173. Golden UI validates app discovery before restoring app-scoped browser
state, and obtains project-file labels/extensions from the required runtime snapshot metadata.

These boundaries distinguish implemented foundations from planned product features:

- `apps/mapgyver/src/media/`: persistent media/channel nodes, typed references, backend operations,
  source/size resolution and Rust-owned generated contracts; live render binding remains open.
- `apps/mapgyver/src/{screen,matrix}/`: planned app-owned models, operations and integration.
- `apps/mapgyver/ui/`: future Screen, Surface Source, Composition and Matrix editors.
- `crates/golden_video/`: compiler, worker/null runtime and optional D3D11 compositor with
  fixed window/Spout outputs and bounded Spout source workers exist. Complete app integration/qualification
  continue in M03, using the shared `crates/golden_time/` authority.
- `packages/golden-video-ui/`: future reusable native-preview bindings; host integration stays shared.

The optional typed native-presentation host lifecycle is implemented and exercised in M02.
Complete renderer integration and native qualification retain their M03/M05 owners. The
owned `native_video_smoke` executable under `apps/mapgyver/tests/native_video/` for the first
real D3D11/runtime/host consumer. The main app still uses the thin workbench. See the
[architecture](../architecture/mapgyver.md), [plan](../plan/MapGyver_Implementation_Plan.md) and
[current acceptance](../progress/mapgyver-status.md).

## Documentation and tooling

- `docs/architecture/`: ownership and dependency design.
- `docs/guides/`: contributor workflows.
- `docs/operations/`: performance, release, troubleshooting, and hygiene runbooks.
- `docs/reference/`: exact maps and policies.
- `docs/plan/`: product implementation plans; root `AGENTS.md` selects Chataigne by default and
  MapGyver only when explicitly requested.
- `docs/progress/`: current product ledgers and qualification evidence.
- `docs/archive/`: non-operative historical plans and qualification evidence.
- `tools/`: bootstrap, checks, qualification, product gates, and release tooling.
- `xtask/`: root watch orchestration.
