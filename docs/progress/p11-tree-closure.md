# P11 tree interaction qualification

The shared tree delivery in `81adcc37` is now qualified in its app consumers and constrained picker.
This closes the fourth fixed P11 checkpoint locally. Graph keyboard operations and complete composed
workflows remain separate checkpoints. Physical input/native focus usability remains P17.

## Ownership and fixes

Golden UI owns tree projection, navigation, focus and caller-supplied picker constraints. Chataigne
owns module, Formula, property and processor policies and supplements. The app fixture imports
Golden's supported `testing/session` and `testing/collections` entries; Golden runtime/test sources
do not import Chataigne. The property fixture mounts the complete production AlchemistEditorPanel.

Actual property-panel mounting exposed an unbounded layout: a 1k-item manager grew to 28,813 CSS
pixels and mounted all 1k rows inside a 544-pixel parent. ManagerListPanel now participates in flex
layout with a zero minimum block size, giving its scroller the actual available viewport. The fixed
scene mounts 26 property rows, 27 Formula rows and 28 module/processor rows. The 42-row limit is unchanged.

Formula source badges previously searched every library item for absent settings. App-owned
formulaSource now consumes Golden's existing structural-child index, including external-file and
Preferences lookup. A regression proves fewer than 12 record reads with 100k real item-root entries.
The library skips its extra source predicate when all source categories are enabled. Partial source
filtering still runs through the same app policy and is checked at all five tiers.

The picker now revalidates root ancestry, source UUID, current global/view/row constraints and
projection options on activation and confirmation. A new candidate clears the previous candidate's
projection. A required multiple-choice projection must be chosen explicitly; double activation cannot
silently choose its first option. A single required option still aligns automatically. Optional/clear,
immediate-pick and cancellation behavior is retained. Root/session replacement cancels the request.

Picker focus cycles within currently mounted dialog controls, including the roving tree row.
Closing restores the original live focus owner when the dialog still owns focus; pending autofocus
cannot overtake a replacement request. These are DOM/component checks, not physical-key evidence.

## Fixed workloads and evidence

The [contract](p11-workspace-qualification-contract.md#app-consumer-and-constrained-picker-closure)
fixes 1k/10k baseline, 20k/50k normal and 100k stress collections, the 70rem x 36rem app scene,
1680 x 1200 browser, full app supplements/private-child exclusions and the existing timing gates.
App user items carry the backend's `itemRoot` role; early fixture roles were corrected without
changing density, scene size or timing limits.

| Evidence                                                                          | Qualified scope                                                                                                                                                                                                                                                                                                                  |
| --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [App consumers](evidence/p11-tree-closure-app-trees-2026-10-26.jsonl)             | 20 consumer/tier correctness groups; 40 timing combinations, 1,240 samples / 120 warmups. Module, Formula library and processor panels use their production navigation operations. The full property editor uses public session selection/reveal and metadata expansion operations; shared keyboard handling has its own matrix. |
| [Constrained picker](evidence/p11-tree-closure-picker-2026-10-26.jsonl)           | 55 correctness groups across five tiers: disabled branches, hidden-row hoisting, view/empty filtering, outside-root candidates, multiple/single/optional/invalid projections, immediate picks, clear, live rejection, UUID changes, request replacement, focus cycling and restoration.                                          |
| [Shared keyboard regression](evidence/p11-tree-closure-keyboard-2026-10-26.jsonl) | 30 navigation/activation/lifecycle groups and 30 timing combinations, 930 samples / 90 warmups. Source replacement now waits for cancelled picker retirement.                                                                                                                                                                    |
| [Collection regression](evidence/p11-tree-closure-collections-2026-10-26.jsonl)   | Five collection tiers, variable-row retention/removal and 15 timing combinations, 465 samples / 45 warmups.                                                                                                                                                                                                                      |

All **85 timing combinations / 2,635 retained samples / 255 warmups** pass. Across 20k/50k normal
cases, worst per-case feedback p95 is **34.1ms**, completion p95 **41.6ms** and maximum callback
interval **33.4ms**. At 100k stress these are **67.5ms / 75ms / 183.2ms**. Normal limits remain
200ms / 2,000ms / 200ms; stress limits remain 400ms / 4,000ms / 400ms. All browser runs report zero
page errors. No input events, desktop automation, forced collection or profiler run is used in these timings.

[Failed rows](evidence/p11-tree-closure-failures-2026-10-26.jsonl) preserve the layout regression,
early Formula timings and the stale-projection confirmation failure. Early app timing rows also
record their incorrect regular/item-root fixture classification. Profiling is diagnostic only;
no failed run is promoted to qualification by changing limits or a baseline.

## Reproduction

```powershell
npm test
npm run check --workspace chataigne-ui
npm run check:browser:trees --workspace chataigne-ui
npm run check:browser --workspace golden_ui
npm run test:browser:trees --workspace chataigne-ui
npm run test:browser --workspace golden_ui -- --constrained-picker
npm run test:browser --workspace golden_ui -- --tree-keyboard
npm run test:browser --workspace golden_ui
```

The app runner's `--profile-formulas` path is a separate diagnostic. Qualified reports use the
complete default app matrix. Source, bundle, machine and default artifact fingerprints are in the
[source/build record](evidence/p11-tree-closure-source-2026-10-26.json).

Root UI suites pass **396 cases** (27 audio / 52 graph / 191 Golden / 126 app); app and both browser
Svelte checks report zero errors/warnings. The locked default app build passes in **2m41s**, including
a fresh frontend. Both Cargo formatters, owned-source formatting and whitespace checks pass.
The inventory is **2,191 sources / 27 oversized**, with no touched source over 1,000 lines.
