# P11 dashboard framing and retained presentation

Golden UI preserves control identity and layout while dashboard presentation changes between
ordinary detail and bounded previews. The backend continues to own authored layout and edits.
This delivery advances the dashboard checkpoint; it does not close P11 or qualify native input.

## Ownership and behavior

`canvas/frame-preparation.ts` captures selected ancestry and validates UUIDs, parent links and
child order through asynchronous preparation. Frames expose inactive tabs and open accordion
members through surface view operations. They prepare at most 32 pending targets per batch.
Inactive tab measurement panes occupy the active pane's rectangle and remain hidden/inert.
Free leaf geometry stays logical; Frame does not mount every selected free control.

`frame-bounds.ts` projects bounds through nested clips and preserves DOMRect prototype getters.
Local scroll owners reveal selected controls before the camera frames their represented area.
For a bulk selection extending beyond scroll content, the containing viewport represents that
axis; the cross axis still uses actual control bounds. This is distinct from centering the final
member of a multi-selection.

`flow-layout.ts` preserves omitted stack extents through bounded spacers, including mixed CSS
units, disabled explicit dimensions, auto sizes and gaps. Each slot is rounded using the existing
CSS serialization before aggregation. Grid previews retain original cells. The shared retention
owner observes focus before DOM removal and includes tab/accordion headers in the bounded native
range cohort. A sticky child-browser anchor stays reachable through local scrolling. The
UUID-keyed page view cache preserves last-observed scroll positions through unmounting and drops
retired identities. These changes belong to generic presentation, not app/domain policy.

## Qualification

The full production browser runner passes **69 correctness groups and five sparse tiers**,
including **18 new lifecycle/framing groups**. All **76 frozen source fingerprints** match the
delivered files. The [complete report](evidence/p11-dashboard-framing-2026-10-25.json) records
assets, browser and machine. The [13 failed probes](evidence/p11-dashboard-framing-failures-2026-10-25.json)
retain source/asset identities and diagnostic failures. Prior sparse, dense and hierarchy gates
remain in the full run, including bounded free ancestry preparation and depth-eight framing.

New cases verify browser-header focus retirement, an inactive selected tab among 600 members,
48-member flow preparation, retained horizontal/vertical/grid geometry, mixed units and auto
sizes, exact native range text across budget changes in five non-free layouts, local scroll
round trips, four 100-member Frame operations and cancellation after source reorder.

The 100-member cases observe every target during preparation. Peak mounted shells are **49**
for horizontal, vertical and accordion, and **34** for tabs. Selection-area center errors are
below **0.000031 CSS pixels**, within the existing 0.5-pixel tolerance. Inactive tab pane
rectangles coincide exactly. Native range release returns to three shells (two for tabs).
The horizontal scroll round trip preserves `scrollLeft = 123984` and a visible browser header.
Mixed-unit geometry differs by 0.187 CSS pixels, within the unchanged tolerance.

Failed probes exposed focus falling to the document, inactive-tab omission, lost omitted-flow
extents, invalid spacer CSS, missing closed-header range text, a scrolled-away browser header
and DOMRect coordinates lost by object spreading. Diagnostic runs also exposed incorrect
single-member/bulk-selection and accordion cross-axis expectations; those oracles were corrected
to inspect the represented selection area while retaining the original tolerance.

Root UI tests pass **307 cases**: audio 27 / graph 52 / Golden UI 113 / app 115. Eleven new unit
cases cover path identity/structure fencing, flow extents and clipped bounds, including inherited
DOMRect properties. Browser and app Svelte checks report zero errors/warnings. Owned Prettier,
whitespace and both Cargo formatters pass. The locked default app build rebuilds the frontend
and passes in **2m22s**. Executable SHA-256:
`6bdbdadd94c1b95d0f3616dc859e3ee6fa115b0959d1365f3ad1fcf9574312d0`.
Inventory is 2,125 source files / 27 oversized. DashboardCanvas remains 945 lines; every touched
runtime/test source stays below 1,000 lines, with no new exception.

## Remaining scope

Continue the remaining edit/drop lifecycle matrix and then the fixed three-warmup / 31-sample
action matrix for discovery, selection, edits, navigation, framing and exact dense picking at
every tier. These correctness observations contain no latency distributions. Preserve all
existing feedback, stall and completion gates. Physical gestures/usability remain P17;
whole-product throughput remains P16.

Reproduce with `npm run test:browser:dashboard --workspace golden_ui`.
`-- --lifecycle-only` selects the 18 new diagnostic groups and does not replace the full run.
**P11 remains 2/6 checkpoints (33% rounded); whole-plan scope coverage remains 57%.**
