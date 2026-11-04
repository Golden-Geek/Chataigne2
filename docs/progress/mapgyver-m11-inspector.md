# M11/4 typed Inspector and Fit Mode

**State: Active.** Unit-preserving time controls are delivered; compact channel/File rows
and media Fit/Fill/Stretch remain required before this gate can be accepted.

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

Add typed media-level Fit/Fill/Stretch only for variable-size sources, hide it in Auto, and omit
it for Solid Color. Verify different-aspect Picture channels in every mode with native pixels
and one forced Resolution; Auto must preserve intrinsic source extents. Reconcile affected
generic/API/script/headless/history/copy/project and browser fixtures with the revised model.
Any schema/DTO changes must update versions, generation and consumers together. M11/4 remains
the sole Active gate; this partial delivery closes no gate and verified acceptance stays 50/67.
