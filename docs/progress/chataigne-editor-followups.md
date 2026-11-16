# Deferred Chataigne editor work

Updated 2026-11-16. The user prioritizes MapGyver product delivery. Noncritical Curve,
Spatializer and other Chataigne-only consumer work is deferred until Chataigne work resumes.
Do not automatically restore these patches or require their qualification for MapGyver.
Shared defects reproduced on an active MapGyver path remain actionable there.

## Spatializer selection and custom-world cancellation

Local named stash: `deferred/chataigne-spatializer-world-input-2026-11-16 user-prioritizes-MapGyver`.
Exact object: `2c7fe7faa7f2ddcd7ac6c1727faaa2e9a99122e7`, based on `8350a82f`.
It contains only the owning source, two new tests and related architecture/report edits;
unrelated user work was excluded. Inspect its tracked and untracked payload before restoring
into a dedicated Chataigne checkout. This local stash is not a published implementation.

Baseline: ordinary bundled Chataigne mounted three radius controls with no endpoint selected.
The patch mounts controls only for selection, separates selection from mutation admission,
uses ordinary Parameter value-access policy, and adds public `GraphCanvas.onWorldInputCancel`
for tool-first Escape, pointer cancellation, capture loss, blur and disposal.

Before deferral, 36 Spatializer and 60 graph unit tests passed, as did graph keyboard browser
checks, Chataigne/MapGyver/graph-fixture typechecks, scoped Prettier and both default app builds.
The corrected ordinary-app Spatializer check passed: exact Inspector before release, retained
SVG/capture, selection-only controls, tool-first/idle Escape, background owner, exact saved
project and unchanged history. The paired State Machine/Alchemist product regression passed.
Physical/native pixels, exhaustive lifetime/read-only cases and whole-gate qualification were
not established. Preview overlays obstruct Home until hidden; retain that navigation issue.

Receipts retained locally:

- `target/m14-spatializer-selection-baseline-2026-11-16-final.log` (reproduced failure).
- `target/m14-spatializer-selection-product-fixed-2026-11-16.log` (exit 0), SHA256
  `658966fde67d07e37bf0f647a78ce01eabc5839bc216a92f7eb769a78c273773`.
- `target/qualification/m14-spatializer-selection-product-7e783a77-c9c0-4e37-ab06-44312c52954b/browser.jsonl`.
- `target/m14-spatializer-graph-regression-product-2026-11-16.log` (exit 0), SHA256
  `a704cf8a9103516d20c77154ff7b25e6e73b0aa870b71a642d96b4296074050e`.

Tested Chataigne binary SHA256:
`2700ab64d156b7957cf4f9035a8a6e901984483697a5084e118d93c71a68fc87`.
Disk binaries can still include the parked patch; rebuild from the intended checkout before
further qualification. Next Chataigne action: review the stash against current public canvas
contracts, restore intentionally, then complete the original unverified cases without a new
diagnostic framework.

## Curve

Exact key/segment-to-Inspector selection is already published in `d2fd83b5`.
The separate connected drag/capture failure remains unresolved: after press the receipt showed
`captured:false`, `focused:true`, `activeTag:CANVAS`. Later motion/history is not accepted.
See [the existing owning report](mapgyver-m14-selection.md) for tested scope and source limits,
and `target/m14-curve-selection-race-connected-2026-11-15/failure.json` for the failure.
Return on a source-supported ownership correction or the required actual-host/native check
when Chataigne is selected. Do not restart unchanged capture/profiler variants.
Failed cache stashes `1371a7bda9d03f1f637889dbab90c25c33de4781` and
`a2d2c197a68ccbdf06cd35a7485d125fa28d6445` remain un-restored and unaccepted.
