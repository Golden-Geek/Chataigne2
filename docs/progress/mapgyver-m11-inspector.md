# M11/4 typed Inspector and Fit Mode

**State: Active.** Unit-preserving time controls and native-verified media Fit/Fill/Stretch
are delivered. Compact channel/File rows and removal of kind/sampling controls remain required.

## Media Fit Mode

Picture and Shared Texture media v6 and Reference v2 own a real typed Fit Mode Parameter.
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
Old media v5/Reference v1 records are unsupported, without a compatibility migration.

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

Sampling remains authored in this checkpoint and defaults to Linear. Its removal and replacement
command/catalog parity are the next required M11/4 slice, alongside expanded real File/Color
channel rows and reconciliation of the older complete browser flows. This is usable partial
delivery, not acceptance of M11/4 or any retained physical/performance gate.

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

Remove visible Kind/SourceKind and authored generic sampling/filter controls while retaining
typed backend kinds and Linear sampling. Provide expanded compact channel rows with one Color
or actual typed File control, without nested Parameters forms. Preserve channel CRUD, labels,
bounded paging/inspection, independent selection and no per-channel authored size.

Preserve the verified media Fit Mode and clock behavior while completing that Inspector change.
Finish command/catalog and full generic/API/script/headless/history/copy/project/browser parity
for the final revised model. Any further schema/DTO changes must update versions, generation
and consumers together. M11/4 remains
the sole Active gate; this partial delivery closes no gate and verified acceptance stays 50/67.
