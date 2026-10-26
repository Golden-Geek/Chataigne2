# P11 actual-app Formula graph scale

Alchemist owns Formula graph policy and its production editor in the app layer. Golden provides
the public canvas, session and snapshot-free mutation APIs. The app's native qualification fixture
uses those real operations and the default executable's authenticated transport and project loader.
The [fixed workload](../../apps/chataigne/ui/tests/browser/app-graph-scale/README.md#formula-contract)
preserves the State tier sizes, viewport, 3/31 sample counts, deadlines and timing/mounting limits.

The complete untraced matrix now passes on the normal bundled executable. The retained
[qualification index](evidence/p11-formula-graph-scale-2026-10-26.json) links all raw timing
samples, source/asset/binary fingerprints and final native regression evidence.

## Workload and observations

The exact 1k/10k/20k/50k/100k starting backend-tree tiers contain
34/344/689/1,724/3,448 ANodes. Backend calibration measures 29 tree records per item with zero fixed
offset; only the remainder (14/24/19/4/8 records) is unrelated padding. Backend tags must identify
one Constant and otherwise Math nodes. Every chain edge's UUID and socket is checked: Constant
`value`, then Math `result`, feed the next Math's `value1`. Graph items, connections and backend
records are distinct dimensions.

Discovery/reveal, single movement, three-item movement and connection creation use the production
editor. The connection case adds the penultimate Math's result to the final Math's free `value2`,
then undoes between samples. Dynamic input growth remains enabled. Each sample checks the exact
new backend identity/socket, visible stroked SVG and retained focus on the initially revealed node.
The original observer incorrectly expected source focus for Formula; the connection had already
committed and rendered with destination focus. The corrected observer preserves State behavior,
where the revealed node and connection source are the same identity.

State and Formula share native orchestration and measurement. Their app fixtures supply public
operations and actual backend observations. Formula's read-only measurement index reconciles
changed backend records and releases deleted/replaced edges; it does not scan the complete tree
on every frame. An owning test covers parameter-only changes, unrelated updates retaining edge
identity, removal and reused IDs after project replacement.

## Load regression and repair

The first full run passed 1k and 10k actions, but 10k loading took **52.913s** and 20k loading
exceeded the existing **120s transport timeout**. The instrumented 10k diagnostic took **54.284s**;
2,482 full-size process snapshots consumed **41.151s** in clone/count/index work. Formula socket
sync itself measured about 19ms auto-input work, 19ms materialization and 7ms type solving there.
The trace identifies snapshot requests as the dominant load cost for this fixture.

ANode config refresh used ordinary mutation callbacks despite only reading and writing the supplied
parameter. Each requested a whole-tree snapshot that subsequent edits invalidated. Both writable
and read-only branches now use the existing public `call_node_mutation_without_snapshot` API.
Property-manager role persistence and property-value color refresh use the same target-local path.
All callback bodies preserve their existing validation, constraints, defaults, control modes,
read-only handling and authored values. The core engine contract is unchanged.

A new owning Rust regression fails against the prior implementation: real Math/Constant ready
callbacks request snapshots for their config refresh. It checks writable and read-only contexts,
applies the queued mutations and compares the materialized authored ANode before/after. The
initial optimized test run retains its expected red result. Focused repair checks use the
repository's `cargo test-fast` dev profile; normal bundled builds and native qualification remain
separate from skip-UI test compilation.

The repaired 10k diagnostic loads in **0.875s**, versus 54.284s before. Across each complete
load/discovery diagnostic, full-size snapshots fall from **2,482 to 81**, with clone/count/index
time falling from 41.151s to 1.238s. Snapshot totals include discovery, rather than being isolated
load-only counts. All 51 Formula tests and 24 operation tests pass in the dev profile; three
existing manual/native operation cases remain ignored there. Strict all-target Clippy passes.

The shared setup deadline is cumulative across loading, projection and canvas readiness. The
existing 120s transport timeout and 300s setup budget are unchanged. Instrumented and partial
runs remain diagnostics; they cannot close scale qualification.

## Frontend projection repair

The first repaired full run passes 20k operations, then fails 50k single movement with a 249.9ms
maximum stall. A separate CPU profile attributes **4.343s of 7.351s active JavaScript** to edge
projection. The panel's connection-ID set accounts for 2.210s; the editor's edge projection accounts
for 2.136s. Both rebuild backend socket models that node presentation already produced.

Connection selection now reads the backend Formula's connection identities directly. The editor
projects node sockets once and shares their index with edge colors and connection admission.
Activity and preview overlays consume those models without introducing another socket projection.
The obsolete backend socket rebuilding helper and unused catalog argument are removed, including
the clipboard path's unused dependency. Backend connection policy remains authoritative.

Owning UI tests retain component identities/colors and missing/incompatible socket behavior and
reject edge projection that walks ANode descendants again. The isolated 50k movement probe passes
with a **108.4ms** maximum stall. This partial probe is evidence for the repair, not full acceptance.

## Connection completion investigation

The next full run keeps 50k movement stalls within 83–108ms, but connection completion p95 is
**5,708.7ms**, exceeding the unchanged 2,000ms normal limit. A separate single-operation 50k
diagnostic observes **5,800.1ms** connect and **2,053.4ms** undo. It starts at 50,000 backend nodes,
temporarily reaches 50,009 (the new connection plus automatic third input), and restores 50,000
after undo. This is phase attribution, not a one-sample latency qualification.

The trace spends about **one second in automatic input counting per socket reconciliation**.
The existing pass scans every Formula connection separately for each Math/Concatenate node.
The current repair builds one incoming-socket index per immutable snapshot pass and reuses the
same count policy for local and bulk callers. A pending-write index also avoids quadratic scans
when many count parameters need changes. Explicit counts, skip targets, numbered/component
sockets, 2–64 bounds, growth/shrink and duplicate pending writes retain their existing rules.
Owning tests compare local and bulk results on actual materialized Math/Concatenate fixtures;
full repaired scale qualification is still required.

The indexed 50k diagnostic observes **2,496.4ms** connect and **947.0ms** undo. Automatic counting
falls from roughly 1,000ms to **4–9ms per reconciliation**. All 52 Formula tests and 24 operation
tests pass; strict Clippy also passes for that repair. Connection completion is still above the
normal limit, so this diagnostic does not close acceptance.

Two further full snapshots belong to connection init/ready even though init only adjusts
permissions and ready is empty. Those phases now opt out while attachment retains endpoint
binding. The resulting 50k diagnostic observes **2,158.7ms** connect and **947.3ms** undo.
The generic opt-in checkpoint trace measures about **15ms** per checkpoint, so checkpoint
cloning does not explain the remaining connection cost. The trace still contains three socket
reconciliations during connection creation and automatic input growth. The passive wire trace
shows project-load recovery finishing before connection creation; the connection publishes a
delta rather than requesting another complete snapshot. These observations and the indexed
comparison are [retained](evidence/p11-formula-graph-scale-connection-indexed-2026-10-26.json).
The engine dev-profile suite passes **635 tests / three existing ignored**. Final qualification
remains pending.

The subsequent untraced full run passes every action through 20k and discovery/movement at 50k,
then fails 50k connection completion at **p95 2,325.3ms** (maximum UI stall **112.5ms**, passing).
Its [failure samples](evidence/p11-formula-graph-scale-lifecycle-failure-2026-10-26.json) remain
evidence against acceptance. Reconciliation currently solves types and then compilation solves
the same materialized graph again for warnings. Compilation now exposes its resolution to the
adapter so socket presentation and validation share that work, including invalid graphs with
repairable type information. All **183 Alchemist library tests** and **76 Formula/operation tests**
pass for shared resolution reuse.

Automatic input growth also exposed an obsolete reconciliation pass: it queued a new count and
immediately materialized, solved and validated the previous count's graph. A typed awaiting-counts
outcome now defers compilation and diagnostic publication to the resulting parameter events in
the existing engine stabilization boundary. It does not add a timer or a new validation phase.
The owning test exercises pending count repair followed by resumed compilation on a real Formula.
All **77 Formula/operation tests pass / three existing cases remain ignored**. Strict all-target
Clippy passes for Chataigne2, Alchemist and the engine. The normal locked bundled build passes.
The new traced diagnostic observes **1,734.6ms** connection completion and **885.2ms** undo,
retaining 50,000 -> 50,009 -> 50,000 backend nodes. Its first connection reconciliation now
queues only count repair in 6.5ms; compilation resumes after the count edit. The
[diagnostic and owning checks](evidence/p11-formula-graph-scale-count-settlement-2026-10-26.json)
do not qualify distributions.

## Complete qualification

All **20 action/tier combinations / 620 retained samples / 60 warmups** pass the unchanged
feedback, completion, stall and mounting gates. Both complete selections are recorded and all
diagnostic/profiling flags are off. Backend IDs, UUIDs, every chain socket, actual parameters,
focused DOM, visible geometry/SVG and drained canvas work are checked.

| Backend nodes | ANodes | Chain edges | Worst completion p95 | Maximum callback stall | Peak mounted nodes |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 34 | 33 | 46.0ms | 4.6ms | 5 |
| 10,000 | 344 | 343 | 341.6ms | 16.7ms | 13 |
| 20,000 | 689 | 688 | 695.9ms | 29.2ms | 13 |
| 50,000 | 1,724 | 1,723 | 1,758.2ms | 104.4ms | 13 |
| 100,000 | 3,448 | 3,447 | 3,541.5ms | 191.7ms | 13 |

The measured machine is Windows x64, Intel Core Ultra 9 275HX with 24 logical processors;
physical memory and OS release are retained in the index. The normal dev-profile executable
SHA-256 is `e1e3aaab79d8cf7f573609839ab710376e8d8862bf8470b4cde9eb4071a93981`.
This artifact also passes **five Formula / four State / 13 Spatializer** native correctness
groups and the four-action **1k State smoke tier** using the shared runner. The prior complete
State scale qualification remains separate; the smoke run does not claim to repeat it.
UI suites pass **449 cases** (27 audio, 58 graph, 199 Golden, 165 app), with zero app/native-fixture
Svelte errors or warnings. These checks qualify authoring actions and callback stalls;
physical input, native webview usability and runtime evaluation throughput retain later owners.

## Remaining acceptance

Formula actual-app scale is complete locally. P11 remains four of six fixed checkpoints complete
(67% rounded); whole-plan coverage remains 59%. Spatializer scale is still required to close
the graph checkpoint, followed by
composed discovery/selection/editing workflows. Runtime evaluation throughput retains P13/P16;
physical-input usability remains P17. The separate all-active State normalization cost remains
unresolved under P13/P16.
