# P11 dense dashboard presentation

Golden UI owns this presentation change. The backend still owns authored positions, sizing,
bindings and transactions. This delivery continues the dashboard checkpoint; it does not close it.
The [architecture](../architecture/dashboard-ui.md) explains ownership and the
[fixed contract](p11-workspace-qualification-contract.md) defines the budgets before qualification.

## Delivered behavior

Free surfaces mount complete detailed views only when at most 192 widgets intersect the
overscanned viewport and the spatial query proves completeness within 2,048 candidate visits.
Otherwise an explicitly labeled overview replaces ordinary controls. It never treats an arbitrary
candidate sample as the complete dashboard. The overview draws at most 256 conservative bounds
from a retained spatial hierarchy. Counts describe authored members of those bounds, including
members outside the viewport. Camera refreshes do not enumerate each dense group's members.
Membership changes rebuild the hierarchy; a geometry edit repairs the affected ancestor bounds.

Up to 16 of the latest selected direct children keep actual controls, alongside existing focused,
pointer, native-range and framing retention. Logical bulk selection remains complete. Selecting
an overview position resolves exact authored paint order independently of the approximate groups.
The overview's keyboard activation resolves the viewport center. Removing an active overview or
retiring a focused control returns focus to the page after reconciliation.

The overlap probes also exposed an existing stacking bug: a label from an earlier widget could
paint above a later sibling's controls. Each widget shell now isolates its own stacking context.
Authored paint order, actual DOM hit testing and logical activation agree in the qualified scenes.

## Correctness evidence

The production browser runner passes **34 correctness groups plus five sparse scale tiers**:
the preceding 14 sparse groups and 20 added dense groups. The private Edge browser uses the
existing 1680 x 1200 scene and real DashboardCanvas/WorkbenchSession. Operations call the same
component/store methods used by handlers, with native DOM focus and geometry inspection; no
synthetic keyboard or mouse input is used. Source/asset/machine records and failed probes are
retained in the [full run](evidence/p11-dashboard-dense-2026-10-25.json) and
[failed probes](evidence/p11-dashboard-dense-failures-2026-10-25.json). The delivered source has
56 frozen fingerprints. Edge is 156.0.4314.42; the recorded reference machine remains Windows
10.0.26300 / Intel Core Ultra 9 275HX / 24 logical CPUs / Node 26.5.0.

| Target authored nodes | Actual authored nodes | Widgets | Overlap groups, edit and run | Bulk mounted controls, including focus |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 1,001 | 90 | Detailed view | 89 after one removal |
| 10,000 | 10,010 | 909 | 256 | 17 |
| 20,000 | 20,009 | 1,818 | 256 | 17 |
| 50,000 | 50,006 | 4,545 | 256 | 17 |
| 100,000 | 100,001 | 9,090 | 256 | 17 |

Every overlap case verifies exact foremost identity, real control content/size, authored slot
fit and `elementFromPoint` reachability. Reorder and removal change the foremost identity
correctly. Bulk selection, focused input identity, live position edits, distant navigation,
reversal and retirement pass in edit and run modes. A focused widget moved beyond the authored
page remains mounted in run mode but is still clipped; that case does not claim visible output
outside the page.

The 40-column grid is also exercised at minimum supported zoom, 0.25. It is a viewport slice,
not a view of every authored widget. Editing views contain 280 actually visible widgets at
the four larger tiers; bounded groups conservatively cover every visible authored rectangle.
Run mode retains the page clip and its detailed controls. Browser serialization of the overview
origin introduces about 0.0011 CSS-pixel containment error, within the existing 0.5-pixel geometry
tolerance. The unit model verifies exact coverage before DOM serialization.

Seven new unit cases cover 100k overlapping group members, negative/grid geometry, conservative
coverage, delta repair, removal/rebuild/clear, invalid budgets, threshold behavior and exact hit
order under reorder/live edits. The root UI suite passes **292 cases**: audio 27 / graph 52 /
Golden UI 98 / app 115. App and browser checks report zero errors and warnings.
Owned Prettier, whitespace and root/Golden Core Cargo formatting pass. The locked default
build rebuilds the frontend and passes in **2m26s**. Executable SHA-256:
`1743501915cc5656b0a1f5478a0f7242c871701615c5db360599cdb2fea31230`.

Failed probes preserve the label-stacking defect, the initial strict floating-point DOM oracle
and the overview retirement focus defect. The geometry oracle now measures serialized bounds
in CSS pixels against the existing tolerance; the runtime fixes stacking and remembers retiring
overview focus before DOM removal. No density, mounting or interaction requirement was reduced.

## Remaining checkpoint scope

These bounds apply per free surface. Simultaneous nested free surfaces, flow-layout composition,
the complete hierarchical/gesture matrix and the prescribed warmups/31-sample action/timing
qualification still remain before dashboard closure. Dense picking currently performs an exact
spatial point query and may inspect many coincident candidates; the timing matrix must measure
that action. Setup observations are not latency distributions or frame-rate acceptance.
Physical input/usability remains P17; product throughput remains P16.

Reproduce from the repository root with `npm run test:browser:dashboard --workspace golden_ui`.
`-- --dense-only` runs only the 20 dense diagnostics; the full delivery uses both sparse and dense.
**P11 stays 2/6 checkpoints (33% rounded); whole-plan scope coverage stays 57%.**
