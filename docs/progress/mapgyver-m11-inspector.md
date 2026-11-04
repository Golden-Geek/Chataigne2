# M11/4 typed Inspector and Fit Mode

**State: Pending urgent preview-only cadence repair.** Unit-preserving time controls and native-verified media Fit/Fill/Stretch
are delivered. Kind/sampling controls are removed and expanded rows expose Color directly.
Catalog assets now have one authoritative typed File and Picture channels directly expose it.
Final complete Inspector/browser parity, compact layout and retained Undo stability remain required.

## Direct channel File controls

Configured Picture channels render the catalog's actual typed File in the generic Inspector.
Unconfigured channels reuse Golden's public FileInput for a contextual backend import. The new
ConfigurePictureFile intent creates/binds an asset atomically, edits existing shared authority,
retains bindings on clear and recovers a missing asset through an explicit replacement. Empty
unbound input allocates nothing. InspectPictureFile is a bounded immutable query returning the
real File Parameter's id/UUID; UI ownership/generation checks prevent stale delegation. The
existing asset picker remains a secondary contextual operation. Permanent channel import forms,
location choices and asset-reference fields are removed; no channel path is persisted.

[Receipt](evidence/mapgyver-m11-channel-file-2026-11-04.json): 413 app tests include materialization,
shared authority, rejection, missing-asset recovery, history/project and four-caller parity. Rust
generates the intents, binding DTOs and TypeScript together; named script methods and templates
are wired. Strict all-target native/process lint, 236 Golden UI tests and Svelte 0/0 pass.
Actual native controls pass initial assignment/Undo/Redo, independent channels, the existing
asset picker, shared real Parameter editing, clear and Save/New/Load: 145 requests/145 applied
receipts, no backend rejections/page errors/final client errors, idle activity and normal exit.
The screenshot is inspected. Decoded pixels, native dialogs and cadence are outside this fixture.

Final compact row layout, the complete legacy generic-Parameters/sources browser entries and
the earlier intermediate Undo timeout remain in M11/4 acceptance. The user's confirmation that
only preview/warping is slow immediately returns priority to M03/9; this slice accepts no gate.

## Single catalog File authority

`mapgyver.media.asset.v2` replaces the location enum and two path fields with one real File
Parameter. Asset authoring classifies native absolute versus validated project-relative paths;
the generic Inspector supplies its ordinary File widget. Import and Relink write that same
Parameter. Clearing retains the asset and every channel binding while making the source
unconfigured. Asset/Parameter identities, source invalidation, sharing and history remain intact.
The existing AssetLocator DTO is unchanged. Rust generates the new asset-type constant for UI
registration, filtering and reference constraints. Old asset records and split path fields reject
before recoverable project decoding; no persisted-data migration is introduced.

[Receipt](evidence/mapgyver-m11-asset-file-2026-11-04.json): 409 app tests, generated contracts,
strict all-target native/process-feature lint and Svelte 0/0 pass. The actual native Inspector
passes relative/absolute File edits, two shared channels, clear/history, Relink/Reload and
Save/New/Load. All 74 observed requests have applied receipts, with no backend rejections,
page errors or final client error toasts. Final activity is idle, the screenshot was inspected,
and the owned host exits normally. Browse is available; this fixture does not operate a native
file dialog or qualify decoded/GPU pixels. Its binary includes concurrent user UI changes.

An intermediate native run timed out observing Undo once. The cause is unproven and remains
in final Inspector stability acceptance despite the final passing run. The fixture now observes
already-connected native WebSocket receipts and waits for UI activity to settle before external
project replacement; it does not qualify a transient Failed indicator as a clean final state.
The first reload-based observation attempt aborted before its flow; no native outcome is inferred.

The subsequent direct channel File slice above replaces its permanent reference/import controls.
No mirrored persistent path, channel-size setting or acceptance-gate credit is added.

## Expanded typed channel controls

Media v7, Reference v3 and command v5 remove authored Sampling, its operation/script method
and its DTO projection. MapGyver fixes sampling to Linear in source, Reference, Composition
and Matrix preparation; Golden keeps its app-neutral sampler support. The typed Fit Mode
command replaces Sampling. Old schema/operation/command records reject rather than silently
changing authored behavior. Project admission also rejects retired sampling Parameters.

Channel rows start expanded, hide immutable kind controls, and render their real Parameter
children through Golden's public `inlineChildren`/`childDensity` Inspector hooks. Solid Color
has one directly editable Color, with no nested Parameters header. Existing labels, context
menus, shared selection and 32-row paging remain in the generic workbench. Golden also consumes
rename input key events: Enter previously finished the rename and then collapsed its row.

[Current receipt](evidence/mapgyver-m11-compact-channels-2026-11-04.json): 407 app tests and six
Composition integration tests pass, along with generated contracts, strict all-target native
lint, 236 Golden UI tests, 17 affected controller tests and Svelte 0/0. Native verification:

- Actual Color edit/Undo/Redo, peer isolation, rename/duplicate/delete/history and 37-channel
  paging pass, with mounted control counts 32/5/32. No kind/sampling controls or nested channel
  Parameter headers appear. Rename retains expansion; normal host exit and screenshot inspection pass.
- Sibling stability passes 12 real row/Resolution actions over 1,020 animation frames: no
  removed/disconnected/faded witness and minimum opacity one, including the final density build.
- Both GPUs retain all 24 complete Fit/Fill/Stretch/Auto images. Requalified Composition uses
  an independent bilinear CPU oracle for the required Linear contract: 54 complete images cover
  nesting, transforms, four blends, resize, missing sources, Undo and project reload/recreation.
  Existing quantization bounds remain; observed errors are at most 1/255. All owners retire normally.
- Two complete ordinary headless-host runs pass source intents/rejection/assets/history/Save/
  New/Load, generic shared/procedural Resolution and Layer clock editing, channel CRUD/paging and
  independent Shared Texture sender edits/history. The old procedural browser fixture was corrected
  from media-wide numeric transitions to the actual Layer-owned clock. The sender fixture now
  preserves already-expanded rows instead of collapsing them before editing.

Picture retains an internal typed asset reference; direct File controls now bind to the catalog
authority as described above, preserving sharing/import/relink/reload. The full
legacy generic-Parameters browser entry also still starts with an obsolete reference-only
Composition form. Its media leaf is reconciled, but the complete entry is not claimed passing.
These remain in M11/4's final parity/acceptance queue. This slice closes no gate.

## Media Fit Mode

Picture and Shared Texture media v7 and Reference v3 own a real typed Fit Mode Parameter.
It applies to every channel at the owning media's forced Resolution. Fit centers an intact
image with transparent bars; Fill centers and crops it; Stretch scales both axes independently.
Auto preserves source-native dimensions and the Inspector hides Fit Mode while retaining its
authored choice. Solid Color and generated Composition/Matrix canvases omit the Parameter and
reject the Fit Mode intent. Those canvases author their own output geometry through Resolution.

MapGyver's preparation layer projects this policy into Golden's existing checked projective
compositor. Picture preparation stays shared per asset; mode changes add no image uploads.
Reference Auto retains its shared-target alias. The typed `ConfigureMediaFitMode` operation,
`configureMediaFitMode` script method and generic Inspector edit the same Parameter.
Schema admission, all current version consumers and Rust-generated TypeScript change together.
Older media/Reference records are unsupported, without a compatibility migration. The original
Fit receipt below used v6/v2; the current Linear/Inspector receipt rechecks the same native pixels.

Verification on 2026-11-04 ([receipt](evidence/mapgyver-m11-media-fit-2026-11-04.json)):

- 406 app tests pass, with three unrelated native/process tests explicitly ignored. Coverage
  includes typed generic edits, shared channels, invalid modes/unsupported owners, Undo/Redo,
  duplication, project loading and four source-operation callers (UI, direct/hosted/named
  QuickJS). Node-count bounds include the additional real Reference Parameter; no bounds relax.
- Generated-contract tests and strict native-feature all-target lint pass. Svelte has zero
  errors/warnings; all 17 affected Composition/Matrix UI controller tests pass.
- Intel Graphics and NVIDIA RTX 5080 Laptop GPU each pass three independent Spout outputs
  for wide/tall channels in one Picture media. Forced Resolution is 24×24 for Fit/Fill/Stretch;
  Auto returns 24×12 and 12×24. All 24 complete images (12,096 pixels) match an independent
  scaling/cropping/linear-sampling CPU oracle within 2/255. Both source images stay uploaded
  exactly once across all forced mode changes. Both renderer owners retire normally.
- The actual native Inspector passes all three modes on two channels, history, hiding in Auto,
  retaining Fill through Auto/re-enable, omission on Solid Color and live Shared Texture and
  Reference controls. It reports no page errors and exits normally. The screenshot was inspected:
  Fit Mode fits the existing generic Inspector. It also visibly confirms the remaining nested
  channel/kind/sampling cleanup. Native binaries include concurrent user label/theme changes,
  which remain outside this milestone.

Sampling removal, replacement command, expanded Color and direct File delivery are verified above.
Final complete browser parity and compact File layout remain required. This is usable
partial delivery, not acceptance of M11/4 or any retained physical/performance gate.

## Time controls

Golden's existing numeric clock editor now honors the Parameter's declared `unit` hint.
`widget = "time", unit = "ms"` displays a clock while keeping the authored/backend number
in milliseconds. Seconds use `unit = "s"` or the existing unspecified-seconds convention.
The Params DSL and `#[param]` forms share hint generation and binding/replacement logic.
No DTO type or authored numeric value changes; the existing Rust-generated unit field is reused.

MapGyver applies this metadata to Layer, Column and Cell transition durations and the five
app-control timeout/stop/restart durations. Labels no longer suggest entering raw milliseconds.
M10 must use the same Golden style for future Video time Parameters. Source File/asset ownership
and channel sizing policies are unchanged by this slice.

Verification on 2026-11-04:

- 403 app tests pass (three explicitly ignored), including all eight declaration types,
  raw millisecond values, history and project roundtrip. The crate-external Golden fixture
  tests both declaration forms, seconds/milliseconds and binding/project loading.
- All 236 Golden UI tests pass, including exact millisecond parsing for 0–999, negative times,
  long durations and invalid input. Strict all-target macro/Core/MapGyver lint passes; Svelte
  reports zero errors/warnings. Existing generated-contract app tests pass without a type change.
- The actual native WebView edits all eight controls and observes exact backend millisecond
  values: Layer 1234, Cell 2345, Column 3456, ready timeout 4321, graceful stop 2100, retirement
  timeout 8000, initial restart 1234 and maximum restart 9012. Layer Undo/Redo passes. No
  application process starts, playback stays unlaunched, and the host exits normally.
- The actual screenshot was inspected: clock values fit the existing workbench Inspector.
  The development binary includes concurrent user label/theme changes, preserved outside this
  milestone. [Receipt](evidence/mapgyver-m11-time-controls-2026-11-04.json).

The initial external fixture needed its normal generated-code `serde_json` test dependency;
the lockfile adds only that already-pinned dependency edge. No product dependency or stored
time-unit migration is introduced. A title-independent navigation check initially omitted
activating the media-manager tab. Explicit default-tab activation corrects the fixture; its
complete rerun again passes all eight edits and normal shutdown without relying on the user's
uncommitted panel-title rename.

## Exact remaining acceptance

Complete final compact File-row layout and the full generic Inspector/browser/caller acceptance,
including the retained intermediate Undo timeout. Preserve the now-delivered real File authority,
asset sharing/import/relink/reload, channel CRUD/labels/paging, independent selection and no
per-channel authored size. The updated legacy generic-Parameters and source browser entries
still require a complete run.

Preserve the verified media Fit Mode and clock behavior while completing that Inspector change.
Finish command/catalog and full generic/API/script/headless/history/copy/project/browser parity
for the final revised model. Any further schema/DTO changes must update versions, generation
and consumers together. M11/4 is Pending while the urgent preview-only cadence repair is Active;
this partial delivery closes no gate and verified acceptance stays 50/67.
