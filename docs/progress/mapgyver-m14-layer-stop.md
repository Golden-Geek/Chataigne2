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

The initial feature delivery left timed native envelopes and shared process retirement for
the second pass. Prior recovery and performance work remains in the plan's return queue.

## Second-pass verification, 2026-11-20

- The current scoped app tests passed all 15 cases. Four current Matrix software
  integration tests passed, including unchanged request and caller behavior.
- The native fixture sampled the production compositor at controlled shared-clock times.
  Transport and script Stop both passed default, dormant override, Column override,
  Cell override and zero-duration cases. It also verified an interrupted two-source
  mix, reactivation before the old deadline and unchanged peer pixels.
- Both adapters passed 222 complete images (170,496 texels) across two independent
  Shared Texture outputs and the production preview. All samples stayed within the
  existing conversion bounds. The run completed in 39.58 seconds with physical output
  windows closed. Receipts: `target/mapgyver-m14-10-envelope-preflight-163616.log` and
  `target/mapgyver-m14-10-native-envelope-164116.log`.
- The controlled-completion process fixture and strict lint passed. Holding the submitted
  plan kept the shared process alive after logical demand ended, and completion released it.
  Receipts: `target/mapgyver-m14-10-process-preflight-164413.log` and
  `target/mapgyver-m14-10-strict-164838.log`.
- The shared-process test initially sampled alpha 171 at a nominal halfway point because
  the stepped renderer clock and the application deadline's elapsed wall time differed.
  The corrected lifecycle test uses the advancing renderer clock and waits for outgoing
  demand to expire. The controlled-time pixel assertions still measure the exact fade curves.
  Failed attempt: `target/mapgyver-m14-10-native-final-165024.log`.
- Intel and NVIDIA verified that the owned process stayed alive during the fade and while
  the peer remained active. Stop All withdrew the last output and retired the process.
  Both renderers returned to zero live plans, sources and outputs. The combined run passed
  240 complete images (184,320 texels) with zero samples outside the oracle in 41.69 seconds.
  It used two independent Spout outputs and the production preview with physical display
  windows closed. The corrected software preflight, strict Clippy and both formatters passed.
  Receipts: `target/mapgyver-m14-10-clock-preflight-165853.log`,
  `target/mapgyver-m14-10-clock-strict-170322.log` and
  `target/mapgyver-m14-10-native-clock-170433.log`.

M14/10 is Accepted. The bundled app and caller checks, both-GPU timed pixels, and held
completion/process lifetime checks cover the Layer Stop contract.
