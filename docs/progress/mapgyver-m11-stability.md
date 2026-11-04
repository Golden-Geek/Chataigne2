# M11 interface and native preview stability

Updated: 2026-11-04. **M11/1 remains Active; this milestone fixes reproduced selection teardown.**

Media Preview no longer discards valid same-document content while a new selection resolves.
Its bounded reader keeps the displayed page, accepts only the latest request and immediately
fences project/connection changes. Grid tiles keep their native view registration by viewport
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

Verify the complete reported Inspector/Matrix Cell interaction with native-frame observations,
independent live outputs, rapid selection, tab close/reopen, reconnect and New/Load. In particular,
the current document fence still hides stale content immediately on reconnect/replacement;
its visible transition must satisfy M11 rather than inheriting the older brief-unavailable waiver.
Continue the same gate with these current fixtures; do not repeat the fixed selection probe
without a new change or required regression purpose. No gate closes in this milestone.
Counts remain **47/67 gates (70%), 6/12 phases (50%), M11 0/10**.
