# Fading Layer Stop (M14/10)

MapGyver's existing Layer Stop trigger now fades its current mix instead of cutting.
The backend uses the latest requested destination on that Layer, including independent
Cell launches, and resolves current optional Cell → Column → Layer duration/curve.
When that destination was cleared, the latest retained outgoing launch supplies the
Column. All contributions fade together from their sampled weights. Zero duration
cuts immediately; repeated Stop preserves the committed deadline; a new Play revokes
the old completion. Column, Cell, group and Stop All cuts retain their contracts.

[The architecture](../architecture/mapgyver-matrix-controls.md) owns the boundaries:
receipt-gated requests and logical outgoing holds in Matrix playback, the existing IO
deadline notifier for headless completion, shared timeline time for rendered fades,
and independent renderer/GPU final-use ownership. No authored state, new DTO or polling
loop is introduced. Caller paths use the same typed Stop Parameter.

## Verified scope

- Full native-feature app library:728 passed,8 explicitly ignored. Generated contracts
  match. The first run found two tests still expecting immediate Layer cuts; retained
  cut coverage now authors zero duration, and Video checks its timed outgoing deadline.
- Updated scoped-stop cohort:15 passed, including script delivery, per-Cell/Column/Layer
  overrides and dormant metadata, Linear/Smooth samples, zero duration, whole interrupted
  mixes, repeated Stop, unrelated Layers, reactivation, headless application/Video holds,
  and immediate Column cuts during a Layer fade. Two new test attempts needed the ordinary
  script stabilization/receipt observation already used by the owning fixture; final
  assertions pass with those public boundaries observed.
- Matrix-rendering software preflight:8 passed,11 native cases explicitly ignored.
- Strict all-target native/application-process-feature Clippy passes; UI check has zero
  errors/warnings. Root and Golden Core formatting pass.

- Both hardware adapters pass the finite native Layer fixture: Intel alpha251→224→0,
  NVIDIA252→225→0.24 independent whole images/18,432 texels cover geometry/Undo and
  completion through two outputs and production preview; maximum residual0 bytes.
  Four partial-alpha samples establish advancing fades, not a complete timing envelope.
  The unchanged90-second watchdog and renderer retirement checks pass; the test exits0
  in12.89s. Overlay uses the user's standing Off instruction.
- The current bundled app's control fixture passes and closes normally: real grid and
  generic Inspector controls at all six scopes, exact Parameter ownership, HTTP
  entry paths,30 rapid transport stop cycles, sparse creation, history and project reload.
  It does not qualify GPU timing or final-use process retirement. The
  [receipt](evidence/mapgyver-m14-layer-stop-2026-11-17.json) records tested scope,
  executable hashes, adapter samples and remaining acceptance.

This is feature delivery, not complete M14/10 qualification. Full timed-alpha envelopes,
all override/caller native combinations, process and final-GPU-use retirement qualification
and deletion/project/shared-consumer cases remain required before acceptance. The gate is
Pending in the second-pass queue; M14/8 is the next feature. Prior recovery/performance
debt remains parked. No acceptance item is awarded by this milestone.
