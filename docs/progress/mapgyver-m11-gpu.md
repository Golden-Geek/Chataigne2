# M11/3 GPU preference and choice

**State: Accepted.** Automatic preference, persisted typed explicit choice and native recovery checks pass.

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

## Typed Preferences ownership

MapGyver registers `VideoPreferencesNode.adapter`, a real Params DSL Enum under
Preferences → Video → GPU. Golden's public `ProjectLifecycle::preferences_tree` hook
and shared host merge preserve existing live nodes/values while inserting missing
detached subtrees. The existing codec saves Preferences separately from projects;
subtree insertion gives loaded nodes fresh engine-scoped UUIDs, while merging keeps
those live identities intact. The generic Inspector edits that same Parameter.

Desktop/headless host workers enumerate adapters once per second and publish only
changed choices through a latest-only, lifetime-fenced node wake. Missing selected
and historical tokens remain marked unavailable for exact Undo/Redo. The catalog
admits at most 64 retained choices and reports overflow without dropping identities.
No enumeration or polling runs inside an engine node. Non-native builds exclude the
producer. A presentation task freezes its startup choice; a later edit reports
`gpu_restart_required` in the generated status contract and footer, while native
renderer recreation retains the actual adapter. An unavailable startup choice
fails truthfully and retries that identity, without an integrated/default fallback.

402 app, 673 Golden Engine and 96 transport tests pass, including separate persistence,
merge idempotence, history, missing/returned choices and stale/lifetime fences. The
three Preferences tests pass again after feature isolation; strict native all-target
lint, generated contracts, four status UI tests and Svelte zero errors/warnings pass.
An additional no-default-features strict lint exposed existing preview/Screen dead-code
warnings; new Preferences producer warnings have been corrected with feature isolation.

The non-native library also compiles. Its 14 pre-existing preview/Screen dead-code
warnings still fail an additional strict no-native lint; Preferences adds none.

## Native Preferences acceptance

The corrected fixture passes five fresh native processes with one private app-data
directory and the real File → Preferences Inspector:

| Cold start | Saved choice | Observed behavior |
| --- | --- | --- |
| Automatic | Automatic | NVIDIA high-performance default; changing to Intel reports restart required and keeps NVIDIA through renderer recreation. |
| Explicit Intel | Intel LUID | Intel is actually active; changing to NVIDIA reports restart required and keeps Intel through recreation. |
| Explicit NVIDIA | NVIDIA LUID | NVIDIA is actually active; persisted choice and control agree, with no restart pending. |
| Missing choice | `dxgi:ffffffff:ffffffff` | Faulted, no active device/resources, selected choice marked unavailable, explicit no-fallback error. Changing to Automatic requests restart without silently allocating another GPU. |
| Recovery | Automatic | Restart restores NVIDIA rendering; error and restart-required state clear. |

Eight full 96×64 output images pass. Six exact-adapter independent SDK consumers
receive **447 copies** with zero absent frames, failed receives or descriptor mismatches.
All five hosts and six consumers exit normally. A fixture writer creates the missing
selection through actual typed Parameters and Golden's codec, in private test app data;
it does not simulate physical device removal. Genuine removal/physical/timing gates
remain required in their owning phases.

Initial fixture runs failed to reveal Preferences via an early shortcut, then lost
the GPU widget when creation changed shared selection. Explicit File-menu navigation
and revealing Preferences before edits correct the harness; partial attempts accepted
no gate. The earlier UUID test incorrectly expected subtree import to retain UUIDs;
it now verifies the existing remap contract and live merge identity preservation.
[Acceptance receipt](evidence/mapgyver-m11-gpu-preferences-acceptance-2026-11-04.json).
M11/3 is Accepted; M11 is 3/10 and verified acceptance is 50/67. M11/4 is next.
