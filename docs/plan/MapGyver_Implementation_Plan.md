# MapGyver implementation plan

MapGyver is a Windows x64 video mapping app alongside Chataigne. MapGyver owns product
authoring and behavior; Golden owns reusable engine, worker, GPU and workbench mechanisms.
See the [architecture](../architecture/mapgyver.md) and [current status](../progress/mapgyver-status.md).

This is the single execution entrypoint. The [updated proposal](MapGyver%20Updated%20Plan.md)
is merged here as of **2026-11-02** and takes precedence over conflicting earlier requirements.
The **2026-11-03 feedback** is incorporated in **M11** below and supersedes conflicting
presentation, source-authoring and resource-capacity requirements. **All M11 gates take
priority over M10 and M09**. Earlier acceptance is evidence for its tested contract; changed
behavior requires M11 acceptance. This planning update claims no implementation or new acceptance.
Implementation is active under the user's MapGyver goal. Planning awards no acceptance.

## Entry, order and delivery

1. Read AGENTS.md, this plan and the compact status page. Reconcile the checkout, current
   shared Golden work and existing evidence. Preserve unrelated edits and completed work.
2. Use branch `mapgyver` and `origin/mapgyver`. Commit and push each coherent milestone
   after relevant checks and status updates, under the standing repository authorization.
   Do not force-push, release or deploy.
3. Continue the selected gate in the status board. Record its concrete deliverable, owner,
   proving check and stopping time in the status page's single replaceable batch entry.
   Default to 45-minute slices; investigations have the AGENTS.md 60-minute/two-experiment
   cumulative limit. Finish or checkpoint the same gate at cutoff rather than expanding scope.
4. Execute the revision in this order: **M04 ownership/Resolution/Reference → M04 Media Preview
   and M05 navigation/Screen interaction → M06 Composition → M08 reusable app-control boundary
   and M07 Matrix redesign together → M11 feedback corrections → M10 Video tiers → M09 final qualification**.
   M08's shared configuration/runtime contract precedes dependent Matrix app controls; this
   dependency does not authorize two active gates. Deliver their coherent slices sequentially.
   From the current checkout, retain **M08/4 → M11/1–10 → M10 → M09**;
   retained M03/M05 qualification stays in its named return lane. A reproduced blocking defect
   still takes precedence. Do not start M10 or M09 while any M11 gate remains unaccepted.
5. The unrestricted finite Surface-warp correction is already delivered and manually accepted.
   Preserve it and its evidence; do not restart that correction.
6. Functional dependencies permit product implementation before unrelated physical/timing
   qualification. Retain M03/M05 blockers with concrete return triggers. A reproduced correctness,
   ownership or integrity defect blocking the selected path must be fixed before building on it.
   Apply AGENTS.md's product/diagnostic allocation during feature delivery; switch to its
   acceptance-closure rules once completing the implemented revised scope.

M04/1–8 are now accepted, including the retained source qualification returned to after
Reference and Preview delivery. [M04/6 and the complete phase audit](../progress/mapgyver-m04-source-parity.md)
close source caller/Picture/project/resource parity. [M06/1's child-media model](../progress/mapgyver-m06-model.md)
now passes the complete degree/structural/API/Parameter/history/project contract.
[M06/2 dependency/sharing](../progress/mapgyver-m06-dependencies.md) also passes all-media cycles,
missing/Undo, deep/large graphs and compiled production sharing. The five-producer/six-alias
software proof satisfied the retained return trigger. [M06/3 native Composition](../progress/mapgyver-m06-native.md)
now passes all nine original stages on both GPUs, including the active black preview canvas,
with unchanged scenes/quantization/error tolerance/budgets. [M06/4's anchored editor](../progress/mapgyver-m06-editor.md)
also passes actual native resize/rotation/cursor, pointer/history/zoom and tab lifetime.
[M06/5's independent panels](../progress/mapgyver-m06-panels.md) now pass bounded selected lookup,
persisted follow/lock/page choices, actual reconnect/project/tab/native lifetime and the current
generic Inspector/File Open/New regressions. M06 is complete. [M08/2 configuration/demand](../progress/mapgyver-m08-configuration.md#configuration-and-demand-acceptance--2026-11-03)
is now accepted, including typed Column/Cell settings, standalone removal, normalized sharing,
independent activation and retained demand through disabled rendering, interruption, history,
project replacement and final-use completion. [M07/1 model acceptance](../progress/mapgyver-m07-model.md#m071-acceptance--2026-11-03)
is complete: typed Layer/Group/Column/Cell ownership, backend 3×3 defaults, actual child media,
bounded Inspector and complete authored API/history/project/copy behavior.
[M07/2 playback acceptance](../progress/mapgyver-m07-playback.md) is also complete: precedence,
independent/atomic activation, disabled/empty policy, interruption and shared-time/final-use
demand pass, with current native fade observations on both GPUs. [M07/3 grid
acceptance](../progress/mapgyver-m07-grid.md) is complete: actual contextual creation/removal,
inline rename, structural copy/delete and virtualized drag sorting/reparenting, bounded shared
Inspector/keyboard/resize/project lifetime and truthful native submission/retirement feedback pass.
[M07/4 native/scale/lifetime acceptance](../progress/mapgyver-m07-native.md) completes M07 at 4/4:
both GPUs pass the complete current raster, source recovery/sharing, sparse bursts/reload,
interruption and final-retirement matrix. App Control lifecycle requirements now avoid per-owner
whole-document snapshots during bulk typed creation. [M08/3 actual runtime acceptance](../progress/mapgyver-m08-runtime.md)
now verifies both native readiness modes, shared owned processes, cancellation, completion-based
retention and stale-project rejection. M08/4 is the single active gate for complete picker/UI/
script/target/access qualification before M11. Retained physical/timing qualification and the
original Matrix fixture budgets are unchanged.

[Typed Layer/Group settings](../progress/mapgyver-m07-model.md) use real Parameters and
the generic Inspector, with atomic Configure/history/copy/project behavior. Layer position,
canvas and duration now drive actual rendering and per-Layer playback; native position/resize/
Undo passes on both GPUs. Cell v3 now owns typed relative position/opacity/enabled with no size;
its current verification is recorded in the accepted model report. The accepted M07/1 fixture
starts with three Layers and three Columns, sparse empty addresses and four paired outputs in
one backend tree; M11/8 changes default exposed outputs while preserving internal compositing.
Catalog creation, exact Undo/Redo/reload and whole-Matrix copies pass; queued reads after deletion
return typed absence and recover on Undo. Full grid/playback/native/scale gates remain unchanged.

M04/7's default Reference/Alt-copy drops established actual Composition/Matrix child-media
containment before Reference/Preview acceptance. Reuse that ownership/copy/drop foundation.
M06/M07 retain their complete transform, editor, playback, scale and native acceptance;
this dependency work does not accept those later gates.

M04/7 verifies the Reference contract below through typed callers, bundled app interaction and
the production native renderer. Whole-app Q04 qualification remains in M09, including M06's
bounded Composition providers and M07's revised grid/active-demand workloads. Reference's 10k
backend receipt fixture supplements its functional evidence; the existing whole-app budgets,
larger authored counts, UI, memory and output requirements remain required in their owning gates.

The unqualified phrase “implement the plan” still selects Chataigne under AGENTS.md.

M05/10 is [accepted](../progress/evidence/mapgyver-workbench-status-2026-11-03.json): responsive
bounded Screen/Surface row supplements and native runtime footer are verified. M05/6–7 are now
[accepted](../progress/evidence/mapgyver-native-zoom-sync-2026-11-03.json): native image/decoration
paint, an internal Source landmark, output-accepted moving-frame hits and projected DOM events
are verified alongside the complete bundled editor input/history/lifetime suites. A verified
owned-foreground desktop capture route supplies native pixels; moving-hit metadata is not a
physical latency measurement. M05/9 [stable output-window bounds](../progress/evidence/mapgyver-output-bounds-2026-11-03.json)
is also accepted: one visible HWND through move/resize/Undo and both actual display targets,
with pending allocation fenced and completion-safe backing replacement. M05/4 [concurrent
fullscreen/rectangle/Spout](../progress/evidence/mapgyver-concurrent-output-2026-11-03.json) now
passes actual editor close, OS-confirmed minimize/restore and changed physical/Spout pixels.
M04/4 [Spout source acceptance](../progress/mapgyver-m04-spout.md) also passes full lifecycle,
cross-adapter recovery and RGBA pixels on both GPUs, plus declared input capacity/retirement.
[M04/5 source UI acceptance](../progress/mapgyver-m04-source-ui.md) passes the complete retained
interaction matrix through typed Channels and generic Inspector controls, including fresh
permissions, safe deleted-channel reads and context-menu history. [M04/6 source parity](../progress/mapgyver-m04-source-parity.md)
also passes revised callers, all-source copies/history/project and the complete both-adapter
native Picture recovery/recreation matrix, completing M04. M05/5,8 remain
in the physical return queue; every original matrix and acceptance requirement remains unchanged.

Historical batches, pauses and execution instructions in the
[evidence archive](../progress/mapgyver-evidence-through-2026-11-02.md) are non-operative.

## Product contracts

- Use the thin app and Golden desktop/headless host, generic Inspector, shared selection,
  persisted manager folders, history, scripting, transport and persistence.
- Every authored setting is a typed Parameter child. Collections use real child nodes.
  DTOs project these values; UI stores never become a second authored document.
- Rust owns control/protocol types and generated TypeScript. Change DTOs, generation and all
  consumers together. No pixels or native handles travel through persisted/control data.
- Ordinary parameters use the generic Inspector. Specialized editors fill their panels,
  use compact accessible icon actions, and send explicit backend intents. The backend owns
  defaults, identity, validation, geometry, structural edits and playback/app policy.
- Every module slice includes command nodes, script functions, callbacks and app-owned snippets.
  Validate UI/script/headless parity, rejection, grouped Undo/Redo, copy and project roundtrip.
- Keep demand, decode/preparation and GPU work bounded and off the engine/UI thread. Consumers
  share rendered targets and decoder ownership. Endpoint integrations need recovery/backoff,
  cancellation, project/device fencing and completion-based final-use retirement.
- No compatibility layer is planned. Version changed document structures explicitly and reject
  unsupported old records clearly; never reinterpret old records silently.

## Milestones and dependencies

IDs remain stable; **M11 executes before M10 and M09**. M04–M08 retain their earlier evidence;
M11 owns acceptance of the new corrections to those contracts.
Counts refer to the numbered acceptance gates below, not commits or elapsed work.

| ID  | Outcome                                                  | Functional dependency                                       | Gates |
| --- | -------------------------------------------------------- | ----------------------------------------------------------- | ----- |
| M00 | Reconciled baseline and qualification workloads          | Explicit product selection                                  | 1     |
| M01 | Functional native host/backend/binding decision          | M00                                                         | 4     |
| M02 | Public multi-app seams and runnable thin shell           | M01 functional decision                                     | 7     |
| M03 | Reusable rendering, channels, time, assets and preview   | M02                                                         | 9     |
| M04 | Source variants, Resolution, Reference and Media Preview | M03 gates 1–3, 5–8                                          | 8     |
| M05 | Screens, Surfaces, navigation and physical output        | M04 working revised contracts                               | 10    |
| M06 | Child-media Composition and transform editor             | M04 and M05 working contracts                               | 5     |
| M07 | Child-media Matrix, playback and virtualized grid        | M06; M08 configuration for app controls                     | 4     |
| M08 | Reusable optional app control and shared runtime demand  | Existing Matrix playback; revised M04 ownership             | 4     |
| M11 | Feedback: stable rendering, simpler authoring, shared UX | Existing M04–M08 product contracts; one gate at a time      | 10    |
| M10 | Tiered Video playback                                    | Revised M04–M08 working contracts and all M11 gates         | 4     |
| M09 | Actual-app qualification and Windows artifact            | All retained feature/qualification gates, including M10/M11 | 1     |

### M00 — Reconcile and define evidence

**Gate 1:** retain the reconciled checkout/public-boundary inventory, exact available native
routes and machine/toolchain/OS/WebView/GPU/driver/display identities, fixed workloads and
budgets. Preserve Chataigne's root-default launch. Evidence:
[baseline](../progress/mapgyver-m00-baseline.md) and
[qualification contract](../progress/mapgyver-qualification-contract.md).

### M01 — Decide the functional native rendering route

Retain the user-approved functional scope and decision: ordinary Tauri/Wry native underlay,
D3D11 and pinned `spout2-sys =0.1.1`. Full performance/display qualification stays in M03/M05/M09.

1. Observed ordinary underlay clipping, layering and DOM/native input on recorded configurations.
2. Caller-owned D3D11 devices and two differently sized GPU-only Spout inputs compose to
   native and Spout outputs; binding/SDK ownership is audited.
3. Scoped resize/restart, editor/output independence and owned-resource/process cleanup smoke.
4. Recorded host/backend/binding decision, limitations and owners of continuing qualification.

Retain the [functional decision](../progress/mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28),
the [native research](../architecture/mapgyver-native-rendering.md) and the approved
[NVIDIA overlay prerequisite](../progress/mapgyver-qualification-contract.md#nvidia-overlay-prerequisite).
This remains four functional gates, not a pass of the original broad native matrix.

### M02 — Establish the second app without product leaks

The shared host owns bootstrap, native windows and COM coordination. App-specific resources,
identity and lifecycle registration stay in MapGyver. Never copy Chataigne's private modules.

1. Public UI-bundling API adopted by Chataigne.
2. Thin MapGyver host/UI scaffold, identity, project specifications and resources.
3. App branding and browser-state isolation for layout, selection, preferences and recent files.
4. Generic generated discovery contracts and endpoint isolation.
5. Explicit app-selected Cargo/npm/xtask/watch/package commands; Chataigne remains the root default.
6. Optional typed native-presentation host lifecycle extension.
7. Independent/concurrent bundled/dev launches and headless/engine/protocol public-boundary checks.

Retain the [shell evidence](../progress/mapgyver-m02-shell.md); native quality is a separate gate.

### M03 — Deliver render/resource and channel foundations

Golden Video owns app-neutral immutable render plans, scheduling, opaque resources and native
execution. MapGyver projects its authored media into guarded plans. Reuse Golden Time and Assets.

1. Deterministic DAG/demand/resource-budget compilation, cycle/resize/rejection and explicit
   texture/color/alpha contracts.
2. Worker runtime/null path, atomic install and project/revision/device fences; bounded desired,
   prepared, in-flight and retiring work with completion-based resource ownership.
3. Exercised D3D11/Spout compositing/conversion and shared-host native presentation; explicit
   caller-device/adapter ownership.
4. Production source/output observations, device-removal/recreation, isolated healthy paths,
   cancellation and bounded recovery/retirement.
5. Stable media/channel IDs and typed references, authoritative backend size/default operations,
   generated contracts, history/reload/resource-recreation identity.
6. Shared monotonic production-time and activation/discontinuity authority.
7. Persistent asset IDs/locators, worker preparation/cancellation, relocation/relink and
   stale-generation/resource admission.
8. Reusable preview registration/geometry and generated control contracts exercised natively.
9. Production Q01/Q02 CPU/GPU/cadence/frame-age/admission/memory/handle/shutdown and applicable
   recovery qualification against the retained contract.

Gates 4 and 9 remain open. Existing clock/timing proposals are unapproved; no budget or physical
requirement is waived. Use [render foundations](../progress/mapgyver-m03-render-foundations.md)
and the [qualification contract](../progress/mapgyver-qualification-contract.md), not a new probe chain.

### M04 — Deliver source variants, Reference media and Media Preview

**A. Media/source ownership (gates 1–6).** Channels are stable concurrent output variants in a
media's **Channels** section. Hide channel nodes from the Media Manager outliner and remove
channel creation from manager `+` actions. Edit variants as real typed child nodes. Shared
settings belong to the media; variant differences are Solid Color color, Shared Texture sender,
Picture asset and Video file. Composition initially has one output; channel retargeting is future work.

Every media has a disableable **Resolution** typed Vec2 in pixels, disabled by default, with
integer constraints, unit steps and backend bounds **1–16384 per axis**. Disabled resolves to
source-native extent or **1920×1080** for procedural canvases. Enabled forces every channel to
that extent. Under M11/4, applicable media use one shared **Fit Mode: Fit / Fill / Stretch**
when Resolution is enabled; no per-channel authored size. Hide Fit Mode for automatic size and
omit it for Solid Color. Remove authored Kind/SourceKind and generic Filter/Sampling controls;
source kinds remain typed backend identity and sampling defaults to Linear. Keep each channel's
actual source difference directly editable in expanded, compact Inspector rows.
M11/5 adds media-level Manual/Auto Shared Texture selection with typed per-channel fields.
Remove authored Shared Texture format, encoding, alpha and maximum dimensions:
discover format/encoding at the native boundary, use straight alpha, and retain internal
allocation limits and recovery. Remove useless `+` actions from declared Parameter folders
through their container capabilities.

1. Default Media Manager, detached creation/import, authoritative identity/defaults and complete
   command/function/callback/snippet API under the revised media/variant ownership.
2. Concurrent variants with distinct native source sizes, disabled/enabled media Resolution,
   shared consumers and stable channels through resize/reorder/history/reload.
3. Worker Picture decode/color/alpha and source/pixel/decode/cache bounds; preserve applicable
   native Picture evidence and bounded upload/cancellation/retirement.
4. Exact independent Spout sender selection/discovery, runtime metadata, absence/restart/rename/
   resize, isolated failure and explicit adapter mismatch without fallback.
5. Actual manager/Channels/generic Inspector interaction, typed field ownership and folder
   capabilities; no ordinary custom forms or channel creation in the manager.
6. Complete revised UI/script/headless parity, missing/relinked/replaced Picture behavior,
   duplication/Undo/Redo/project roundtrip and functional native resource recreation.

**B. Reference media (gate 7).** A typed target reference mirrors target channels/content,
using target channel identities for synchronization. It owns no source settings, transport or
decoder. Default drops into Composition/Matrix create Reference children; **Alt+drag** copies
the entire media subtree with independent identities. Share target rendering; an explicitly
enabled Reference Resolution adds a shared-source resampling pass.

7. Reference channel add/remove/reorder synchronization, missing/recovered targets, mixed
   Reference/Composition/Matrix cycle rejection, shared rendering/decoder ownership, resampling,
   default-drop/Alt-copy semantics and full Parameter/API/history/project parity.

**C. Media Preview (gate 8).** Add a dockable panel that follows selected media or descendants,
with channel choice/all-channel grid and independently persisted lock/channel/panel state for
multiple instances. Remove Inspector media previews. Reuse native leases/retirement.

8. Actual independent preview instances, selection-follow/lock/grid, reconnect/project fencing,
   close/lock retirement and uninterrupted independent outputs; bundled/native interaction.

### M05 — Deliver Screens, Surfaces and shared navigation

Retain crop followed by perspective warp, calibration, ordered alpha composition, fullscreen/
physical rectangle/virtual Spout outputs and display identity/recovery. Finite invalid quads
remain editable with red edges/dim red fill; omit only unrenderable layers and recover when valid.
Invalid crops reject atomically. Screen/Surface settings stay in generic Inspector.

Golden's reusable viewport animates zoom toward the requested scale around a stable pointer
anchor. Screen, Surface Source and Composition share it. Native imagery, overlays and hit testing
stay aligned during animation; disposal/document replacement cancels obsolete work.

1. Atomic Screen/Surface authoring, stable routing/targets/crops/quads, default manager and
   complete module API/history/project lifetime.
2. Reference-pattern crop/warp correctness, finite invalid intermediate editing and safe recovery.
3. Calibration without a source restores routing; overlapping Surfaces preserve order/alpha.
4. All three target modes work concurrently and continue while editors close/minimize.
5. Display identity survives hotplug, mixed DPI and negative origins.
6. Screen editor: actual Surface/shared Inspector selection, background deselection and editor-local
   individual handle selection. Under M11/6, unselected Surfaces have only a thin border;
   selected Surfaces alone have a light overlay, slightly thicker border and handles. Arrow keys move
   handles **1 output pixel**, **10 with Shift**; held repeat is one Undo gesture. **Alt+drag**
   translates all four corners. Backend **Fill Screen** stretches the quad; **Fit to Screen**
   centers it with source aspect preserved. Verify pointer/keyboard input and smooth zoom alignment.
7. Surface Source crop/mapping/calibration, full-source native imagery, smooth zoom, selection,
   grouped history and public-caller parity.
8. Retained native physical matrix: exact output/preview geometry, alpha/color/reference patterns,
   100/125/150/200% and mixed DPI, negative origins, overlays/capture, measured ≤1-physical-pixel
   settled alignment and contract input latency. Unavailable configurations stay open.
9. In-place native output bounds updates for rectangle position/size/display reassignment:
   output stays visible with the same window identity and completion-safe GPU resize/retirement.
10. Responsive Screen outliner supplements for output/target, resolution and FPS; Surface source
    media supplements, plus a public Golden footer contribution hook for actual FPS, active GPU,
    frame time and resource usage. Use bounded runtime observations rather than frame-rate engine
    polling; M10 adds Video observations through the same hook.

### M06 — Deliver child-media Composition and editor

Canvas items become actual child media nodes, replacing reference-only instance authoring.
Default drop inserts a Reference; Alt+drag copies independently. Transform Parameters belong
to those children: placement, scale, pivot, rotation, opacity and blend. Rotation is authored
and publicly edited in **degrees**, converted only at the rendering boundary.

1. Single-output Composition, ordered owned media children, typed transforms and structural
   copy/reorder/removal; complete API/Parameter/Undo/Redo/versioned project behavior.
2. All-media dependency/cycle validation, missing target/deletion/Undo, nested/deep/large graphs
   and shared target rendering without source/decoder duplication.
3. Correct native nested/shared-source output, canvas resize, alpha/blends, pivots, signed scale
   and degree conversion, with one render per demanded output.
4. Backend-authoritative selection/geometry/history; corner resize fixes the opposite corner,
   edge resize acts on one axis, outside-corner rotation uses a rotation cursor. Remove the
   anchor-adjacent rotation control. Verify actual pointer input, smooth zoom and grouped history.
5. Multiple independent Composition panels follow selection or lock per panel, restore state
   and survive project/tab lifetime. Remove internal headers and permanent creation/refresh bars;
   M11/6 places relevant actions beside shared navigation and M11/7 adds contextual child creation.

### M07 — Deliver child-media Matrix, playback and grid

New Matrices have **three layers and three columns**. Use a full-panel virtualized square-cell
grid, clearer layer/group separation and distinct selection/requested/actual runtime markers.
Remove the internal header and custom property forms. Selected offscreen objects remain
editable through bounded backend inspection independent of visible rows.

| Owner  | Typed authored settings                                                                                                                      |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Layer  | Position, size (layer canvas), opacity, blend, transition duration, **Expose Channel (default false)**; retain enabling/group behavior       |
| Column | Optional transition override, optional App file and conditional app-control settings, including integer pre/post                             |
| Cell   | Position relative to Layer, opacity, enabled, optional transition override, optional App file and conditional app controls; **no Cell size** |
| Group  | Typed opacity/blend/enabling, **Expose Channel (default false)** and actual hierarchy/order; a rendered blend owner, not just a folder       |

Populated Cells own actual child media; empty addresses stay sparse until content/settings need
a Cell node. Cell media retains its own size. Transition precedence is **Cell override → Column
override → Layer duration**; retain the **250 ms** default. Disabled Cells leave active layer
content unchanged. Enabled empty Cells retain transparent-transition policy. Explicit individual
Cell triggering does not change the current Column anchor; Column launches remain atomic.

1. Revised stable sparse document/derived layer/group/final identities, grouping and actual
   child media; 3×3 defaults, default Reference/Alt-copy, complete typed API/history/project behavior.
2. Deterministic transition precedence, independent Cell triggers, atomic Column launches,
   disabled/empty policies, interrupted blends and final-use outgoing demand under shared time.
3. Actual virtualized-grid interaction: drag sort/reparent Layers/Groups, sort Columns, double-click
   rename, centered empty-cell `+`/context Add Media and compact filled-cell remove; shared
   Inspector survives virtualization. Complete caller parity and truthful native playback feedback.
4. Native layer/group/final pixels, ordering/blend/alpha/interruptions, source loss/Undo/recovery,
   shared consumers, bounded sparse large-grid plans, lifetime and owner retirement under revised behavior.

M11/8 separately verifies opt-in Layer/Group output exposure and media snapshot/name feedback
in Cells. Existing default-output/model evidence does not prove those changed contracts.

### M08 — Deliver reusable optional app control

Remove the **Applications panel**, standalone authoring workflow and obsolete association UI.
Keep a reusable MapGyver app-control system on Golden's generic owned-process primitive,
available to future media; sequences remain outside scope. Columns/Cells have an optional
executable **App file**; show other controls only when populated. Arguments, environment and
other collections are typed child nodes.

Share one owned runtime for identical normalized executable plus launch/control settings.
Combine demand across Cells, Columns and Matrices. Cell pre/post distances are relative to its
Column position, clamped and non-wrapping. Direct Cell activation and outgoing transitions retain
demand. Preserve readiness, timeout, restart/backoff, cancellation, project fencing, graceful/
forced stop, tree cleanup and final-render-use retention. Never adopt/kill unrelated processes by name.

1. Retain the public app-neutral Golden IO primitive: explicit executable/arguments/environment/
   directory, suspended launch/Job Object before resume, cancellation/drop safety, truthful
   root/tree/unsupported/access outcomes and bounded process retirement.
2. Reusable MapGyver configuration and runtime-key boundary, typed conditional Column/Cell settings,
   removal of standalone Applications/associations, aggregate normalized demand and neighbor
   semantics through jumps/reorder/delete/direct activation; fake-time/process and history/project checks.
3. Actual owned-process readiness/first-frame, timeout/crash/backoff, shared-demand deduplication
   without transient restarts, launch cancellation, final-render retention and stale-project rejection.
4. Complete UI/script/headless commands/callbacks/snippets and actual app picker/lifecycle,
   singleton/external/elevated/launcher/breakaway outcomes, project/shutdown/recovery/cleanup.
   Retain unverified retirement/access-denial cases; authored Undo is distinct from OS side effects.

### M11 — Resolve the 2026-11-03 feedback before Video and final qualification

MapGyver owns media, Matrix, Screen/Surface and test-pattern policy under `apps/mapgyver`.
Golden owns reusable GPU admission/adapter/preview mechanisms, generic Inspector presentation
and shared viewport/workbench controls. App registration supplies branding and product behavior;
do not put MapGyver-specific policy or assets in Golden packages.

Source: the user's **MG Feedback.txt** (23 top-level bullets), reviewed with `flashes.mp4` (2.03 seconds,
852×954, 30 fps), `bad view item.png`, `Screenshot 2026-11-02 152703.png` and
`Screenshot 2026-11-02 154226.png`. The video illustrates repeated Inspector/channel expansion
and transient redraws; it does not establish a root cause. The captures show the long display
device path, existing edge-positioned navigation and the desired Golden graph toolbar reference.
The supplied root `mapgyver_icon.png` and `apps/mapgyver/ui/src/lib/icons/nodes/*.svg` are the
asset sources. These are reported defects/design references, not passing runtime evidence.

Each numbered gate requires its complete listed behavior and affected caller/history/project
and lifetime checks; do not award partial acceptance. Execute in order, in bounded coherent
slices, with one active gate. M11/4 establishes Fit Mode for current applicable sources and the
contract M10 Video will consume. M11 never requires implementing Video ahead of M10.

1. **Stable interface and preview transitions.** Owner: shared Golden workbench/Inspector and
   native-preview/host lifecycle, with app integration. Reproduce the supplied interaction on
   a recorded build before choosing the fix. Clicking controls, expanding/collapsing Inspector
   folders and clicking Matrix Cells must not flash unrelated UI. Media selection changes must
   not tear down/reload the panel or show a transient blank/loading flash; preserve valid native
   content until its replacement is ready and fence stale selections/project generations.
   Verify repeated recorded real-input/native-frame sequences, independent live outputs,
   rapid selection, tab close/reopen, reconnect and New/Load. A DOM-only test cannot close this
   gate. Prior permission for a brief unavailable preview on restore does not waive these defects.
2. **Useful concurrent 4K capacity with bounded ownership.** Owner: Golden Video resource
   accounting/admission/native allocation and MapGyver demand projection. Reproduce the reported
   4096×4096 Solid Color rejection: charge **138481672 B**, reserved **405631384 B**, limit
   **536870912 B**. Determine real texture/pool costs, shared-target reuse, live/prepared/retiring
   overlap and release before changing policy. Replace an arbitrary fixed 512 MiB product ceiling
   with adapter-aware admission and explicit safe headroom; do not hide allocation failures,
   remove accounting or duplicate resources per consumer. The planned discrete-GPU fixture is
   **at least eight independently demanded 4096×4096 Solid Color outputs**, plus Reference aliases
   and live replacement, with exact formats/counts/steady and peak reservations recorded.
   Verify native content, sustained bounded memory, overload rejection/recovery and final-use
   retirement. Also exercise the integrated adapter against its reported safe capacity; record
   hardware limits explicitly. This new capacity requirement supersedes the fixed allocation
   ceiling only; timing, private-growth and retirement budgets remain required.
3. **Discrete GPU by default, explicit user choice.** Owner: reusable Golden adapter enumeration/
   selection and shared desktop host, registered through MapGyver options. Prefer a suitable
   NVIDIA/AMD high-performance GPU over an integrated adapter on hybrid/Optimus laptops.
   Expose a persisted typed application option for automatic preference or an explicit available
   adapter, report the actual active GPU and any restart requirement, and handle missing selected
   devices/recovery truthfully. Respect explicit choice; never silently move a shared-texture
   connection across incompatible adapters. Verify cold starts, both available GPUs, persisted
   selection, unavailable preference and recovery; controls must agree with the rendering device.
4. **Minimal typed media/channel Inspector.** Owner: MapGyver media Params/authoring and Golden
   generic Inspector hooks. Remove user-facing Kind/SourceKind and generic Filter/Sampling
   Parameters, keep backend kinds typed and default to Linear. Use expanded compact channel rows:
   one Color per Solid Color channel, one File per Picture channel, and one File per future Video
   channel; ordinary edits must not require opening nested Parameters folders. Preserve real
   child nodes, labels, CRUD, bounded inspection/virtualization and shared selection. No authored
   per-channel size. Add a typed media-level **Fit Mode (Fit / Fill / Stretch)** only where source
   sizes can differ; hide it while Resolution is automatic and omit it for Solid Color. Verify
   different-aspect Picture channels in all modes against native pixels and a single forced
   Resolution; Auto preserves source-native extents. All time-related Parameters across existing
   media, transitions and app control use Golden's existing time-style UI and preserve units.
   M10 must apply the same style to Video. Cover generic/API/script/headless/history/copy/project
   parity; change versioned schemas, codegen and consumers together where contracts change.
5. **Shared Texture Manual/Auto discovery.** Owner: MapGyver typed media/channel authoring and
   source supervision; Golden supplies live sender discovery and reusable Parameter adornments.
   A media-level Manual/Auto setting chooses the channel control. Manual uses a String Parameter
   with inline **Detect**, showing a live sender list and setting that same String on selection.
   Auto uses only a dynamic enum Parameter populated from available senders, retaining a missing
   selected value through absence and automatically recovering when the exact sender returns.
   Sender-list changes update open controls without resetting authored choice. Verify multiple
   channels, appearance/disappearance/restart/rename, mode changes, isolated failure, adapter
   mismatch, history/project roundtrip and equivalent script/headless edits. Discovery and
   recovery stay off the engine/UI thread; do not implement reconnect policy in UI stores.
6. **One workbench canvas/navigation experience.** Owner: shared Golden viewport/graph/workbench
   controls; MapGyver registers contextual actions. Remove every extra internal header from
   Matrix, Composition, Surface and Screen panels and remove Refresh View. Reuse Golden graph's
   panel/view navigation, manipulation mechanisms and shortcuts wherever applicable. If graph
   domain assumptions prevent whole-view reuse, extract/reuse its public toolbar/navigation/
   shortcut components; do not build another lookalike. Put selection-dependent icon actions
   at the **top center, on the same line as navigation/zoom**, following the supplied graph
   toolbar reference. Preserve editor-specific domain gestures, keyboard focus, smooth anchored
   zoom, native/DOM alignment, reduced motion and pointer/history lifetime. In Screen editor,
   unselected Surfaces show a thin border with **no blue fill**; only selected Surfaces show a
   lighter overlay, slightly thicker border and handles. Hover must not reintroduce selected
   decoration. Verify all four panels and Golden graph, multiple selections, narrow panels,
   native composition and the retained invalid-quad indication without covering unrelated areas.
7. **Create owned media directly in Composition.** Owner: MapGyver backend catalog/child-media
   creation intents and Composition editor context menu. Right-clicking the canvas offers every
   catalog media type supported by Composition, creating an actual owned media child with
   backend defaults, placement, identity and selection. This coexists with default Reference/
   Alt-copy drops; it must work without a pre-existing item in Medias. Reuse atomic detached
   trees, cycle validation and the public authoring boundary. Verify actual menus, native output,
   complete module commands/functions/callbacks/snippets, script/headless callers, single-gesture
   Undo/Redo, copy/reorder/delete and project roundtrip; unavailable types have truthful states.
8. **Matrix source feedback and real Group outputs.** Owner: MapGyver Matrix Parameters,
   authoring/rendering/catalog and virtualized grid. Populated Cells show a media preview snapshot
   and label **`media [channel]`**, rather than just the channel. Reuse bounded preview leases/
   snapshots for visible Cells; no per-cell source/decoder duplication or frame-rate engine
   polling. Layers have a typed **Expose Channel** option, **off by default**. Groups are actual
   compositing owners with typed **Blend Mode** and the same opt-in channel exposure, not folders.
   Existing blend fields must be wired/verified rather than duplicated. Backend output discovery,
   References and demand honor exposure; internal Layer/Group composition and Final output
   continue without exposure. Verify nested Group blend/alpha pixels, default and toggled
   discovery, stable identities/history, existing/missing Reference recovery, copy/projects and
   large sparse viewport/source-loss/preview retirement. Do not accept a UI-only channel filter.
9. **Consistent identity, tabs, outliners and footer.** Owner: MapGyver registration/assets/UI
   contributions and public Golden workbench hooks. Use the supplied node SVGs for Media rows
   and corresponding panel tabs. Generate app/window/taskbar/installer/package icon variants
   from root `mapgyver_icon.png`, including the test-pattern asset; replace placeholder MapGyver
   branding. Rename **Screen Manager → Screens** and **Media Manager → Medias**. Screen rows show
   the display's human-readable name, never its long internal ID/path; retain stable backend IDs
   and truthful missing-target state. Matrix, Composition, Surface and Screen tab titles follow
   the currently displayed object as Inspector does, respecting locked/independent panels,
   rename, selection and document lifetime. Footer statistics align **right**, contain no `...`
   or `submit` wording, present durations in **ms** and memory as **MB/GB**, replacing MiB with
   matching numeric conversion. Preserve actual observation meaning and accessible compact
   unavailable/recovering states. Verify native/app/package identity, tab/outliner interaction,
   narrow layouts and footer states through the existing public contribution hook.
10. **Official default Surface test pattern.** Owner: MapGyver app-owned pattern policy/assets,
    using reusable Golden rendering/time/text/image primitives. Center the MapGyver icon, put
    **surface resolution and actual FPS below**, and **current system time with milliseconds
    above**. Draw alternating dark/light grey square cells adapted to output aspect, subtle
    row/column labels such as A1/B2, and **thin rainbow grid edges moving diagonally**. The
    **second row from bottom** is a color-test palette; the **second row from top** is a
    black-to-white brightness palette. Include a correctly proportioned fit circle touching
    the limiting canvas edges and both corner-to-corner diagonals. It is the default pattern
    for Surface calibration and works with no source; leaving calibration restores routing.
    Verify landscape, portrait, square, 4K and resize/warp/native output, text readability,
    square/circle proportions, palette locations, live milliseconds/FPS and bounded animation
    demand. Sample wall time at an IO/render boundary; keep animation on the shared runtime
    clock and preserve deterministic geometry/palette tests without main-thread polling.

Close each gate with the relevant software/strict/codegen/UI checks before native acceptance,
and update affected architecture docs with implementation. M09 reruns the changed full-app
workloads; M11 does not waive retained physical, timing, codec or device-removal requirements.

### M10 — Deliver Video playback in codec tiers

Start only after **M11/1–10 are Accepted**. Video has **one transport per media and one file
per channel**; References only mirror outputs. Reuse M11's simplified Channels, shared Fit Mode,
time-style Parameters, icons and stable-preview contracts rather than restoring obsolete controls.
Use a shared monotonic media timeline and timestamp-based sampling. Defaults: paused at zero,
rate 1 and looping off. Shorter channels hold their final frame until the shared timeline ends.
Frame stepping uses the selected channel's timestamps; References never independently reset transport.

Use FFmpeg libraries through reliable Rust bindings for demuxing/broad decode and D3D11 hardware
frames where supported. Select/pin the actual bindings and redistributable build before coding
that boundary. Keep hardware frames GPU-resident. HAP performance playback uses the reference
decoder's compressed-texture route. Primary integration sources:
[FFmpeg codecs](https://ffmpeg.org/ffmpeg-codecs.html),
[FFmpeg D3D11 hardware contexts](https://github.com/FFmpeg/FFmpeg/blob/master/libavutil/hwcontext_d3d11va.h)
and [HAP developer guidance](https://hap.video/developers).
Library capability is not acceptance of any codec/profile in MapGyver.

1. **Transport/common codecs:** H.264, H.265 and ProRes; play/pause/stop/seek/frame-step/rate/loop;
   mixed-rate timestamp sampling, shorter-file hold and shared Reference transport. Complete
   Parameters, commands/functions/callbacks/snippets and UI/script/headless/history/project parity.
2. **Performance playback:** HAP, HAP Alpha, HAP Q, concurrent high-resolution files and shared
   consumers; compressed textures, alpha/color correctness and one decoder/render owner.
3. **Extended compatibility:** DXV and WebP, including animated WebP. Record codec/profile variants
   individually; unverified variants remain explicit follow-up requirements within this gate.
4. **Bundled integration:** pinned redistribution/build/license dependencies, exact seek fixtures,
   bounded file/decode workers and queues, coalesced superseding seeks/stale-result rejection,
   missing-file relink/retry, device recovery and actual-app sustained 1080p/4K/resource performance.

Use frame-numbered fixtures for exact seeks, mixed frame rates and stale-seek rejection.
Record cold/warm seek latency, timestamp accuracy, sustained throughput and per-format software
upload cost. Do not start native acceptance until prerequisite software/build checks succeed.
Audio, effects, mesh warp, edge blending, streaming and cross-platform/cross-adapter guarantees
remain outside this plan.

### M09 — Qualify the actual app and Windows artifact

**Gate 1:** after all M11 feedback gates, qualify the complete revised bundled Windows x64 product
and all retained M03–M11 requirements, not only a static render or native probe. Run exact
binary/assets/dependency
identity and license checks, Rust/UI/codegen/public-boundary and affected Chataigne regressions,
Q01/Q02 prescribed release series, revised Q03 soak, Q04 authored/active scale and full Q05
recovery/retirement. Include seven media kinds, Media Preview, revised editors, all three output
modes, optional app control and footer statistics through a public Golden contribution hook:
actual video FPS, active GPU, frame time and resource usage. Publish bounded worker observations.

The [qualification contract](../progress/mapgyver-qualification-contract.md) explicitly replaces
the obsolete two-channel Composition fixture with single-output Composition and Reference use,
adds Video and separately measured software-decode uploads. Preserve GPU-sharing requirements,
all existing numeric budgets, historical failures and unavailable configurations. No unapproved
GPU-work/window-timing proposal becomes operative through this merge.

A reviewable packaged artifact, onboarding and reproducible app-specific run/package commands
are required. Release/signing/publication need a separate instruction.

## Verification and status maintenance

At every coherent milestone run relevant owning Rust tests/strict Clippy, generated-contract
drift checks, Svelte/UI checks and affected Chataigne regressions. Use meaningful tests beside
their owner under `tests/`. Run formatting at the root and Golden Core manifests. Native/
hardware validation follows inspected terminal success of its compile/software prerequisites.

Keep [mapgyver-status.md](../progress/mapgyver-status.md) a compact **current-state page**:
overall and phase/subphase bars, verified counts, one active gate, a return queue with concrete
blockers/triggers, current usable capability, next deliverable and one replaceable batch entry.
Update it in place; do not append dated reports, command transcripts or historical dashboards.
Put necessary detailed evidence in an owning report or small evidence artifact and link it.
Progress counts only complete verified gates; partial/checkpointed/unavailable work stays open.
Document denominator changes and retain prior evidence without treating it as proof of changed code.
