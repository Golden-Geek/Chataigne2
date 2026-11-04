# MapGyver parameter ownership

Golden Core Parameter nodes own every authored setting. MapGyver owns schemas, validation and
specialized operations; Golden owns typed controls, references, Inspector, events, history and
persistence. DTOs are operation inputs or projections, never a second authored document.
This is the **revised target schema**, not a claim that its conversion is complete.
See the [plan](../plan/MapGyver_Implementation_Plan.md) and
[current acceptance](../progress/mapgyver-status.md).

Use Params DSL labels, constraints, typed references/enums/files and conditional folders.
Editable collections use real child nodes. Identity, labels, hierarchy and order use ordinary
Golden topology/metadata. Selection/zoom, panel locks and native observations are presentation/
runtime state. Declared Parameter folders must not advertise unsupported Add actions.

Time Parameters keep their stored/API milliseconds and declare `widget = "time", unit = "ms"`.
Golden's generic numeric editor displays a clock and converts edits back into those same units;
seconds-based Golden Parameters use `unit = "s"` (or the existing unspecified seconds convention).
The public Params DSL and `#[param]` forms share hint generation, including rebinding after load.
Matrix Layer/Column/Cell transition durations and all five app-control timing fields use this
metadata. No parallel UI store or project/schema conversion owns their values.

## Ownership inventory

| Owner                     | Required typed settings                                                                                                                   |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Media                     | Kind and disableable integer Vec2 Resolution in pixels, disabled by default, unit steps and 1–16384 bounds; shared source/output settings |
| Source variant            | Only source differences: Solid Color RGBA, Picture asset, Shared Texture sender, Video file                                               |
| Reference media           | Typed target media reference and inherited optional Resolution; target-channel synchronization is backend structure                       |
| Composition child media   | Placement, scale, pivot, rotation in degrees, opacity and blend on the actual owned child media                                           |
| Asset                     | One typed File; the backend derives a project-relative or native absolute locator                                                         |
| Screen                    | Enablement, target kind, display choice, physical rectangle, Spout name/extent and typed recovery hints                                   |
| Surface                   | Enablement, source references, crop, four finite corners, opacity and calibration                                                         |
| Matrix Layer              | Position, canvas size, opacity, blend, transition duration and enabling                                                                   |
| Matrix Group              | Opacity, blend, enabling and real hierarchy/order                                                                                         |
| Matrix Column             | Optional transition override, App file and conditional app-control fields with integer preload/postload                                   |
| Matrix Cell               | Relative position, opacity, enabled, optional transition override, App file and conditional app control; no size                          |
| App-control configuration | Arguments, directory, environment, window/readiness/sender, deadlines and restart policy; editable collections are typed child nodes      |
| Video media               | Shared transport settings/intents for play/pause/stop, seek/step, rate and loop; one file per channel                                     |

Shared Texture format/encoding is discovered natively; straight alpha and internal allocation
limits are runtime contracts, not editable channel descriptors. Disabled media Resolution uses
source-native size or 1920×1080 for procedural canvases. Enabled forces every channel's extent.
Composition initially has one output.

Default Composition/Matrix drops create Reference media; Alt+drag duplicates the media subtree
with independent identities. A Reference owns no source/transport/decoder settings. Empty
Matrix addresses stay sparse until content or authored settings require a Cell.

Remove opaque node-data codecs, JSON/string collections, ordinary custom property forms,
standalone Application definitions/association UI and Inspector media previews as their revised
owners ship. Old accepted field counts describe earlier implementations, not the revised schema.

## Validation, history and scaling

Golden ancestor validation applies domain invariants before committing generic Parameter edits.
Use the borrowed graph/proposed-value view rather than a whole-tree snapshot. Structural intents
and generic edits enforce the same cycle, geometry, reference and range rules.

Use `Edit::SetParamsAtomic` for distinct built-in Parameters with cross-field invariants.
Prepare all values, validate ancestors against the complete normalized proposal, then commit
without intervening callbacks. History replays the whole proposal. Gesture groups retain one
Undo operation while accepted samples can publish live feedback.

Surface quad validation accepts finite invalid intermediate geometry. Render preparation owns
homography executability; invalid crops still reject atomically. Composition geometry is
backend-authoritative; authored/public degrees convert only at the rendering boundary.

Complete subtree copy/import preserves values, defaults, controls and editable constraints with
independent identities. Loaded declarations rebuild producer constraints under Golden policy.
Explicit schema/version checks reject unsupported old opaque records; no silent compatibility path.

Params DSL bindings are scoped to their declaring owner, including reused folders. Immutable kind
dependencies should avoid materializing irrelevant fields. Known subtrees use detached batched
creation rather than sequential child edits and repeated snapshots.

The existing MapGyver materialization guard is 4,194,304 generated-work units with Golden's
unchanged 64-pass limit, applied before new/load/recovery/import expansion. This does not waive
memory/render/scale budgets or prove the revised schema meets them. Revalidate actual node counts
and bounded offscreen Inspector queries as new child ownership is delivered.

Ordinary properties use generic Inspector widgets. Specialized UI only supplies contextual
operations, native presentation or gestures against these same Parameters. Verify each converted
owner through generic editing, direct API/script/headless callers, rejection, Undo/Redo,
duplication, project roundtrip and applicable native behavior before counting its gate.

Picture catalog assets use `mapgyver.media.asset.v2`. Their single `parameters/file` Parameter
is authoritative across every referencing channel. Empty File values retain the asset identity
and bindings while leaving the source unconfigured. Native absolute paths and validated portable
relative paths project into the existing typed `AssetLocator`; classification and validation
belong to asset authoring, without filesystem IO. Import and relink update that same File.
Relink, clear, history and project reload retain its Parameter identity. The Rust-owned asset
node-type constant is generated for UI consumers. Previous split location/path asset records
and retired fields reject before Golden's recoverable decode; no persisted migration is provided.
The direct channel File-row presentation remains separate M11/4 work until verified.

App control uses `mapgyver.app_control.v1`: App file/directory are File Parameters;
window/readiness, stop/restart and environment inheritance are typed Parameters. Ordered
Argument and Environment nodes own their own String Parameters. `ApplicationDefinition`
is a validated input/read projection. Configure retains entries by ordinal, removes surplus
entries, applies one atomic Parameter proposal and appends missing subtrees in one forest.
Entry changes invalidate the owner's runtime revision through a snapshot-free mutation.
Obsolete `mapgyver.application` records are rejected before recoverable decoding.

App-control lifecycle snapshot requirements follow the work each stage performs. Fresh controls
materialize and bind their declared Parameters through child events; attachment requests a tree
snapshot only when descendants already exist and need rebinding. Empty init/ready callbacks do
not request snapshots. This keeps bulk Column/Cell creation from cloning the whole document
three times per settings owner while retaining loaded/replaced subtree binding. Inbox processing
keeps its existing shared snapshot contract for declaration reconciliation.

These settings stay materialized when inactive. Params DSL dependencies deliberately control
materialization and cannot represent reversible presentation toggles without missing write
targets. The generic Inspector's app visibility filter only chooses which existing widgets to
show; it never changes settings or runtime policy. Empty File values represent unconfigured
locators, so shared extension constraints permit clearing them; chosen paths retain extension
validation and app-owned absolute executable validation.

Matrix Layer v3 and Group v2 declare opacity, blend and render enablement as real Parameters.
Their immutable kind comes from the persisted node type; `ElementDefinition` and row snapshots
only project these values. `ConfigureItem` uses one `SetParamsAtomic` proposal, sharing validation,
invalidation and exact Undo with direct generic edits. Forest copying preserves the declared
Parameter/control tree with fresh identities. Unsupported row schemas and non-null opaque row
data reject before recoverable decoding. Ordinary blending controls use the generic Inspector;
the old custom blending form and opaque row codec are removed. Layer-only position, integer
canvas size and transition duration are also declared Parameters. `ConfigureLayer` writes them
atomically; Group nodes do not materialize them. Layer v2 and obsolete Matrix-wide transition
Parameter records reject before recoverable decoding; there is no compatibility migration.
The full Matrix model/default/grid conversion remains in M07.

Layer position is the top-left in its parent canvas's pixels. Its own canvas clips children at
their native size; optional Media Resolution resamples the exported texture without changing
the authored logical placement. Render preparation builds the mapping off the engine thread.
Final/Group capture reads the current Media canvas rather than caching a stale Resolution.

Cell v3 and Column v2 declare this subtree directly and own their application identities. The public
`ApplicationId` equals the persistent consumer UUID; the default settings child's internal UUID
may be omitted by sparse persistence and must not escape as a consumer identity. `EnsureCell`
materializes an empty address using backend defaults; `ConfigureCellApplication` configures it
in one authoring/history transaction. Ordinary app-control reads/configuration resolve through
that Column or Cell owner. There is no standalone Application catalog, creation API or
association list. Settings follow owner copy/removal and cannot be detached or separately
disabled. Old standalone records and opaque Column schemas reject before recoverable decoding.
Cell and Column copy operations retain their normal fresh-identity semantics. Column v2 also
declares transition override, whole-millisecond duration and curve Parameters; `ColumnDefinition`
is their projection. The generic Inspector owns ordinary editing of both subtrees.

Cell v3 declares relative position, opacity and enabled Parameters, with no Cell size. Its owned
media retains its own dimensions. `ConfigureCell` atomically edits existing Parameters or inserts
one detached, fully configured sparse Cell subtree; validation precedes any materialization.
Cell v2 and opaque Cell data reject before recoverable decoding. Ordinary controls are available
through the shared Inspector's contextual Inspect Cell settings action.

Source bindings retain each clip's Cell placement/opacity with its source. Live settings edits
update the same authored source without changing its source epoch; replacement/deletion cannot
redirect retained outgoing content to a new source's settings. Cell alpha multiplies transition
weight and post-composite Layer opacity, including the one-input fast path. Disabled destinations
hold previous content for Column launches and skip individual activation; general hierarchy
enablement remains an independent Golden node capability.

Typed pre/post distances stay outside the process runtime key. Matrix projects configured Cell
controls into the same plain demand input used by Column consumers, regardless of media content
or render visibility. The cached logical projection does not build a render index. An app-owned
invalidation callback keeps the reusable settings node independent of Matrix types and wakes
demand when configured settings activate, change or clear; default unconfigured materialization
and source-only Cell edits do not cause application rescans. Live clip retention accumulates
newly observed Cell controls through that clip's lifetime.

`applications/runtime/grouping` assigns one worker to each immutable normalized launch/control
key, combining Column/Cell leases across Matrices. `ApplicationRuntimeId` identifies that
ephemeral physical owner; `ApplicationId` continues to identify the persistent authored consumer.
Effective setting changes split only the affected lease; equal settings join the existing owner.
Outgoing consumers retain their previous owner/configuration until their retained demand ends.
Suspended creation resumes only while at least one matching current consumer is admitted.

The runtime admits 4,096 consumers and at most 64 active/retiring worker keys. Retiring owners
continue to occupy their bounded process slots. Final render plans resolve consumer bindings
to runtime identities when retained; later edits cannot redirect those existing plan leases.
Multiple consumers of one key charge one owner/lifetime entry rather than duplicate holds.

Runtime snapshots flatten the Rust-owned `ApplicationProcessStatus` and add bounded, immutable
current consumer metadata (identity, revision/instance and per-consumer configuration state).
Membership updates once per committed reconciliation; process polling never rebuilds it.
Lifecycle callbacks contain the compact process status once per physical transition, keeping
reservation/event bounds independent of consumer count. Rust generates every TypeScript shape.
The generic Inspector presents the selected consumer's actual configuration state and runtime
consumer count. Runtime identities and native observations are not project data.

`LaunchCell` changes one Layer's ephemeral request without changing the Column anchor. Cell
transition override Parameters take precedence over the Column override and typed Layer
duration. There is no authored Matrix-wide fallback. Disabled Cells skip
individual activation and preserve the previous Layer request during Column launches. Empty
enabled addresses transition toward transparency without creating authored Cell nodes.

Playback retains shared Column mixes plus sparse independent Layer mixes. Source bindings remain
in one 4,096-entry cache; per-Layer/clip app references retire at their own transition deadline.
The original 32-way Column interruption limit remains, and independent Layers use the same
32-contributor limit. Aggregate memory/timing/native scale qualification remains M07/M09; these
functional changes do not waive those budgets. Immutable Layer maps/stamps and captured app arrays
avoid copying whole-Matrix binding state for each Layer. A separate authored-policy cache keeps
logical/headless control independent of render-index construction.

The modal Layer duration supplies the shared mix, with deterministic shortest-duration ties.
Only active source/app Layers and retained consumers need sparse exceptional requests, bounded
by the existing 4,096 binding limit. Admission checks the completed request's actual timed mixes;
a zero-duration cut cannot hide a timed Layer from its unchanged 32-contributor limit.

Logical capture includes every committed Layer request, including a Column-origin request held
by a disabled destination. Such a hold keeps that Cell and its originating Column control;
an individual trigger keeps only its Cell control. Holds deduplicate consumer identities before
admission, so many Layers holding one Column do not consume duplicate demand capacity. The
Column anchor continues to own clamped neighbor preparation independently of these Layer holds.

Rust-generated request identity and per-Layer stamps distinguish requested from submitted mixes.
Double-clicking an actual Layer Cell invokes the backend intent; the grid only presents selection,
requested/submitted markers and typed responses. Playback requests never enter project/history.
M08/2's configuration/demand matrix is accepted: render-disabled/transparent Layers, independent
outgoing deadlines, interruption, reorder/deletion/Undo, project replacement and completion-based
process retention. Complete revised Matrix model/grid/native/scale and aggregate runtime/access/
picker/command qualification remain in M07 and M08/3–4.
