# M14/6 Screen creation defaults

MapGyver owns fresh Screen allocation; Golden supplies physical connection metadata and the
read-only detached creation context. See [ownership and lifecycle](../architecture/mapgyver-screen-defaults.md).
M14/6 remains open for physical output and native keyboard checks. The common software,
headless and no-external desktop scopes passed.

## Numbered display feedback

- A fresh Windows app selected **RTK FHD HDR** (GDI display 2) with **Optoma UHD** also active.
  Debug automation opened zero physical output windows while the app published Spout video.
  [App result](evidence/mapgyver-m18-embedded-glsl-2026-11-19.json)
- Eight Screen creation tests and four output-binding tests passed. They cover startup ordering,
  authored display identity, multiple assignments, and isolated non-window output admission.
- Physical pixels and output recovery on both GPUs, occupied assignments and the native New
  shortcut still need their qualification run.

## Development build refresh

- The previous `target/debug/MapGyver.exe` and the installed `1.0.0b7` beta predated
  `88be30d71`. The old selection sorted device paths, so Windows display numbers did not
  determine the initial target. Fresh assignments in the fix prefer the lowest available external number.
- Rebuilt the ordinary development executable on November 20. The numbered-display regression
  passed with an RTK display numbered 2 and a projector numbered 3 in either catalog order.
- The rebuilt Windows app passed all 16 startup, New, Inspector, reload and independent Spout
  checks. Startup and New picked RTK FHD HDR (2), and the fixture opened zero physical output
  windows. The live catalog contained the laptop panel and RTK only. Physical qualification
  remains open. Receipt: `target/mapgyver-startup-display-140610/result.json`.

- Rechecked on November 20 with the laptop, RTK, and Optoma all connected. The Windows
  catalog returned Optoma before RTK, with GDI indices 5 and 2 respectively. The development
  build selected RTK on startup and on each of three New operations. All 16 checks passed,
  including two exact Spout images and zero physical output windows. The installed executable
  still reports `1.0.0b7`, which predates the fix. Receipt:
  `target/mapgyver-startup-displays-165302/result.json`.
- Several fixtures explicitly open physical outputs and choose targets from Windows catalog
  order, independently of the application's startup ranking. Keep those cases in the physical
  qualification queue on this topology. Saved projects retain the display authored on each Screen.
- Rechecked the November 20 18:06 development build with both RTK and Optoma active. All 16
  checks passed. Startup and three New operations targeted RTK and opened zero physical
  windows. The Windows Start menu shortcut still points to the October 19 installed beta
  `1.0.0b7`, which predates the numbered-display fix. Receipt:
  `target/mapgyver-startup-displays-180922/result.json`.

## Automated test isolation

- The shared Windows browser launcher now suppresses fullscreen and rectangle outputs by
  default. Fixtures that measure physical output explicitly opt in. Screen previews and Spout
  delivery use the same test host.
- The owned NVIDIA WebView2 run passed 16 checks with RTK FHD HDR (display 2) selected on
  startup and after New, zero physical output windows, and two complete red Spout images
  before and after reopening the saved project. Windows listed the laptop and RTK for this
  run. The projector was absent from that catalog.
- Receipt: `target/mapgyver-test-display-isolation-125611/result.json`.

## Clearing a Fullscreen target

- In the November 20 Intel editor fixture, `ConfigureScreen` with `target: null` rejected
  a newly assigned Fullscreen Screen while the display enum offered three connected displays.
  The runtime reported `value is not in enum options`.
- Receipt: `target/mapgyver-m141-corner-screen-0-191440/failure.json`.
- Fixed the shared display options so **No display selected** stays available. Clearing a
  display removes its recovery hints in the same undoable edit. Reopening preserves the empty
  Fullscreen target, and a new Screen can claim the released display.
- All 120 Screen tests, strict app-library lint and the development build passed. With the
  laptop, RTK and Optoma connected, the Intel WebView2 run passed 20 checks. It cleared the
  display through public configuration and the Inspector, verified Undo/Redo and reopening,
  picked RTK on New, rendered two exact Spout frames, and opened zero physical output windows.
  Physical routing and the native shortcut remain pending. Receipt:
  `target/mapgyver-clear-fullscreen-192927/result.json`.

## Physical RTK startup and New

- The November 20 fixture used `--physical-fullscreen=RTK FHD HDR` with the laptop, RTK and
  Optoma connected. The debug host admitted only the resolved RTK fullscreen destination.
- Intel and NVIDIA each showed exact red desktop samples at startup and green samples after
  File → New. Both runs measured fullscreen RTK bounds `[2560, 0, 4480, 1080]`. Each opened
  two RTK output windows and zero Optoma windows, and exited normally.
- Creating two more Screens authored the Optoma target on the second Screen and left the
  third unconfigured. The RTK window kept displaying the expected red image.
- Four target tests, strict app-library lint, the development build and the ordinary isolated
  20-check browser run passed. The isolated run opened zero physical output windows.
- Receipts: `target/mapgyver-rtk-routing-194133-0/result.json`,
  `target/mapgyver-rtk-routing-194133-1/result.json` and
  `target/mapgyver-rtk-routing-194133-isolated/result.json`.
- Remaining: one-external physical defaults, simultaneous physical output on both externals,
  hot-plug and native Ctrl+N on both GPUs. The restricted run verified sequential allocation
  with both externals occupied. The detached atomic case still uses software evidence.

Implemented behavior:

- Fresh startup/New candidates seed their initial Screen and ephemeral display catalog before
  publication, using one metadata worker. Live creation performs no OS IO.
- Known external connections are eligible even when primary. Internal and unknown/virtual
  connections are excluded. Fresh Screens prefer the lowest available Windows display number.
  Stable device paths order ties and displays with unavailable numbers.
- Disabled fullscreen assignments and earlier detached roots reserve their matched displays.
  Ordinary and dependent items in one atomic batch cannot claim the same free display.
- No free external means typed Fullscreen/empty Display, an unconfigured render projection
  and no implicit Shared Texture sender. Intrinsic decoding defaults remain unchanged.
- Existing authored targets, sparse load, history and runtime recovery retain their semantics.

The complete app library suite passes715 tests, with eight existing hardware tests ignored,
sequentially in108.33s. This includes all115 Screen tests, fresh-candidate seeding, generic/
domain/script creation, atomic ordinary/dependent allocation, exact history/reload, rendering,
display matching and generated-contract comparison. Screen code generation, the MapGyver UI
typecheck and scoped browser-fixture Prettier pass. Three shared creation-context tests and
five desktop display tests pass. Strict app/engine/desktop library/test Clippy passes with
warnings denied, and the ordinary default-feature app build exits0. The existing headless host
contract passes six groups: startup, repeated empty reload, exact three-Screen load and three
New operations with fresh identities and zero bootstrap history. HostPID50404 exits0 and
errors are empty. Each fresh target is compared with the current independent host catalog;
this does not test later creation after load or physical output pixels.

The current independent Windows catalog contains one connected internal panel and no known
external display. This host cohort exercises the no-external fallback; one/multiple external
physical cases are not claimed from these observations.

Receipts: `target/m14-display-app-tests-accepted-2026-11-16.log`,
`target/m14-display-codegen-2026-11-16.log` and `target/m14-display-ui-check-2026-11-16.log`.

The first full run passed713 tests and found two
no-native-host fixtures inheriting the new startup inventory while supplying independent
mock catalogs. Their setup now clears inherited metadata; identity matching and pixel/history
assertions remain intact. Earlier Screen fixture assumptions about implicit Spout targets and
primary/secondary classification were made explicit. Failed receipts remain in `target/`.

**Loaded/headless correctness:** opened candidates now prepare missing ephemeral inventory once,
distinguishing unprepared from prepared-empty state. The existing headless worker refreshes
once per second through the current catalog inbox; unchanged catalogs avoid actor captures.
Loss/recovery and old-publisher replacement preserve authored targets and history. Configuration
of an unselected Fullscreen Screen now preserves its mode when changing frame rate or AA.

The updated complete library run passes718 tests (eight existing hardware tests ignored) in
90.20s, including both real-actor inventory tests and exact Fullscreen configuration history.
Strict app/library/test lint passes. The tests drive normal runtime ticks for catalog wakes;
load compares authored snapshots across decoding and the exact loaded document/history before
and after metadata preparation, allowing intrinsic declaration-order canonicalization by the
existing codec. Prior failed receipts remain preserved. The default-feature app build and
strict lint exit0. The extended actual headless contract passes seven groups, including
immediate creation after empty load, one current Display enum option, a null destination on
the current no-external topology and exactly one creation history item. HostPID38648 exits0;
errors are empty. See the [inventory receipt](evidence/mapgyver-m14-screen-inventory-2026-11-16.json).
Loaded/headless correctness is functionally verified for these scopes; desktop and original
physical topology/output/recovery requirements remain open under M14/6.

Shared/default receipts: `target/m14-display-core-creation-tests-2026-11-16.log`,
`target/m14-display-host-tests-2026-11-16.log`, `target/m14-display-clippy-2026-11-16.log`
and `target/m14-display-default-build-2026-11-16.log`, all exit0. The original long-running
Core test handle finished normally and was not restarted. Observer build and existing headless
host checks also exit0; see `target/m14-display-headless-host-2026-11-16.log` and the
[checkpoint receipt](evidence/mapgyver-m14-screen-defaults-2026-11-16.json). This checkpoint
delivered common allocation; the later inventory receipt verifies loaded/headless correctness.

**No-external desktop/native qualification:** Intel and NVIDIA each pass16 recorded checks in
the actual WebView2 application: typed Target=Fullscreen/empty Display, read-only selection,
startup/New/File-menu and exact reload flows, post-load creation, actual requested GPU identity,
two exact red96×64 SDK images before/after reload and old-output withdrawal on New. Four images
cover24,576 texels. Both owned hosts exit0 with empty page errors; the original deadlines,
watchdogs and pixel expectations are unchanged. See the [desktop receipt](evidence/mapgyver-m14-screen-desktop-2026-11-16.json).

The fixture now observes an active GPU only after configuring an output. For an unselected
replacement it requires Idle, no GPU adapter/resources/screens and a new existing production
`native session retired` receipt. `preview/host/worker.rs::Owner::drop` waits renderer, Picture
and Video retirement before that receipt; Idle sampling follows complete owner destruction.
Configured replacements retain Running/retiring=0. Null Idle counters are not measured zeros.
The first two failed Intel receipts preserve both obsolete Running assumptions. Source inspection
resolved the final correction; no profiler, observer framework or budget change was added.

`--defer-native-shortcut` is explicit partial coverage: API New and the File menu are tested;
physical Ctrl+N is not. The default invocation still requires that native input. Both supported
computer-use connections report the native pipe missing; no helper repair chain was attempted.

**Return triggers** — use a controlled physical session for the remaining topologies, hot-plug
and simultaneous output on RTK and Optoma. Verify the actual Ctrl+N action through native desktop
input. The November 16 no-external run had only the internal panel; the November 20 RTK run had
both externals. M05 recovery evidence applies to its recorded configurations. M14/6 remains open
before M09. Standing Overlay Off applies.
