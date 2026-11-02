# MapGyver architecture

MapGyver owns media authoring, projection mapping, Composition, Matrix and optional app control.
Golden supplies reusable execution, host and workbench mechanisms. Windows x64 is the first
target. This document describes the **revised target architecture**, merged on 2026-11-02;
implementation and verification state belong only to the
[status page](../progress/mapgyver-status.md). Execute the
[implementation plan](../plan/MapGyver_Implementation_Plan.md).

## Ownership

| Layer                                             | Responsibility                                                                                                                    |
| ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `apps/mapgyver/src/app`                           | Thin identity, registration, lifecycle and public Golden host composition                                                         |
| `apps/mapgyver/src/media`                         | Media/variant/Reference/Composition/Video document policy, backend operations, dependency validation and render-plan projection   |
| `apps/mapgyver/src/screens`                       | Screens/Surfaces, source crops, finite authored warps, calibration and output policy                                              |
| `apps/mapgyver/src/matrix`                        | Sparse layers/groups/columns/cells, atomic playback, transitions and app-demand contributions                                     |
| MapGyver app-control system                       | Reusable app-owned configuration/runtime-key/demand/readiness policy, independent of an Applications panel or Matrix-only service |
| `apps/mapgyver/ui`                                | Product manager registrations and specialized Media Preview, Screen, Surface Source, Composition and Matrix panels                |
| `crates/golden_video`                             | App-neutral render DAG/runtime, D3D11/Spout resources, bounded native presentation, frame/decoder integration mechanisms          |
| Golden Time / Assets / IO                         | Shared monotonic time, persistent asset locators/preparation, bounded workers/cancellation and generic owned processes            |
| Golden Core desktop host                          | Ready-to-run bootstrap, native windows/dialogs, COM coordination and optional presentation lifecycle                              |
| `packages/golden-ui` / `packages/golden-video-ui` | Workbench/outliner/Inspector/selection, docking, shared viewport behavior, contribution hooks and preview leases/geometry         |

Neither Golden nor MapGyver imports Chataigne's private modules or Alchemist. Shared mechanisms
are public, app-neutral APIs. Persistence formats belong to persistence/protocol owners, not
host bootstrap. Codec libraries are integrated through supported APIs/bindings rather than a
new handwritten protocol/decoder.

## Authored document and operations

Every setting is a real typed Golden Parameter; editable lists contain real child nodes.
Identity, hierarchy, labels and ordering use Golden topology/metadata. Definitions/DTOs are
validated inputs or read/runtime projections, never parallel persisted settings. See
[parameter ownership](mapgyver-parameters.md).

Backend intents own defaults, unique labels, geometry, reference/cycle validation, structural
edits and playback. Generic Parameter changes enforce the same ancestor invariants; cross-field
changes use atomic Parameter edits. Known subtrees are created/imported as detached trees in one
operation, including fresh media and the three-by-three Matrix default. Undo groups represent
whole gestures, not individual input samples. Copy captures complete Parameter/control subtrees
with independent node identities.

Media kind labels and source defaults are backend-owned and shared by the generic creation
catalog, explicit app intents and Picture import. Each creation/import prepares a complete
detached subtree; the app operation and generic creation routes validate the same ownership
rules and commit one history step.

Rust owns DTOs and generated TypeScript. UI, transport/headless callers, command nodes, script
functions, callbacks and app-owned snippets use the same operations. Structural schema changes
receive explicit version handling and clear unsupported-record rejection; no compatibility
shim or silent reinterpretation is planned.

## Media, channels and Reference ownership

Channels are stable **concurrent output variants** within each media's Channels section.
They are hidden from the Media Manager outliner and its creation actions, while remaining real
typed editable nodes. Persisted media folders organize the actual document. Shared media settings
live on the media; channel fields contain source differences only.

`ChannelsSection` in the app Inspector presents at most 32 actual channel nodes per page,
using Golden's generic child inspectors and context menu. The Media Manager filters out
channels and only folder rows carry an inline creation button. Its row context menus use Golden's
public `canCreateNodeChildren` predicate: folders expose Add/Paste, media rows omit child creation.
Channels retain their real backend factory and Inspector creation actions. Channel snapshots are queried by
exact reference when expanded, with session/project fencing; collapsed variants perform
no inspection reads. Backend `MoveChannel` owns relative ordering, and source-channel
catalog entries preserve the current media selection when creating a variant. Composition
has one owned output; its output and Matrix exports do not advertise source-variant creation.

| Media          | Variant/source ownership                                                                 |
| -------------- | ---------------------------------------------------------------------------------------- |
| Solid Color    | Per-channel color/alpha                                                                  |
| Picture        | Per-channel asset reference; worker decode and admitted initial upload                   |
| Shared Texture | One exact Spout sender per channel; native-discovered format/encoding and straight alpha |
| Reference      | Typed target media reference; mirrors target channels and rendered content               |
| Composition    | Ordered owned child media; one output initially                                          |
| Matrix         | Actual media children in sparse Cells; derived layer/group/final outputs                 |
| Video          | One shared media transport; one file per channel                                         |

Every media has a disableable integer-constrained Vec2 **Resolution**, in pixels, default
disabled, bounds 1–16384 per axis and unit steps. Disabled uses each source's native size;
procedural canvases default to 1920×1080. Enabled forces every channel to that extent.
The shared-Resolution/Sampling document uses node type `mapgyver.media.v5`. Projects containing
older media types are rejected as unsupported; per-channel sizing or sampling is never silently
reinterpreted. `ChannelDefinition.size` and `.filter` are runtime/read projections. Change Resolution
through its generic Parameter (including enabled state) or `ConfigureMediaResolution`;
change shared Sampling through its generic Enum Parameter or `ConfigureMediaSampling`.
`ConfigureChannel` accepts only source settings. Media revision guards invalidate captures
for pixel changes and enable-only edits without walking or rebuilding all channel subtrees.
Procedural Composition/Matrix canvases project this same Resolution, falling back to 1920×1080
when disabled. They have no channel canvas Parameters. Matrix transition/empty-cell settings
also belong to media Parameters; `MatrixEdit::Configure` accepts only `MatrixSettings`, while
`MatrixDefinition.canvas` is a read projection. Document reads, placement, playback and rendering
use the owning media projection. Derived output sources cannot be configured as source variants.
Changing procedural Resolution updates canvas coordinate extents and preserves authored transform values.
Shared Texture no longer persists format, encoding, alpha mode or maximum dimensions.
Only the exact sender selector belongs to its channel Parameters. Golden discovers supported
BGRA/RGBA byte storage at the native receiver. Classic Spout does not carry independent
transfer/alpha metadata; the discovery profile interprets byte colors as sRGB with straight
alpha. Unsupported formats fail explicitly. Source extents remain bounded by the render host's
texture limits. The existing aggregate source byte budget admits actual discovered bridge
pools before allocation, including pools still retiring; declared Golden subscriptions retain
their explicit metadata and maximum reservation. No admission budget is increased.
Internal device/allocation limits still reject unsupported resources and supervise recovery.

References preserve target-channel correspondence by stable channel identity, not position
or label. They own no decoder, source settings or transport. Default drops into Composition/
Matrix create Reference children; Alt+drag copies the media subtree independently. Missing
targets remain explicit and recover without rebinding to a different source. Reference
Resolution, when enabled, adds a resampling pass over the shared target rendering.

A single directed dependency model spans Reference, Composition and Matrix. Reject direct/
indirect feedback at backend edits, raw Parameter changes and load. Multiple consumers share a
target's render and decoder owners. Edits, deletion/Undo, resize/reconnect and project replacement
preserve identity while runtime generations fence stale preparation.

## Rendering and native lifetime

The retained native route is ordinary Tauri/Wry underlay, D3D11 and the pinned Spout binding.
[Native research](mapgyver-native-rendering.md) explains the functional decision and continuing
DPI/physical limitations. That decision is not production qualification.

Golden Video compiles immutable demanded DAGs with explicit storage/color/alpha, normalizing
internal composition to linear premultiplied alpha. Workers prepare sources/plans, apply
project/revision/device fences and install atomically at frame boundaries. Keep one preparation,
one latest desired plan and at most two retiring generations; coalesce or reject pressure.
No engine-loop GPU polling or recurring main-thread source processing is permitted.

Charge prepared/live/in-flight/retiring resources and source/output pools until actual final GPU
use. Shared aliases do not duplicate owned allocation charges. Native handles/pixels remain at
native boundaries. Cancellation invalidates results but cannot replace completion or prove a
blocked driver call has retired. Device loss, source failure and output failure have bounded
retry/backoff and truthful states while healthy paths continue.

Preview regions use native leases, coalesced geometry and matching installed/submitted-frame
acknowledgements before uncovering imagery. Closing/relocking a panel retires only its lease;
independent output demand continues. Layout acceptance alone does not prove visible frame
alignment. Browser-only sessions expose truthful native-preview unavailability.

## Screens, Surfaces and editor geometry

Screens target physical fullscreen displays, physical desktop rectangles or virtual Spout.
Persist desired display identity/recovery hints; discovery is bounded worker metadata.
Rectangle position/size/display changes update the existing visible window in place. The host
owns HWND lifetime and safe resize; GPU owners retain old resources through completion.

Surfaces own source references, crop, four finite corners, opacity, enabling and calibration.
Crop validation remains strict. Finite crossed/concave/degenerate quads remain editable with red
edges and dim red fill; rendering omits only unrenderable layers and resumes when valid.
Calibration can render without a source and later restore authored routing. Ordered overlapping
Surfaces preserve alpha.

Screen, Surface Source and Composition share Golden viewport navigation. Zoom animates toward
the requested scale with a stable pointer anchor; native imagery, overlays and hit testing share
the same transform. Disposal/document replacement cancels obsolete animation.

Screen clicks select the actual Surface for shared Inspector; background clicks clear Surface/
handle selection. Handles show only for selected/hovered Surfaces. Individual handle selection
is local editor state while Inspector keeps the Surface owner. Arrow repeat moves selected
handles by one output pixel, or ten with Shift, as one undoable interaction. Alt+drag translates
the whole quad. Backend Fill Screen stretches to bounds; Fit to Screen centers source aspect.

Surface Source uses the full uncropped source preview with independent crop overlays and the
same typed authoring/history. Source loss preserves routing; it never substitutes another channel.

## Composition

Composition items are actual child media rather than reference-only instances. Placement, scale,
pivot, rotation, opacity and blend are Parameters on those children. References provide normal
shared-source placement; Alt-copy gives an independent source subtree. Composition exposes a
single output initially; channel retargeting is future work.

Authored/public rotation is in degrees, converted at rendering. Backend geometry fixes the
opposite corner during corner resize, limits edge resize to one axis and rotates from outside
corner handles. Remove the anchor-adjacent rotation control. Signed/nonuniform scale, pivot and
source alpha remain independent. Shared source rendering does not collapse child transforms.

Multiple Composition panels independently follow selected Composition or lock and restore their
state. Use the whole panel canvas and compact icons; remove redundant internal header and
creation/refresh controls.

## Matrix and optional app control

Matrix has stable layers/groups/columns/cells and layer/group/final output identities. New
Matrices start with three layers and three columns. Empty addresses remain sparse; content or
authored Cell settings materialize a Cell node. Populated Cells own actual child media.

Layers own position, canvas size, opacity, blend and transition duration. Cell media retains its
own size; Cells have relative position, opacity, enabling and optional transition/app settings,
with no Cell size. Columns have optional transition/app settings. Groups retain typed opacity/
blend/enabling and real hierarchy. Transition precedence is Cell override, Column override,
then Layer duration; default remains 250 ms.

Column launches commit atomically. Individual Cell triggering leaves the Column anchor unchanged.
Disabled Cells preserve their active layer; enabled empty Cells transition to transparency.
Interrupted transitions retain outgoing source/app demand through final rendered use.

The app-control system is reusable MapGyver policy on Golden IO owned-process primitives.
Remove standalone Applications authoring/panel and association UI. Column/Cell App file is
optional; conditional Parameters and typed argument/environment children configure it.
Normalize executable and all launch/control settings into a shared runtime key; aggregate demand
across Columns, Cells and Matrices. Cell preload/postload is relative to its Column position,
clamped without wrapping. Direct activations and outgoing renders also hold demand.

Launch suspended, assign owned Job Object before resume, and retain the owned tree through
readiness, timeout, crash/backoff, cancellation and graceful/forced final stop. Project/launch
epochs reject stale outcomes. Never adopt/terminate unrelated processes by executable name;
unsupported/singleton/elevated/breakaway outcomes stay truthful. Undoing authored settings does
not imply undoing an OS side effect.

## Workbench and bounded observations

Use Golden filtered manager trees, shared selection/context menus, generic Inspector, docking,
icons and tokens. Remove ordinary forms and Inspector media previews. The Matrix grid fills its
panel with virtualized square cells, drag sorting/reparenting, double-click rename, centered Add
Media for empty cells and compact remove for filled cells.

Bounded backend queries inspect selected definitions independently of visible grid rows and
canvas items. Keep one active read and one latest replacement, fenced by project/session/history.
Scrolling a selected object out of view must preserve Inspector editing without a whole-document
read or stale UI-owned definition.

Media Preview follows selected media/descendants, offers channel/all-channel grid and supports
multiple independently locked panels. Persist panel state separately from authored media.
Screen outliner supplements show output/target, resolution and FPS; Surface supplements show source.
A public Golden footer hook accepts app contributions for actual FPS, GPU, frame time and resource
usage. Coalesced worker observations drive presentation; no frame-rate engine polling.

## Video runtime

Video's per-media transport samples channels against one monotonic timeline. Defaults are paused
at zero, normal speed and looping off. Shorter files hold their last frame until the shared
timeline ends. Stepping follows the selected channel's frame timestamps. References neither
allocate another decoder nor independently reset the original transport.

Use FFmpeg libraries through pinned reliable Rust bindings for demux/broad decode, D3D11 hardware
frames where supported, and the HAP reference decoder's compressed-texture path for performance.
Keep file access/decode on bounded workers, coalesce superseded seeks, reject stale results and
support missing-file relink/retry and device recovery. Hardware frames stay GPU-resident;
software decode uploads are accounted separately without loosening Spout/GPU-sharing rules.

M10 delivers common codecs (H.264/H.265/ProRes), performance codecs (HAP/HAP Alpha/HAP Q),
extended compatibility (DXV/WebP including animation), then bundled recovery/performance.
Codec/profile support requires actual fixtures; library capability is not a product pass.
Audio is outside scope.

## Qualification

The [qualification contract](../progress/mapgyver-qualification-contract.md) owns workloads,
budgets and reporting. Preserve M03 timing/memory/recovery and M05 physical/DPI/input gates,
including historical failures and unapproved scope proposals. Revised Q03 uses a single-output
Composition and References; Video adds frame-numbered mixed-rate seek/concurrency fixtures and
a separate measured software-upload allowance. Authored scale is separate from active GPU load.

Software/null tests qualify policy, not native pixels, ownership release or physical alignment.
Compile/software prerequisites must pass before native/hardware experiments. Only verified
gates count toward the compact dashboard; unsupported/unavailable configurations remain open.
