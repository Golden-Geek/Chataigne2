# P11 actual-app State graph scale

Golden UI owns the persistent node index. The State Machine panel owns its graph adapter and
uses public Golden canvas operations. This milestone measures that production panel against the
default native application's authenticated transport and real project loader; it does not fabricate
UI snapshots. The [fixed fixture contract](../../apps/chataigne/ui/tests/browser/app-graph-scale/README.md)
records source provenance, shape, sample counts, mounting bounds and timing limits.

## Index repair

The original index stored every key beneath seven hash levels. The valid connected State fixture
exceeded the normal 200ms stall limit at 50k backend nodes. A CPU profile attributed about 18.1s
of 24.9s active JavaScript to persistent index reads. A bounded 32-slot read cache was insufficient:
the unchanged 50k three-node move still stalled for 204.1ms. That cache is removed.

The index now creates branches only where hashes share a prefix, bounded by seven levels. Full
hash collisions retain separate complete node identities in one leaf. Forks share the trie and
retire the original ownership token so writes to either side copy shared paths. Changed-key
traversal handles leaf/branch shape changes without reporting unchanged entries. There is no
whole-index copy or retained read cache.

A direct allocation comparison at 50k sequential keys uses 66,941 trie nodes versus 275,372 at
`2f3b4dde` (about 76% fewer), and copies five versus nine nodes for one replacement. At 100k,
the corresponding counts are 132,195 versus 527,442 and five versus nine. These are structural
work counts from the actual current/baseline classes, not a wall-clock or total-memory claim.

Owning tests cover generation identity, full hash collisions, removal/clear, mutations to both
sides of a fork, changed-key reconciliation and 50k allocation bounds. The existing staged
600-node insertion test retains its identical frame count across 1k/10k/100k bases and atomic
publication assertions. Compact paths naturally allocate different node counts at different
depths; its old near-identical-allocation assertion becomes a fixed architectural bound of
eight trie nodes per changed key, including the changed parent. Timing and mounting gates are
unchanged.

## Workload and remaining acceptance

The five tiers contain exactly 1k/10k/20k/50k/100k retained backend tree nodes, corresponding to
124/1,249/2,499/6,249/12,499 State graph items. Each State except the last has one directed
transition to the next. Declaration expansion is calibrated using the real backend before timing;
these tiers need no unrelated padding. There is one active State in the connected network.
Setup, loading and declaration calibration are reported separately from ordinary action latency.

Discovery/reveal, single movement, three-item movement and directed connection creation each
retain 31 samples after three warmups. Every sample checks exact backend identities and source
values, focused DOM, visible geometry or stroked SVG and drained canvas/admission work. Connections
are undone between samples. The fixture observes public operations used by actual handlers;
physical keyboard/mouse usability retains P17.

The initial all-active source fixture triggered normalization of 1,249 States after connection
creation, repeatedly capturing the full tree. Its failure and trace are preserved. Correcting
activity preserves density and gates; it does not repair that bulk-normalization cost. P13/P16
must measure and resolve that production path separately.

State scale alone cannot close the graph checkpoint. Formula and Spatializer actual-app scale,
then composed discovery/selection/editing workflows remain required. P11 stays four of six fixed
checkpoints complete (67% rounded); whole-plan coverage stays 59%. P03 remains complete.

## Verification

The final default executable passes all **20 action/tier combinations**, retaining **620 samples
and 60 warmups. The table reports the maximum per-action p95 feedback/completion and maximum
callback stall at each tier (milliseconds), plus peak mounted graph nodes including warmups.

| Backend tree nodes | State items | Feedback p95 | Completion p95 | Maximum stall | Peak nodes |
| -----------------: | ----------: | -----------: | -------------: | ------------: | ---------: |
|              1,000 |         124 |         11.0 |           34.9 |           8.4 |          3 |
|             10,000 |       1,249 |         20.9 |          196.0 |          16.8 |          8 |
|             20,000 |       2,499 |         38.5 |          479.2 |          46.0 |          8 |
|             50,000 |       6,249 |         70.0 |         1304.1 |         141.8 |          8 |
|            100,000 |      12,499 |        164.3 |         2799.9 |         304.0 |          8 |

All fixed feedback, completion, stall, 128-node and 32-finder-row gates pass. No tracing, CPU
profiling or partial tier/action selection was enabled. The first full successful run is also
retained separately; it used the prior default executable with unchanged backend source.

The current root UI suite passes **446 cases** (27 audio / 58 graph / 199 Golden / 162 app).
App and graph-fixture Svelte checks report zero errors/warnings. Both Cargo formatters, package
Prettier and whitespace checks pass. Rust source is unchanged; no new backend-suite or Clippy
run is claimed for this milestone.

The normal locked build rebuilt the frontend in **2m24s**. PowerShell's stderr redirection
reported a failing wrapper status despite Cargo's Finished result; an explicit native-exit
confirmation passed in **2m11s**, reusing the unchanged frontend bundle. Both used
`CARGO_INCREMENTAL=0` and an unset `GC_SKIP_UI_BUILD`. Final binary SHA-256:
`4f2af35bfa0e16379ca997d455d6c9f8ccadcdd4c5de8eacd63cfc156fc2527f`.
That exact artifact passes the full scale matrix and all **four State / five Alchemist /
13 Spatializer native regression groups**. No other validation ran during timed scale cases.

Evidence: [final native matrix](evidence/p11-state-graph-scale-native-2026-10-26.jsonl),
[first successful matrix](evidence/p11-state-graph-scale-prebuild-2026-10-26.jsonl),
[seven preserved failures/diagnostics](evidence/p11-state-graph-scale-failures-2026-10-26.json),
[State regression](evidence/p11-state-graph-scale-state-regression-2026-10-26.jsonl),
[Alchemist regression](evidence/p11-state-graph-scale-formula-regression-2026-10-26.jsonl),
[Spatializer regression](evidence/p11-state-graph-scale-spatializer-regression-2026-10-26.jsonl)
and [source/build/check record](evidence/p11-state-graph-scale-source-2026-10-26.json).

```powershell
npm test
npm run check --workspace chataigne-ui
npm run check:browser:graphs --workspace chataigne-ui
cargo fmt --all
cargo fmt --manifest-path crates/golden_core/Cargo.toml --all
$env:CARGO_INCREMENTAL='0'
# GC_SKIP_UI_BUILD must be unset.
cargo build --locked
node apps/chataigne/ui/tests/browser/state-graph-scale-native.mjs
```
