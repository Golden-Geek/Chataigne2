# Matrix playback acceptance

**M07/2 Accepted — 2026-11-03.** Matrix owns playback intents, shared-time mixes, source binding
and consumer demand under `apps/mapgyver/src/matrix`. Golden supplies time, bounded publication,
render completion and owned-process mechanisms. Playback remains ephemeral and never enters
authored history or project files. [Model acceptance](mapgyver-m07-model.md) is unchanged.

## Original requirement audit

| Requirement | Inspected proof |
| --- | --- |
| Cell → Column → Layer precedence; 250 ms default; curves and Cut | Current Layer/Cell/playback suites plus `playback_acceptance`: a real default three-Layer Matrix uses a 17 ms Cell override, 500 ms SmoothStep Column override, then 100/900/700 ms Layer fallbacks. Exact envelopes and independent endpoints share the same interrupt anchor. Default Layer declarations remain 250 ms; Cut bypasses timed admission safely. |
| Independent Cell triggers | `cell_playback` and four-caller parity preserve the Column anchor and peer Layers, including direct activation without an anchor. Current bundled controls confirm the independent request/marker and actual process behavior. |
| Atomic Column launches | The complete request is admitted/staged once; configured bursts, foreign-column rejection and 32-contributor recovery leave the previous request intact on failure. The combined three-Layer test verifies simultaneous starts from actual interrupted mixes. |
| Disabled and empty policy | Typed Cell Enabled and Golden hierarchy enablement hold previous content/consumer demand or skip direct activation. Enabled empty destinations fade to transparency without creating nodes; authoring a playing empty address does not activate new content until relaunch. Current Cell/native and process receipts are retained. |
| Interrupted blends and shared time | Timeline and render-plan tests assert exact sampled weights, new clip identity, stable source sharing, no fade restart from repeated reads and one completion refresh. Current native Layer/Group/Final observations verify three-color mixture preservation and monotonic incoming/outgoing weights on both GPUs. |
| Outgoing source and application demand | `retained_demand`, rendering retention/application and runtime suites cover independent deadlines, interruption, reorder/delete/Undo, missing definitions, disabled/transparent Layers, project replacement and final-use holds. Existing current-build Windows tests preserve real process identity through logical demand and renderer completion, then retire it after the final release. |
| Ephemeral state and unchanged bounds | Exact project/history comparisons pass after launch bursts and the combined precedence/interruption scene. Existing 32-contributor, 4,096 binding, publication-credit, retained-owner and plan limits remain unchanged. Full native/scale/timing qualification remains M07/4. |

## Current verification

[Receipt](evidence/mapgyver-m07-playback-acceptance-2026-11-03.json): **386 library tests pass**,
two retained exclusions, in **141.94 seconds**. Strict app all-target/native Clippy and both Cargo
formatters pass. The unchanged production code retains the previous **28 UI tests**, Svelte 0/0,
**12 Windows process cases** and bundled independent Cell/typed disabled-destination PID-hold/
Unload evidence. Their executable hashes are unchanged and recorded in this receipt; they were
not rerun just to increase test counts.

The native test passes in **35.12 seconds** on Intel Graphics and NVIDIA GeForce RTX 5080 Laptop
GPU. Each adapter observes three accepted interrupted-mixture samples through Layer, Group and
Final outputs: **18 accepted three-color image observations** total. Initial/final colors also
pass **16 complete 32×24 output/preview images**, with zero nominal/residual pixel error. All
sampled texels are spatially uniform, alpha coverage and the original color-weight/ratio bounds
hold, and the original finite readback, plan/resource and completion-safe retirement checks pass.

The existing uniform-color fixture now authors each source at 32×24: Cell media keeps its own
size, so the removed implicit 1×1 stretch cannot define this scene. Owned References add alias
records; endpoint retirement counts the original four distinct producers rather than raw channel
records. Neither change raises a resource limit or weakens an image/weight tolerance. The native
baseline passes without a repair experiment. The broader native Matrix scene remains unaccepted.

**Gate change: M07/2 Accepted.** M07 is 2/4; the fixed plan total is **44/57 gates (77%)**, with
**5/11 fully accepted phases (45%)**. M07/3 is the single next Active gate. Its full-panel square
grid, complete contextual operations, naming/removal, drag sort/reparent and presentation remain
required. M07/4 and retained physical/timing/final qualification remain open, with original budgets.
