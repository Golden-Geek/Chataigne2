# M11 interface and native preview stability

Updated: 2026-11-04. **M11/1 remains Active; selection and graph-reconnect teardown are fixed.**

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

## Exact remaining M11/1 acceptance

Verify the complete reported Inspector/Matrix Cell interaction with native-frame observations
and independent live outputs. Selection and reconnect now have actual native pixel evidence;
tab close/reopen and New/Load run in the native continuity flow, with intentional retirement and
eventual recovery verified. Full transition-frame coverage for those latter cases remains open.
Continue the same gate with these current fixtures; do not repeat the fixed selection probe
without a new change or required regression purpose. No gate closes in this milestone.
Counts remain **47/67 gates (70%), 6/12 phases (50%), M11 0/10**.

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
