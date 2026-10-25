# P10 mounted canvas qualification contract

This fixes the final P10 workload and provisional gates before formal qualification runs.
The [implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md#p10--keep-canvas-interaction-incremental)
and [performance contract](../operations/performance.md) remain authoritative. Earlier development
and production dense-wire probes are diagnostics, not qualification passes.

## Surface and workload

Build the actual generic GraphCanvas fixture with production Svelte/Vite and serve the generated
bundle in an isolated headless browser. Record OS, CPU, browser/tool versions, source hashes,
authored nodes/edges and actual mounted node/edge counts. Keep 1k/10k baselines, 20k/50k normal
tiers and a separate 100k stress tier. Use the same spaced grid/chain with explicit crossing,
loop and obstacle geometry; normal viewport is 100rem x 60rem at zoom 1, stress also exercises
zoom 0.5. Framing/Home may expose substantially more nodes; record those counts rather than
shrinking the view to obtain a pass.

The grid has 64 columns with 16rem horizontal and 6rem vertical spacing. Each node is explicitly
13rem x 5rem with one input and one output; adjacent identities form a chain. Four additional
nodes provide a separate crossing/loop/obstacle corridor at negative Y. Ordinary normal views
mount 84 nodes initially; stress zoom 0.5 exposes more. Home retains the same minimum zoom 0.2
and may mount over 1,000 nodes. These are presentation workloads, not backend graph generators.

Each action gets three warmups and 31 retained samples: single/multi-node move preview and commit,
pan, anchored zoom, Frame Selection, Home, live activity, payload editing and changing obstacles.
Verify actual node geometry/text/classes, camera anchors, SVG stroke/path correctness, identities
and source acknowledgement. Record frame intervals continuously around actions and route/camera
completion separately. Keep the 1,100-wire cache-overflow scene and include varied paths as well
as parallel wires. Functional crossing/loop/obstacle cases remain required.
The parallel dense view has two nodes. The varied dense view has 12 source and 12 destination
nodes in columns, two ports each, four intervening obstacles and 1,100 authored edges across
576 distinct endpoint pairs. Both keep the 1,024-route cache cap and display overflow as curves.

Calls enter the same presentation/edit/navigation operations used by event handlers; no synthetic
keyboard/mouse input or desktop control is permitted. Fixture edit callbacks acknowledge explicit
positions without choosing backend defaults or mutation policy. Backend transaction/history and
transport evidence stays with P08/P09; composed-app qualification stays with P16 and native
input/webview usability with P17. A graph-count attribute or two animation frames alone is not
acceptance evidence.

## Gates and measurement boundaries

| Metric                                                   | Baseline / normal tiers |               100k stress |
| -------------------------------------------------------- | ----------------------: | ------------------------: |
| Ordinary-action first DOM/layout feedback p95            |                   200ms |                     400ms |
| Animation callback interval p95                          |                  33.4ms |                    66.8ms |
| Maximum observed callback interval                       |                   200ms |                     400ms |
| Camera/edit/route completion p95                         |                 2,000ms |                   4,000ms |
| 1,100-wire routing completion, parallel and varied paths |                 2,000ms | Separate dense-view probe |

The ordinary feedback gate comes from the plan. Frame gates allow two nominal 60Hz slots for
normal views and four for stress while retaining hard stall observations; they do not assert
display FPS. Completion includes the existing 240ms camera animation and deferred routing.
Stress doubles feedback/completion ceilings because it is a separate capacity tier. The dense
routing ceiling requires a clear improvement over the recorded eight-second development case;
historical failures keep their original scope and never become retroactive passes.

Feedback starts before the operation call and ends after Svelte flush plus a verified DOM/style
or SVG geometry change. Framing additionally observes the first changed camera view. Completion
ends only after the expected edit/camera state and route queue settle. Retain feedback/completion
samples and animation intervals (including tails), not only their percentiles. Fixture generation,
initial document construction and bundle compilation are outside ordinary-action timings but
their scene setup durations are recorded. Compositor paint, physical input delivery, full app
widgets and native webviews remain outside this fixture's claims.

If a gate fails, retain the failure, diagnose and fix the implementation. Do not drop an action,
reduce visible density or relax a threshold to turn the same run green. The execution ledger
receives completion credit only when the required matrix and owning gates pass.
