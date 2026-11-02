# MapGyver plan revision: media authoring, editors and video

> Merged on 2026-11-02 into [MapGyver_Implementation_Plan.md](MapGyver_Implementation_Plan.md).
> This proposal is retained as source context; the merged plan and compact status page govern execution.

## Summary and execution order

Adapt [MapGyver_Implementation_Plan.md](MapGyver_Implementation_Plan.md), keeping it as the execution entrypoint. Synchronize the architecture and [progress ledger](../progress/mapgyver-status.md).

Prioritize this task list after completing the current unrestricted Surface-warp correction. Deliver in this order:

1. Media/channel ownership, Resolution and Reference media.
2. Media Preview, shared navigation and Screen interaction improvements.
3. Composition ownership and transform controls.
4. Reusable app control and Matrix redesign.
5. New **M10 Video playback**, delivered in codec tiers.
6. **M09 final qualification**, covering the revised product and all retained acceptance gates.

Revise M04–M08 in place; retain applicable evidence and reopen acceptance affected by changed requirements. Preserve unfinished timing, memory, recovery and physical-display gates. New requirements begin unverified, with explicitly revised acceptance counts.

## Media model and preview

- Channels remain stable concurrent outputs, presented as **variants within a media’s Channels section**. Hide them from the Media Manager outliner and remove channel creation from its `+` actions. Keep channel editing on real typed child nodes.
- Move shared settings onto the media. Variant fields contain only differences: Solid Color’s color, Shared Texture’s sender, Picture’s asset and Video’s file. Composition initially exposes one output; channel retargeting remains future work.
- Give every media a disableable **Resolution** vector in pixels, disabled by default. Use Golden’s typed Vec2 parameter with integer constraints, unit steps and backend validation, retaining the current 1–16384 bounds. Disabled means source-native size, or 1920×1080 for procedural canvases. Enabled forces every channel to that extent.
- Remove Shared Texture’s authored pixel format, color encoding, alpha mode and maximum dimensions. Discover format/encoding at the native boundary, use straight alpha, and retain internal allocation limits and recovery.
- Remove the useless `+` from declared Parameters folders through correct container capabilities.

Add **Reference media**:

- A typed media reference mirrors the target’s channels and rendered content. It owns no source settings, transport or decoder.
- Normal media drops into Composition or Matrix create Reference children. **Alt+drag copies** the dragged media subtree with independent identities.
- Synchronize mirrored channels using target channel identities; retain explicit missing-reference states and reject cycles across Reference, Composition and Matrix.
- Share target rendering. An explicitly enabled Reference Resolution uses a shared-source resampling pass.

Add a dockable **Media Preview** panel:

- Follow the selected media or media descendant; provide channel selection and an all-channel grid.
- Support multiple instances, each with independent lock, channel choice and panel state.
- Remove media previews from the Inspector.
- Reuse native preview leases and retirement. Closing or locking panels must not interrupt independent outputs.

All authored settings use Golden Parameters and the generic Inspector. Update Rust-owned DTOs, generated TypeScript and all consumers together.

## Screen and Composition editors

Shared Golden viewport behavior:

- Animate zoom toward the requested scale while keeping the pointer anchor stable.
- Apply it to Screen, Surface Source and Composition editors.
- Keep native imagery, overlays and hit testing aligned throughout animation; cancel obsolete animation on disposal or document replacement.

Screen editing:

- Surface clicks select the actual Surface in shared selection and Inspector. Background clicks clear Surface and handle selection.
- Show handles only for selected or hovered Surfaces.
- Track individual handle selection as editor state while retaining the owning Surface in Inspector.
- Arrow keys move selected handles by **1 output pixel**, or **10 with Shift**. Held keys repeat; a held interaction forms one undoable gesture.
- **Alt+drag** translates all four corners together.
- Add contextual **Fill Screen** and **Fit to Screen** actions. Fill stretches to screen bounds; Fit centers the source with its aspect ratio preserved. Both are backend operations that update the authored quad.
- Preserve finite invalid intermediate warps and safe invalid-render feedback from the current correction.
- Add an in-place native output bounds update. Rectangle position, size and display reassignment must keep the output window visible, preserving its identity and safe GPU retirement during resize.

Composition:

- Follow the selected Composition, with per-panel locking and multiple independent panels. Remove the redundant internal header and creation/refresh controls.
- Make canvas items actual child media nodes. Store placement, scale, pivot, rotation, opacity and blend as typed parameters on those children; replace reference-only instance authoring.
- Corner resizing keeps the opposite corner fixed. Edge handles resize along one axis.
- Rotate from outside corner handles with a rotation cursor; remove the current anchor-adjacent rotation control.
- Express authored rotation and public edit values in **degrees**; convert at the rendering boundary.
- Keep selection, geometry calculations, structural edits and history authoritative in the backend.

## Matrix and optional app control

Replace the Matrix’s internal header and custom property forms with a full-panel virtualized grid and generic Inspector.

- New Matrices start with **three layers and three columns**.
- Use square cells, clearer layer/group separation and consistent runtime/selection markers.
- Support drag sorting/reparenting of layers and groups, column sorting, and double-click rename.
- Empty cells offer a centered `+` and contextual Add Media. Filled cells offer a compact remove action. Each populated Cell owns its actual child media.
- Selection opens the corresponding Layer, Group, Column, Cell or media parameters. Inspector editing survives virtualization.
- Keep empty grid addresses sparse; materialize Cell nodes when content or authored settings require them.

| Owner  | Authored parameters and behavior                                                                                                                             |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Layer  | Position, size, opacity, blend mode and transition time. Size defines the layer canvas; cells retain their media’s size.                                     |
| Column | Optional transition override, App file, and conditional app-control settings including preload/postload integers.                                            |
| Cell   | Position relative to its layer, opacity, enabled state, optional transition override, App file and conditional app-control settings. No Cell size parameter. |

Resolve transition duration as **Cell override → Column override → Layer duration**. Preserve the existing default duration of 250 ms. Disabled cells leave the layer’s active cell unchanged. Enabled empty cells retain the existing transparent transition policy.

Add explicit individual Cell triggering without changing the current Column anchor. Column launches remain atomic across participating layers.

App control:

- Remove the **Applications panel**, standalone authoring workflow and obsolete association UI.
- Retain a reusable MapGyver app-control system, backed by Golden’s generic owned-process primitive. Future media systems can attach it; sequences are outside this plan.
- Columns and Cells expose an optional executable **App file**. Show the remaining controls only when it is populated.
- Keep arguments, environment entries and other editable collections as typed child nodes.
- Share one owned runtime instance for identical normalized executable and launch/control settings; combine demand across Columns, Cells and Matrices.
- Apply Cell preload/postload relative to its Column’s position. Direct activation and outgoing transitions also retain demand.
- Preserve readiness, restart/backoff, cancellation, project fencing, shutdown and final-render-use retention. Never adopt or terminate unrelated processes by executable name.

Add app footer statistics through a public Golden contribution hook: actual video FPS, active GPU, frame time and resource usage. Add responsive Screen outliner supplements for output/target, resolution and FPS, and Surface supplements for source media. Publish bounded runtime observations rather than polling the engine at frame cadence.

## Video, acceptance and delivery

Add **Video media** with one transport per media and one file per channel. Reference media simply mirrors its rendered outputs.

Use FFmpeg libraries through Rust bindings for demuxing and broad decoding, with D3D11 hardware frames where supported. Use the HAP reference decoder’s compressed-texture path for performance playback. These are established integration paths documented by [FFmpeg](https://ffmpeg.org/ffmpeg-codecs.html) and [HAP](https://hap.video/developers).

Deliver M10 through separate acceptance gates:

1. **Transport and common codecs:** H.264, H.265 and ProRes; play/pause/stop, seek, frame stepping, rate and looping.
2. **Performance playback:** HAP, HAP Alpha and HAP Q, concurrent high-resolution files and shared consumers.
3. **Extended compatibility:** DXV and WebP, including animated WebP. Unverified variants remain explicit follow-up gates.
4. **Bundled integration:** packaging, recovery, seek accuracy, resource bounds and actual-app performance.

Use a shared monotonic media timeline with timestamp-based channel sampling. Defaults are paused at zero, normal speed and looping off; shorter files hold their last frame until the shared timeline ends. Frame stepping follows the selected channel’s frame timestamps. Reference use never independently resets the original transport.

Decode and file access run on bounded workers. Coalesce superseded seeks, reject stale results, support missing-file relink/retry and recover from device loss. Keep hardware-decoded frames GPU-resident; measure bounded uploads for software-decoded formats separately. Audio remains outside scope.

Acceptance must cover:

- Parameter editing, UI/script/headless parity, commands, callbacks, snippets, undo/redo, duplication and project roundtrip.
- Reference synchronization, missing targets, mixed-media cycles and one shared rendering/decoder owner.
- Multiple locked previews and Composition panels without shared editor-state interference.
- Real pointer/keyboard checks for handles, smooth zoom, transform anchors, sorting, renaming and continuous rectangle output movement.
- Matrix transition precedence, disabled cells, independent triggers, sparse scale and combined app demand through outgoing retirement.
- Frame-numbered video fixtures for exact seeks, mixed frame rates, superseding seeks, codec/alpha correctness and simultaneous 1080p/4K playback. Record cold/warm seek latency and sustained throughput against the existing timing/resource budgets.

Revise the full-app qualification fixture explicitly: replace its obsolete two-channel Composition requirement with the single-output Composition and Reference workflow. Add Video workloads and a separate software-decoding upload allowance; preserve the existing Spout/GPU-sharing requirements.

Commit and push each coherent milestone on `mapgyver` after relevant checks and ledger updates. Run generated-contract checks, Rust tests/Clippy, Svelte/UI checks, affected Chataigne regressions, and formatting at both root and Golden Core. Native acceptance follows successful software prerequisites.

No legacy compatibility layer is planned. Changed document structures require explicit version handling and clear rejection of unsupported old records. Planning itself awards no implementation progress.
