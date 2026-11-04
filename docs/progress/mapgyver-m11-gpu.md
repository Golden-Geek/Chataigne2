# M11/3 GPU preference and choice

**State: Active; automatic selection delivered, full acceptance remains open.**

Golden Video owns read-only DXGI enumeration and preference resolution in
`crates/golden_video/src/native/{device,selection}.rs`. MapGyver's presentation worker
uses that public boundary, pins the chosen identity for its task lifetime and reports
the device's observed LUID through the generated runtime-status contract. No GPU
enumeration, selection policy or native work moves into the engine or UI.

## Delivered automatic selection

`preferred_adapters()` uses DXGI's high-performance order independently of raw
enumeration indices. `select_adapter(Automatic)` chooses the first hardware adapter;
`Explicit(id)` requires that exact identity without software/missing-device fallback.
Renderer/output recreation retains the chosen adapter, including after New/Load.
The native test receiver now uses the renderer's exact reported identity instead of
assuming adapter zero. Labels, dedicated-memory size and display ownership are not
adapter identity. LUID tokens can become unavailable after OS/device re-enumeration;
they are not promised as permanent physical-device identifiers.

On 2026-11-04, raw enumeration lists Intel first and NVIDIA second, while Windows'
high-performance enumeration lists NVIDIA first. The actual rebuilt desktop host
selects **NVIDIA GeForce RTX 5080 Laptop GPU**, `dxgi:00000000:00016022`, on both
cold starts even though Intel owns the laptop display. The first start installs
one, two and three authored 4096×4096 previews without unavailable/budget messages.

The second start passes eight concurrent native previews, real transport reconnect,
layout reload, peer close, lock changes, closing every preview and project New/Load.
The independent receiver uses NVIDIA's actual raw index 1 and retains one adapter
identity/shared handle throughout its connection: **770 copies, zero absent frames,
failed receives or descriptor mismatches**. Eight complete 96×64 pixel observations
pass, including editing output while every preview is closed. Reconnect sampling
records 472 observations with zero removed, missing or unavailable views. Both apps
and the independent consumer exit normally. These checks do not close retained
physical, scanout, timing or final qualification gates.

## Validation and limits

- 399 MapGyver library tests pass, three explicitly ignored; 191 Golden Video tests
  pass, 24 explicitly ignored. This includes OS-order selection, exact missing/software
  rejection, reordering and two-word identity-token contracts. Generated preview
  TypeScript matches Rust after regeneration; its initial stale-output failure is fixed.
- Strict all-target Golden Video/MapGyver Clippy passes with native qualification and
  process-fixture features. Four runtime-status UI tests pass; Svelte reports zero
  errors/warnings. Both required Cargo formatters and scoped Prettier pass.
- The first continuity attempt failed in the new test helper when an asynchronous
  WebView status wait returned null. A bounded explicit RPC-read loop fixes the helper;
  the complete corrected run above passes. No product acceptance was awarded to that failure.
- The binary includes concurrent uncommitted user label/theme changes; those changes
  are preserved outside this milestone. This is a development-host check, not a clean
  release artifact. [Receipt](evidence/mapgyver-m11-gpu-selection-2026-11-04.json).

## Exact remaining acceptance

Register a real typed application Enum Parameter in Golden Preferences for Automatic
or an explicit available adapter. Persist it separately from projects, expose the
actual device and restart requirement, and retain a missing explicit choice with
truthful recovery. Verify the same setting through backend/Inspector/history and
cold starts on both GPUs, persisted choice and missing-device/return cases. Shared
textures must never silently move adapters. M11/3 remains the sole Active gate;
M11 stays 2/10 and overall verified acceptance stays 49/67.
