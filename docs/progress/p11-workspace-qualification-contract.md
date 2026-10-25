# P11 workspace qualification contract

P11 has six fixed acceptance checkpoints derived from the active plan: manager/catalog windowing,
historical log windowing, dashboard viewport awareness, tree focus/keyboard navigation, graph
keyboard operations, and complete composed workflow qualification. Delivery commits do not add
checkpoints. Partial consumer work does not close a checkpoint that still has required consumers.

The first collection milestone qualifies shared row windowing in actual ManagerListPanel,
OutlinerPanel and NodePickerModalHost. Creation-catalog menus remain part of the first checkpoint;
this milestone does not claim that all P11 collections or keyboard workflows are complete.

## Collection workload and gates

Before formal timing runs, fix 1k/10k baselines, 20k/50k normal and 100k stress collection sizes.
Each graph fixture has that many manager items plus ten structural/nested nodes. Two actual panels
share a workbench session, and a picker uses the same graph. The manager/outliner fixture is
70rem wide and 36rem high; variable-height rows use a 30rem x 32rem viewport. The private browser
viewport is 1680 x 1200. Manager rows include 4rem supplements; generic variable rows are 2/4rem.
Do not shrink these dimensions or remove tall rows after failure.

Each list mounts at most 42 rows, including overscan and one pinned focused row. Test scrolling,
far selection, search and clearing search, hidden-row hoisting, expansion beyond the initial
depth, rejected expansion, unmount/remount, and focus retention/fallback. Confirm actual row text,
selection, rectangles and source identity; a data-count attribute alone is insufficient.

Measure manager filtering, composed manager/outliner selection and picker filtering with three
warmups and 31 retained samples per action/tier. Record verified first DOM/layout feedback,
completion and animation callback intervals/tails. Normal feedback p95 and maximum callback stall
must be <=200ms; stress ceilings are <=400ms. Completion p95 must be <=2,000ms normal / 4,000ms
stress. These are operation-feedback/stall gates, not a 60-FPS or physical-input claim. Setup and
fixture construction are separately recorded. The fixed canvas frame gates remain P10 evidence.

Use production Svelte/Vite, an owned headless browser and component/store operations used by UI
handlers. No synthetic keyboard/mouse input or desktop control is permitted. DOM focus and scroll
geometry are inspected in the isolated fixture. Native input/usability remains P17; full P11
keyboard and broader composed-workflow acceptance stays open until its own required cases pass.

Retain failures and raw samples. Do not relax limits or reduce density to obtain a pass. Source,
bundle, tool and machine evidence must identify what was actually exercised. Later collection,
dashboard and keyboard milestones add their explicit workloads before qualifying them.
