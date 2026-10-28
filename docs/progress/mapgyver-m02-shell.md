# M02 shell and app isolation

The app owns identity, project policy, UI composition and resources. Golden owns the shared
desktop/headless host, transport, persistence codecs and workbench. The MapGyver shell uses
those public boundaries; it contains no Chataigne modules or product Media/Screen placeholders.
Current acceptance counts belong to the [ledger](mapgyver-status.md#m02-acceptance-items).

## Implementation boundaries

- [MapGyver app](../../apps/mapgyver/src/app/mod.rs): `mapgyver` identity, Cargo version,
  `MapGyver Project`/`.mapgyver`, `MapGyver` data directory and configured shared runtime.
  A new project has the standard Dashboard; the host adds shared Preferences at runtime.
- [Desktop configuration](../../crates/golden_core/hosts/desktop/src/host_config.rs): listener,
  bundled assets and owned development frontend. Operator overrides remain at the host.
  MapGyver defaults to backend `7011`/frontend `5174`; Chataigne keeps `7010`/`5173`.
- [Protocol](../../crates/golden_core/services/protocol/src/types/discovery.rs): Rust-owned
  application/discovery DTOs and generated TypeScript, wire version `0.13.0`. The public
  discovery route is `/.well-known/golden-ui`; there is no Chataigne discovery alias.
- [Application bootstrap](../../packages/golden-ui/components/app/ApplicationRoot.svelte):
  validate the expected app before rendering workbench routes or restoring browser state.
  Fresh credentials/reconnects revalidate identity. Layout, selection, recent path, session
  credentials and client-instance keys include app identity; credentials also include endpoint.
- [Project-file metadata](../../packages/golden-ui/project-file-spec.ts): the snapshot must
  supply its format. There is no UI-selected JSON fallback. File menus and keyboard/API actions
  wait for valid metadata; app/session changes clear stale format state.
- [Full-project codec](../../crates/golden_core/services/persistence/src/application_project_document.rs):
  a required `application_id` envelope around the generic graph document. Validate it before
  sparse expansion, callbacks or live replacement. Runtime save captures retain identity for
  off-thread encoding. Generic graph, subtree and Preferences codecs remain independent.
  Browser uploads validate before resolving or writing their destination, so rejected content
  cannot overwrite an existing own-project file or its backup under a matching filename.
- Stream Deck's specialized panel now belongs to Chataigne's module-editor registry. Golden's
  default workbench has only generic panels. Branding/version/browser-folder labels come from
  backend metadata.

Full application projects without an identity, or with another app's identity, are rejected.
No migration or compatibility fallback is supplied. Only four repository-owned Chataigne
test/qualification fixtures were tagged; user project files were not changed.

The root Prettier configuration is shared by both frontends. Workbench session contracts live
in their own type-only module. Golden's existing project I/O and sparse codecs are split into
focused modules while retaining the public `app::*` APIs.

## Reproduction

```powershell
cargo run -p MapGyver
cargo run -p MapGyver -- --dev
cargo run -p MapGyver -- --headless
cargo xtask watch --app mapgyver
npm run build:mapgyver
npm run check:mapgyver
```

Root commands retain Chataigne as the default. `tools/dev.ps1 -App mapgyver` and
`tools/dev.sh --app mapgyver` select it explicitly. Packaging scripts run Tauri from the chosen
app directory, retaining the signing prerequisite; this is not installer/release qualification.
The native PNG/ICO resources are generated from MapGyver's editable SVG using Tauri's icon tool.

## Live Windows verification — 2026-10-28

All runs redirected backend Preferences/window-state files through an `APPDATA` root under
`target/mapgyver-m02-smoke/appdata`, with distinct `MapGyver` and `Chataigne` subdirectories.
WebView2 retained its normal per-bundle browser profile; the new app-scoped browser keys were
exercised there. Previously unscoped browser keys were neither migrated nor removed.

| Mode | MapGyver | Chataigne |
| --- | --- | --- |
| Bundled desktop, concurrent | Backend 7011, subscribed UI, 10-node snapshot | Backend 7010, subscribed UI, 170-node snapshot |
| Development desktop, concurrent | Frontend 5174/backend 7011, subscribed UI | Frontend 5173/backend 7010, subscribed UI |
| Headless, concurrent | Ready HTTP/read model, zero WebSocket clients | Ready HTTP/read model, zero WebSocket clients |

The first MapGyver development page did not connect because its CSP omitted the backend HTTP
origin. Its thin framework hook now delegates to Golden's public CSP helper using the configured
backend port. A fresh owned development window then subscribed successfully alongside Chataigne.
Both development responses preserved SvelteKit's nonce and allowed their selected backend.
When bundled assets are disabled with `--no-frontend`, the host also retains the app's configured
frontend URL instead of selecting another app's development server through generic port discovery.

For both actual hosts:

1. Discovery reported the correct app ID, display name, Cargo version and directory name.
2. Saving returned HTTP 200 and wrote the correct top-level application ID and file extension.
3. Uploading the other app's content under a native-looking filename returned HTTP 400,
   `recoverable: false`, with an explicit application mismatch. The live generation, file metadata
   and node count stayed unchanged.
4. Reopening the app's own saved file succeeded and advanced project generation from 1 to 2.
5. Snapshots exposed the separate configured app-data directory.

The bundled MapGyver window was visually observed with `MAPGYVER 0.1.0`, outliner, Dashboard,
inspector, logger and shared Preferences. Computer-use accessibility was limited by the elevated
app's Windows integrity; Chataigne activation was unavailable. No input/DPI/latency pass is claimed.
HTTP subscriptions and native window inventory confirmed both UI clients started.

GUI smoke runs stopped through owned shutdown sentinels and exited zero. Owned Vite listeners
were released. Headless children were terminated/reaped by the test owner after readiness checks;
that is not a graceful-shutdown qualification.

## Optional native-presentation lifecycle

The [public extension](../../crates/golden_core/hosts/desktop/src/native_presentation/mod.rs)
and [host adapter](../../crates/golden_core/hosts/desktop/src/native_presentation/host.rs)
belong to Golden. `run_default_with_native_presentation` and
`launch_engine_with_native_presentation` accept the typed extension separately from the
ordinary, cloneable `DesktopHostConfig`. Existing default entrypoints select no extension;
headless mode never prepares or starts it. The current underlay route is Windows-only.

The host constructs its transparent main window and clears native child clipping, then calls
preparation on the UI thread. Preparation may inspect the platform WebView through Tauri's
scoped UI API and return owned configuration in an unstarted `NativePresentationTask`.
GPU/SDK resource construction, use and retirement belong inside that task's worker. Its stop
token supports a lock-free observation and a cancellable wait; requests are bounded/idempotent.
No recurring UI polling or renderer wait is introduced.

Explicit exit signals cancellation while the event loop continues pumping. Task completion
follows retirement of its captures/resources; the host then requests native destruction and
waits for `Destroyed` before allowing exit. Joining happens after `run_return`. Worker errors/panics
are retained and returned when the host finishes.
Preparation errors leave by ordinary teardown instead of Tauri's setup-error panic path.
Spontaneous task completion leaves the editor open, preserving unsaved work. The frontend's
save/discard/cancel workflow remains before its confirmed host `window_destroy` command;
direct Tauri destroy permission is removed from both app capabilities.

The opt-in [Windows smoke](../../apps/mapgyver/tests/native_presentation/smoke.rs) is a public
consumer using its own application identity, loopback port and owned window. Default Cargo
tests never launch it. It exercises lifecycle/STA ownership without constructing GPU resources;
renderer recovery, preview geometry, output windows and quantitative retirement budgets remain
with M03/M05/M09.

The supported feature-gated binary passed on Windows: real platform-WebView COM access ran on
the UI STA, cancellation reached the worker, the UI remained responsive during delayed retirement,
the resource dropped on its worker while the HWND was live, and `IsWindow` was false after host
return. Run `cargo run --locked -p MapGyver --features native-presentation-smoke --bin native_presentation_smoke`
with an owned child `APPDATA` root and frontend/bind overrides unset. WebView2 uses a separate
bundle identifier/browser profile. Agent sandbox denial required native profile access for the
run; it did not require machine/display/overlay changes.

An initial integration-test executable failed before main because Tauri only adds its Windows
manifest to binary targets. The opt-in binary uses that supported path. The first actual native
run also exposed a still-live HWND after `run_return`; explicit destruction plus the native
acknowledgement fixes it. These are lifecycle checks without GPU resources or process/memory
retirement qualification.

## Validation and remaining scope

The ledger records final commands/results. Coverage includes protocol/codegen, identity-aware
persistence/recovery, engine behavior, configured host defaults, CLI app selection, public facade,
MapGyver project roundtrip, UI discovery/reconnect/storage isolation and pre-snapshot file guards.
Both apps' default desktop builds and both Svelte checks were exercised.
Final review added regression coverage for rejected uploads preserving existing primary/backup
files and never resolving a new destination, including recovery requests. Pure host tests cover
configured frontend selection with bundled assets, development mode and `--no-frontend`.

GPU execution, media, screen/surface policy and editors retain their M03–M08 owners. Original cadence, resource, DPI and
physical-output failures retain their M03/M05/M09 acceptance and unchanged budgets.

Known observations: Vite reports a large shared-workbench chunk; no splitting/performance claim
is made here. Earlier shell GUI shutdowns logged WebView's `Chrome_WidgetWin_0` unregister error 1412
despite exit zero. Full WebView/process/resource retirement remains unqualified. No overlay
settings, display configuration, native GPU timing captures, tags or releases were changed.
