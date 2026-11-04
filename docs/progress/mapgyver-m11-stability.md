# M11 interface and native preview stability

Updated: 2026-11-04. **M11/1 Accepted.** Selection and graph-reconnect teardown are fixed;
recorded native input, lifecycle, desktop pixels and independent live output verification pass.

Media Preview no longer discards valid same-document content while a new selection resolves.
Its bounded reader keeps the displayed page, accepts only the latest request and immediately
fences project/session changes. Grid tiles keep their native view registration by viewport
slot, letting Golden's existing native last-frame retention replace the source without an
unregister/re-register cycle. MapGyver owns this selection/presentation fix; Golden's runtime,
protocol, resource budgets and completion barriers are unchanged.

## Reproduction and verification

- Recorded native baseline `MapGyver-codex-m08-retirement.exe`: eight follow-selection switches
  inserted **eight loading states** and disconnected the original native view. The identity
  witness was absent/replaced in 442 of 450 animation-frame samples. This is a DOM/lifetime
  reproduction in the actual WebView; it is not 442 independently captured blank GPU frames.
- Corrected native build: **16 grid-selection switches**, both registrations preserved,
  **zero loading insertions or disconnected-view samples** across 1289 animation-frame samples.
- After Computer Use selected/activated the exact owned window, the existing read-only
  foreground/PID-guarded desktop capture sampled **94 actual composited frames**, checking
  7×7 center regions in **both native previews**. Red appeared in 49, blue in 45, other/blank in
  **zero**. Both initial and final native captures show the selected red pair. This verifies
  those two regions across the recorded sequence, not full-image raster accuracy or latency.
- The tightened Inspector fixture verifies **12 actual arrow-collapse/expand and Sampling
  actions** with **two mounted sibling Parameter witnesses**: 1020 animation-frame samples,
  no removals, disconnections or fades; minimum sampled opacity 1. The earlier header-only
  pass does not stand in for this complete Parameter witness. Setup errors involving an obsolete
  checkbox label and title text (which starts rename rather than toggling) were corrected.
- Four reader tests pass, including retained content/latest reply and immediate generation/
  connection fencing. Golden Video UI's declared Vitest suite passes **30 tests**. Svelte has
  **0 errors/warnings**; strict all-target native/process-feature Clippy passes. The initial
  generic Node invocation could not load the Vitest suites; the owning package command resolves it.
- The bundled browser continuity regression passes eight concurrent views, independent channel
  choice, real transport reconnect, layout reload, all-preview closure and New/Load lock recovery,
  with no page errors. Those lifecycle cases are browser evidence; native-frame qualification
  for their full M11 scope is still required. Both Cargo formatters and scoped JS/TS formatting pass.

[The receipt](evidence/mapgyver-m11-preview-selection-2026-11-04.json) pins binaries, artifacts and
local logs. The native pixel observer performs no input; Computer Use owns foreground activation
and the fixture uses documented CDP input in its isolated native app. Capture directories must
be fresh. No capture or native check is inferred from DOM data alone.

## Retained limits of the selection/reconnect checkpoints

The first two checkpoints did not independently establish complete Inspector/Matrix or native
project-transition acceptance. Their receipts preserve that limit. The additional input and
project-transition verification below completes those requirements; no earlier DOM-only result
is retroactively described as native pixels. Native sampling is bounded regional evidence, not
a claim of exhaustive raster coverage or acceptance of unrelated retained timing budgets.

## Native graph reconnect correction

The graph WebSocket and the native preview session have different lifetimes. The Media Preview
reader now uses Golden's existing public backend `projectGeneration` for displayed content;
read replies still require the exact snapshot, history, current selection request and connected
session. Disconnecting the graph socket suspends reads without unregistering native views.
Explicit loading, project-generation replacement, unknown project identity, session replacement
and disposal still fence old presentation. No protocol, renderer or resource budget changed.

The recorded native baseline reproduced two removed preview subtrees (five native views) and
201 missing-original animation samples. Its independent SDK consumer still received 271 frames
without absent/failed/mismatched receives. The repaired native flow preserves all five original
views: zero removals, missing samples or unavailable overlays across 147 animation samples.
Computer Use activated the exact owned window; a foreground/PID-guarded read-only capture then
checked five 7×7 regions across **13 actual composited frames**, with **zero changed/blank regions**.
Initial white/blue/white/white/white content remained identical in those regions. This is sampled
native visual evidence, not full-raster or latency qualification.

One independent SDK consumer stayed live through eight concurrent previews, real reconnect,
layout reload, peer/all-preview closure and lock changes: **1551 copies**, zero absent/failed/
descriptor mismatches, one shared handle and advancing frame counters. Complete 96×64 output
pixel checks pass at eight checkpoints, including green after editing with every preview closed,
and restored red after Save/New/Load. New intentionally destroys old outputs, so the continuous
receiver stops before New; a new pixel receiver verifies restored output. Native host exits 0.

Five reader tests and Golden Video UI's 30 tests pass; Svelte reports 0 errors/warnings. Strict
all-target native/process-feature Clippy and the fresh native app build pass. Rust behavior is
unchanged; the previous 398-test result remains applicable. Both Cargo formatters and scoped
JS/TS formatting pass. [Reconnect receipt](evidence/mapgyver-m11-preview-reconnect-2026-11-04.json)
pins binaries, artifacts, checks and limits. This capability checkpoint accepts no additional gate.

## M11/1 acceptance — native input and project transitions

The unchanged corrected app binary was exercised through documented WebView input. Computer Use
selected and activated each exact returned owned window; the existing foreground/PID-guarded
read-only capture verified actual composited desktop pixels. Tests use isolated app data and
projects, normal host shutdown and real backend operations. Product files were unchanged in
this qualification slice; concurrent unstaged label/theme work remains outside these commits.

| Requirement | Verified acceptance |
| --- | --- |
| Ordinary controls and Inspector expansion | Twelve real collapse/expand and Sampling actions. Two mounted unchanged sibling Parameters stay connected, with no fade/removal across 1022 animation samples. **72 desktop frames** preserve both complete Parameter regions (250×59 and 250×56 physical pixels) and a locked red native preview region. No changed UI frames or blank/changed native regions. |
| Matrix Cell clicks | Sixteen actual alternating Cell clicks, each confirming selected state. **35 desktop frames** preserve the unrelated locked red preview and its complete 282×47 toolbar region. No changed UI frames or blank/changed native regions. The native Inspector/Matrix sequence's independent SDK receiver gets **2708 copies**, zero absent/failed/mismatched receives, one shared handle and advancing frame counters; complete 96×64 red output pixels pass before/after both sequences. |
| Rapid media selection | Recorded baseline reproduced teardown; corrected **16-switch native grid** preserves both registrations, with 94 actual desktop frames showing red/blue and zero blank/other sampled regions. Latest-selection/history/project fencing is separately covered by reader tests. |
| Reconnect | Real graph socket loss/recovery preserves five original native views and their five sampled regions across 13 desktop frames. Independent output remains live across the complete native continuity flow. |
| Tab close/reopen | Three actual close/reopen cycles recover the reopened preview while preserving the peer's original registration and red content. Independent SDK output gets **1430 copies**, zero absent/failed/mismatched receives and a stable handle. Complete output-pixel checks pass after every cycle. |
| New/Load and stale-project fencing | **87 actual desktop frames**: 56 tab-cycle frames preserve red peer content; 8 settled-New frames contain no old red content; 14 settled-Load frames show restored red; 9 frames straddling intentional replacement are recorded separately. The unrelated File menu region is unchanged throughout. New retires all preview views; locked panels report foreign-document state, then recover the authored media after Load. The old receiver stops before intentional output destruction; restored output is verified by a new receiver and full 96×64 pixels. Both actual New and loaded screenshots were visually inspected. |

All relevant software prerequisites were terminal before native checks: five reader tests,
30 Golden Video UI tests, Svelte 0 errors/warnings, strict all-target native/process Clippy and
fresh app build. Rust is unchanged; the existing 398-test result remains applicable. Scoped
fixture formatting/syntax checks and both Cargo formatters pass. Native hosts exit normally.

Fixture setup corrections are explicit: the File menu's accessible name is `Open File menu`;
project Load returns a typed path DTO rather than `{ok:true}`. An initial run aborted on the
incorrect Load assertion and closed its owned window, invalidating that capture; it is not
product failure or passing evidence. The complete corrected sequence passes unchanged product
code. Pillow's deprecated pixel iterator was replaced with its supported equivalent after the
passing Inspector capture emitted a warning; pixel comparison criteria were unchanged.

[Acceptance receipt](evidence/mapgyver-m11-stability-acceptance-2026-11-04.json) pins the two native
sequences, artifact hashes and scope. **M11/1 is Accepted; M11 is 1/10 (10%), accepted gates are
48/67 (72%), fully accepted phases remain 6/12 (50%).** No denominator or requirement changed.
Next dependency-ready gate: M11/2 concurrent 4K capacity and bounded adapter-aware admission.
