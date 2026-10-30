# MapGyver app

This crate owns MapGyver's identity, project policy, frontend composition, and
desktop packaging configuration. Golden owns the engine, persistence, transport,
desktop host, and generic UI. App-owned media contracts and rendering policy
live under `src/media`; mapping authoring lives under `src/screens`. Composition builds on
those public app-owned contracts. `src/app/labels.rs` shares backend label allocation.

The initial project contains Media and Screen managers and Golden's standard empty Dashboard.
The primary app panel creates Solid Color, Picture and Shared Texture media with backend-owned defaults and
edits independent channel sources, color/alpha, automatic/fixed size and sampling. Media and
channels can be renamed, channels reordered, and media/channels removed through the
same history-aware app operations used by scripts and headless clients. Undo/redo,
project replacement and reconnection refresh the panel without polling.

Picture imports use one `ImportPicture` app intent: the backend builds the asset and
detached media/channel tree atomically, or imports/assigns a source to an existing Picture
channel while preserving its output settings. One Undo reverses the complete import.
The asset catalog appears separately in manager inspection. Channels can select the same
asset, and relinking that asset preserves its identity for every consumer. Missing asset
references stay explicit instead of silently selecting another source.

Shared Texture channels accept an exact Spout sender name, independent source maximum dimensions,
storage/color/alpha requirements and output size/sampling. An empty selection explicitly disconnects
that channel. Existing native supervision reconnects to the selected name without choosing a global
active sender. The desktop editor also lists discovered names and offers explicit refresh. Missing
authored names remain selectable and editable; a scan never changes the channel definition.

Golden Video's public `native::SpoutDiscovery` owns worker-local SDK enumeration without opening
a renderer or receiver. MapGyver's existing presentation metadata worker performs on-demand scans,
coalesces concurrent refreshes and caches at most 64 exact names. Its encoded-name budget is 48 KiB,
leaving room within the shared host's unchanged 64 KiB reply limit; partial catalogs are explicit.
The native control callback reads the cache and signals the worker, without SDK/engine work.
Rust owns the request/snapshot DTOs and generates their TypeScript. Revisioned completion events
prevent an older scanning reply from overwriting a finished catalog.

Discovery currently uses the desktop-native metadata capability. Browser/headless callers retain
exact-name authoring. Interactive selection, cross-host/source parity and full independent-source
rendering/recovery acceptance remain open in M04.

The Picture editor accepts explicit project-relative or absolute host paths; the native
desktop also offers Golden's file picker. This is source-file linking, not a browser upload.
Asset path validation, ID/label allocation and all domain changes stay in Rust. Worker
decode, color/alpha handling and preparation budgets remain owned by the existing Picture
and Golden Assets runtime; the editor performs no image decoding or frame transport.

`src/media/protocol.rs` owns those DTOs; `generate_media_protocol` generates the
frontend contracts. `src/media/catalog.rs` provides ordered, read-only inspection
in one media/channel traversal. The UI under `ui/src/lib/media` registers through
Golden's public panel API and consumes its public workbench session facade. It
does not allocate identities, choose backend defaults or mutate node internals.
App invariants run at the detached candidate-validation boundary, after queued
default/load edits attach and before live activation, for both startup and replacement.

Screen Manager owns Screens and ordered Surfaces. Its Add menu offers Screen Operation commands,
a Create Screen preset and a Screen Script with embedded method/callback snippets. The public
`mapgyver.screens.edit` app operation accepts the Rust `ScreenEdit` contract. Hosted scripts use
`local.editScreens(requestId, requestJson)` and `local.inspectScreens(requestId)`; results arrive
through `onAppOperationResult` after commit or rejection. Commands also expose `execute()`.
Creation allocates a Screen and initial full-source/full-screen Surface in one detached subtree
and Undo step. Rename, reorder, configuration and removal use the same atomic route for every
caller. Screen and Surface identities survive history and project reload.

Screens start disabled with no target. Authored targets distinguish a stable fullscreen display
identity, a rectangle in signed physical desktop pixels, and an exact virtual Spout sender name
with texture extent. Surfaces store a media/channel reference, normalized source crop, destination
quad, opacity, calibration mode and enablement. Invalid extents, crop, crossed/concave/degenerate
quads, opacity and cross-owner references are rejected before mutation. Off-screen convex quads
and mirrored winding are allowed. Deleting a source preserves its reference for Undo; calibration
edits retain routing even while that source is missing.

`src/screens/protocol.rs` generates `ui/src/lib/screens/generated` through
`generate_screen_protocol`; the typed UI operation adapter consumes that contract. The registered
Screen Manager tab exposes Screen/Surface creation, selection, names, order and removal, target
settings and a Surface Source form for assignment, crop, destination corners, opacity and
calibration. Forms start from authored backend data; selecting a target mode supplies no guessed
extent, origin or display identity. Apply submits one atomic operation. Missing channel selections
remain explicit, including when a source disappears while its form is open.

The focused authoring controller refreshes from committed history, project/connection changes and
app events rather than polling. Session/project/request fences reject late inspection/edit replies;
unmount retires the controller. Catalogs do not create thousands of DOM controls: navigation pages
contain 50 Screens/Surfaces and source search shows at most 100 matches plus the selected source.
`src/screens/rendering.rs` captures bounded Screen/Surface metadata and revision guards on the
control actor. The preparation worker resolves channel textures, validates project identity and
builds ordered mapped layers through Golden Video. `ChannelRenderer::publish_screen_targets`
publishes these Screen textures and media routes together, deduplicating demanded channels and
sharing each Screen pass across same-sized consumers. Surface edits invalidate captures while
runtime texture identities remain stable. Source deletion keeps an explicit transparent channel
placeholder and Undo restores it. Initial Surface resampling is linear.

Golden owns checked inverse homographies and GPU sampling; authoring uses these same validation
primitives before mutation. Source crop precedes perspective mapping, with mirrored/off-screen
quads supported. Finite native reference patterns cover crop/warp and alpha/backdrop preservation.
The Screen Manager now previews these composed textures through the same native session as
Channel previews, including while its authored output is disabled. Screen rectangle/Spout extents
define preview resolution; unconfigured/fullscreen targets use the backend's default canvas until
the display catalog can resolve an extent. Optional Screen-route bindings allow a preview-only
texture to feed the existing window compositor without opening a physical or Spout output.
Calibration Grid and Color Bars are now source-independent procedural layers. Grid uses the
app's 16×9 white-on-black policy; bars use white/yellow/cyan/green/magenta/red/blue/black. Patterns
fill the mapped quad while retaining the authored source/crop for Off. Quad, order, enablement and
opacity continue to apply. Active calibration adds no media/asset/input demand and creates no
calibration texture; Golden evaluates it at target resolution. Switching Off restores the saved
source route and crop without changing their identities. Backend/runtime and finite GPU checks
verify these semantics. An ordinary-app, independent Spout receiver flow also verifies delivery
with no editor loaded for Grid/Bars, restored media routing and enable/disable/resize recovery.
Calibration visual/form-input acceptance remains open.

Enabled virtual Spout Screen targets now publish through this same renderer. Output capture is
independent of preview registrations; hiding or closing views cannot remove its source demand.
The host worker captures at most 128 Screen definitions and admits seven virtual targets plus
the host preview consumer within the existing eight-output budget. Runtime output IDs remain
stable through toggle/rename/reorder and are separate from persisted Screen identities. Duplicate
enabled sender names fail atomically; names admit 239 UTF-8 bytes, reserving the SDK counter
namespace. Native output storage is BGRA8 sRGB with straight alpha; Golden converts at delivery.
Output availability/errors reuse app preview status; the native backend retains its bounded
independent endpoint retry strategy.

Golden's public native bindings are immutable per renderer session. Target enablement, name or
extent changes therefore retire the shared session through actual GPU completion before creating
the replacement; this can briefly interrupt other outputs and restarts prepared source sessions.
Surface edits and editor geometry/visibility changes only replace the render plan. This explicit
session boundary avoids overlapping same-name SDK senders or swapchains on one HWND. Physical
target windows, display recovery and full graphical Screen/Surface Source editor gestures remain
M05 work. No physical display, minimize, latency, color or full target-concurrency pass is inferred.
The finite delivery check is `node apps/mapgyver/tests/native_output/check.mjs` from the workspace
root, after building `MapGyver` and `native_qualification` with `--features native-qualification`.
It uses an isolated port, ephemeral unsaved authoring and clean shutdown; artifacts stay in `target`.

Media Manager's Add menu offers Media Operation commands, creation presets for each source kind,
and a Media Script with app-owned function/callback snippets. Commands keep a JSON `MediaEdit`
request and correlation id in ordinary authored parameters; Execute or `command.execute()` runs
it through the same atomic backend route. The generic command supports the complete operation
contract, including channel configuration, import/relink, rename, reorder, removal and inspection.
Presets are serialized from Rust operations rather than separate schemas or default policies.

Hosted scripts expose `local.editMedia(requestId, requestJson)` and
`local.inspectMedia(requestId)`. Call `local.listen({ level: 1 })` once to receive
`onAppOperationResult(reply)` for manager methods and its direct commands. Applied replies carry
the committed `MediaEditResult`; rejected operations carry a message and no partial changes.
Use distinct correlation ids for concurrent requests. Any script can also use Golden's existing
`gc.invokeAppOperation` route. The ready-to-use snippets live under
`src/media/script_templates/snippets` and are embedded in the app, including packaged builds.

Golden's public `ProcessCtx::enqueue_app_operation` admits node requests under the same bounded
service, callback completion fences and result/disposal ownership as script requests. Source
destruction or disablement expires MapGyver's request owner, so Undo cannot resurrect abandoned
work. Manager and command nodes are passive; no polling or recurring engine work was added.
Command triggers emit pulses without an empty history entry. One successful structural operation
is one Undo step, and merely loading a saved command does not execute it.

On Windows, the ordinary app enables worker-owned native Channel/Screen previews and enabled
virtual Screen outputs by default, without requiring any preview to initialize the renderer.
Each view registers a clipped region through `golden_video_ui`; app source resolution and shared
host preparation live under `src/preview`, with media rendering under `src/media/rendering`.
Golden owns the window,
native geometry, renderer and retirement. Browser/headless editing starts no presentation;
`--no-default-features` also retains the engine-only build. Preview metadata/status crosses IPC,
while images upload once on the rendering boundary and frames remain native.

The shared host supplies the existing authoritative `ProductionRuntime` to its presentation
factory. `capture_app` captures bounded owned data/revision guards on the control actor; all
IO, plan building and rendering stay on the preparation worker. Picture roots follow the
current project's directory. A coarse worker wake detects authored guard changes without
polling the actor; geometry, GPU and file completion also notify the worker.

`src/preview/protocol.rs` owns tagged Channel/Screen source keys and sparse status metadata;
`generate_preview_protocol` generates the shared UI contracts under `ui/src/lib/preview/generated`.
The shared UI context owns one session and separate typed source identities. Bounded actor capture
deduplicates channel demand across views and Surfaces, and includes Screen membership/definition
guards so edit, deletion and Undo trigger fresh preparation. An unavailable/unsupported Screen
has its own status and no resolved region; healthy Channel views continue. Screen warnings are
overlays over the composed result. No frame pixels, native handles or per-frame matrix solving
cross the UI boundary. Generator bootstrapping may use `GC_SKIP_UI_BUILD=1`; bundled/native
verification uses the ordinary build with frontend embedding enabled.

Native Solid Color and 4K PNG Picture output have been observed in the ordinary app. Explicit
relink to an HD image, missing-file feedback, relink recovery and normal Picture shutdown also
worked with stable channel/asset references. These checks used public backend operations;
interactive Picture controls and complete source/recovery/parity acceptance remain open.

`src/preview/host/config.rs` owns production admission. Raw plan-input limits increased from
1/4 MiB per-plan/aggregate to 257/260 MiB because their accounting includes retained image leases.
This adds no pixel copy: the preparation cache remains capped at 256 MiB, source input at 64 MiB,
decode workspace at 128 MiB, GPU resources at 512 MiB and native Spout input at 256 MiB.
PNG workspace admission now charges peak overlapping buffer lifetimes rather than adding
disjoint codec/conversion/orientation phases. JPEG admission remains conservative; 4K JPEG was
not verified. No M03 qualification budget changed or is claimed satisfied. Physical 4K output,
color/alpha accuracy, timing and resource qualification retain their existing gates and failures.
Remaining M04 source/recovery/UI parity checks are tracked in the
[active acceptance ledger](../../docs/progress/mapgyver-status.md#m04-acceptance-items).
The native feasibility probe under `tools/mapgyver/native-probe` remains a separate
diagnostic executable; it is not the app's rendering implementation.

Run these commands from the workspace root:

| Command | Purpose |
| --- | --- |
| `npm run run:mapgyver` | Launch the bundled desktop app. |
| `npm run watch:mapgyver` | Run the shared development watcher for MapGyver. |
| `npm run dev:mapgyver` | Start only the frontend development server. |
| `npm run build:mapgyver` | Build the frontend. |
| `npm run check:mapgyver` | Check frontend types and Svelte components. |
| `npm run package:mapgyver` | Build the desktop bundles from MapGyver's app directory. |
| `npm run package:check:mapgyver` | Compile through the packaging path without creating bundles. |

`cargo run -p MapGyver -- --dev` launches the app with its frontend development
server. Use `cargo run -p MapGyver -- --headless` for the shared transport without
a window. The packaging commands retain the workspace signing prerequisite.
MapGyver defaults to backend port `7011` and frontend development port `5174`;
`GC_UI_BIND` and `GC_UI_FRONTEND_URL` retain the shared host's explicit overrides.
Unqualified root commands continue to select Chataigne.

`build.rs` selects the `mapgyver-ui` npm workspace through
`golden_codegen_support::ui_bundle`; it embeds the resulting assets into the
app's Cargo output directory. The frontend discovers and validates the backend's
MapGyver metadata before creating a workbench session or accessing app storage.

The editable icon is `ui/static/favicon.svg`. Regenerate native icon exports with
`npm exec -- tauri icon apps/mapgyver/ui/static/favicon.svg --output apps/mapgyver/icons`
from the workspace root; the Windows app tracks and uses only `icon.png` and `icon.ico`.

The shared host's optional native-presentation seam has an explicit Windows lifecycle smoke:
`cargo run --locked -p MapGyver --features native-presentation-smoke --bin native_presentation_smoke`.
Set `APPDATA` for that child process to an owned test directory, and unset `GC_UI_BIND` and
`GC_UI_FRONTEND_URL`. It opens/closes only its own window and uses a separate application ID.
Default tests do not launch it. This checks UI/worker/window lifetime, without GPU qualification.

The production renderer's owned Q01/Q02 runner lives under `tests/native_qualification` and
uses Golden's shared host/runtime with independent GPU source and consumer processes. Build it
with `--features native-qualification --bin native_qualification`; see the
[runner contract](../../docs/progress/mapgyver-m03-qualification-runner.md) for commands,
observations and qualification limits. Short diagnostic controls do not close native quality gates.
