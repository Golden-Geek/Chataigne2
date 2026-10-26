# Actual-app graph scale fixtures

The State, Formula and Spatializer project prototypes are canonical project documents exported
by the owning Rust/native cases at `2f3b4dde`. The first two include their actual adapter-created
connection/transition from the same native roundtrip. Spatializer uses the initial authored
Voronoi scene. These JSON files are persisted test data, not alternate protocol declarations.
`projects.mjs` clones owned UUIDs and rewrites references in those existing records; the actual
app's project loader reconstructs declarations, validates and runs lifecycle. No UI protocol
snapshot or graph adapter result is fabricated.

Measure retained backend tree nodes separately from graph items, declared fields, unrelated
records, sources/targets and generated matrix cells. State/Formula have several backend records
per graph item. Spatializer's per-list cap remains 4,096; never call 100k endpoints a supported
Spatializer workload. Prototype expansion, loading and readiness are setup observations and
are excluded from ordinary-action distributions.

State's fixed native contract uses 1k/10k baseline, 20k/50k normal and 100k stress backend tree
nodes. Two small calibration loads (8 and 16 items) establish declaration counts before timing;
active state/transition chains consume that budget, with only the sub-item remainder filled by
unrelated Folder records. It is a real State graph, not a sparse graph hidden in a large unrelated
project. All state items use a 256-column, 24rem grid, with one directed transition to the next item.
The exact item/edge/tree counts and serialized source hashes are recorded for every tier.

Preserve one active State in the connected network by cloning the backend-normalized active and
inactive prototypes. The initial all-active clone was invalid as an ordinary connected-network
fixture: adding a transition normalized 1,249 States and exposed repeated whole-tree snapshot
cost. Its 10k failure and diagnostic trace remain evidence for bulk-normalization performance;
correcting activity does not change graph density, tier sizes, operations or timing gates.

Use the production StateMachinePanel in its unchanged 70rem by 36rem fixture and an owned
1680 by 1200 headless browser against the default executable. Four action/tier combinations
measure far label discovery/reveal, single movement, three-item movement and a new directed
connection. Every combination retains 31 samples after three warmups. Each sample checks exact
backend identities, source values, focused DOM, visible geometry or SVG, and drained canvas work.
One measured connection is undone between samples. Mount at most 128 graph nodes and 32 finder
rows. Normal gates are p95 first DOM/layout feedback <=200ms, completion <=2,000ms and maximum
animation callback stall <=200ms; stress gates are 400ms/4,000ms/400ms. Setup has a fixed 300s
deadline. Preserve failures without reducing density or relaxing gates.

Run `node apps/chataigne/ui/tests/browser/state-graph-scale-native.mjs` after the default locked
build. `GC_GRAPH_SCALE_TIERS=1000` selects a smoke tier; partial tier selection is explicitly
reported and cannot close qualification. No input synthesis or desktop control is used.
`GC_GRAPH_SCALE_ACTIONS` similarly selects only a subset of operations. `GC_GRAPH_SCALE_TRACE=1`
and `GC_GRAPH_SCALE_CPU_PROFILE=1` enable passive diagnostics; instrumented runs are not timing
qualification, even if they happen to pass. All selections and diagnostic flags appear in the report.
State scale alone cannot close graph keyboard qualification: Formula, Spatializer and composed
workflows retain their required owners and evidence.

## Formula contract

Use the same tiers, dimensions, grid, warmups, retained samples, deadlines, mounting bounds and
normal/stress timing limits as State. Calibrate declaration expansion with eight and sixteen
ANodes, then fill each exact backend-tree tier with one Constant followed by Math ANodes and a
directed chain. Only the sub-item remainder may be unrelated Folder records. Validate every source
and target UUID and socket: the Constant's `value` and each Math's `result` feed the next Math's
`value1`. Report ANode/connection counts separately from retained backend nodes.
The 300s setup budget is cumulative across loading, projection and canvas readiness; the
transport's existing 120s project-load timeout remains unchanged.

Measure the same four actions. Connection creation adds the penultimate Math's `result` to the
last Math's free `value2`, preserving an acyclic graph and the existing `value1` connection.
Require the exact new backend identity, target socket and visible stroked SVG, then undo between
samples. The fixture's read-only connection index reconciles changed backend records, rather than
scanning the complete tree on every measurement frame. Production Alchemist remains responsible
for its graph adapter and operations.

Run `node apps/chataigne/ui/tests/browser/formula-graph-scale-native.mjs`. State and Formula share
the orchestration and measurement owners; each app fixture supplies its real public operations
and backend observations. Formula scale does not establish runtime evaluation throughput, Spatializer
scale, composed workflow acceptance or physical-input usability.
