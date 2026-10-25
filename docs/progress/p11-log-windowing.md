# P11 historical log windowing

Golden UI's LoggerPanel owns history projection, filters, native text selection and live-follow
presentation. VirtualRows owns mounted membership, variable-height geometry, logical reveal and
end anchoring. The session logger owns record publication and backend/UI record ordering. Domain
logging, retention capacity, wire contracts and application behavior remain at their existing owners.

## Implementation

Paused and filtered views now use the same bounded renderer as live history. The old 200-row
live-tail render shortcut is removed; all retained collapsed records remain reachable. An
unlimited-source snapshot displays its retained records without the old hidden 128-row fallback.
Backend repeat counts, expanded-repeat budgets and global duplicate grouping retain their existing
semantics. Separate backend records are not merged by the ordinary Collapse view.

Focus resolves a logical record key before mounting it. Follow-latest anchors the end through
height measurements; active native text selection pauses following. The panel retains a bounded
selection cohort, with endpoints prioritized and at most 64 record rows pinned. Single multiline
and adjacent-record native ranges survive distant scrolling and append. Native selection works
within this retained DOM cohort; **Copy logs** is the bulk-history path and formats every filtered
logical entry independently of mounting. Removed/filtered focused rows return focus to the list.

The logger store now publishes raw, readonly arrays instead of deep reactive proxies. Queued
backend changes edit one detached buffer and publish a replacement array. UI appends, repeat
replacement, clear and trimming preserve prior published histories. Synchronously flushing a
queued batch also cancels its outstanding timer. Inactive commented controls and their unused
handlers/styles were removed from the panel; transport, update-rate and retention policy did not move.

## Qualification

The [fixed contract](p11-workspace-qualification-contract.md#historical-log-extension) precedes
timing. Actual LoggerPanel and WorkbenchSession run in a private production Edge scene, 60rem x
32rem, with 1k/10k baseline, 20k/50k normal and 100k stress records. Long multiline messages use
measured heights. Ordinary mounting stays within 42 rows; selection cases stay within 112,
including up to 64 retained selection rows and focused-row retention.

Correctness covers distant history, live/paused append, actual row text and bounds, focused DOM
retention, multiline/four-adjacent-record native selection, combined source/tag/content filtering,
focus fallback, backend repeat expansion/global grouping, unlimited history, eviction and clear.
Separate browser probes prove five store-publication invariants: backend append, replacement,
clear, trimming and UI repeat replacement preserve earlier snapshots.

Filtering and a four-record UI append batch each retain three warmups and 31 measured samples at
every tier: **310 measurements, 30 warmups**. All ten cases pass fixed feedback/stall/completion
gates. Values below are feedback / completion p95 in milliseconds. Setup is a separately recorded
single observation per tier, not a latency distribution or a full-product snapshot benchmark.

| Records | Filter p95 | Four-record append p95 | Setup ready |
| ------- | ---------- | ---------------------- | ----------- |
| 1k | 4.7 / 12.8 | 1.9 / 8.6 | 19.7ms |
| 10k | 5.3 / 12.7 | 5.9 / 8.5 | 13.3ms |
| 20k | 5.6 / 12.6 | 9.0 / 12.7 | 26.3ms |
| 50k | 8.6 / 16.7 | 23.5 / 27.4 | 50.8ms |
| 100k stress | 22.8 / 29.4 | 53.0 / 55.3 | 101.6ms |

The maximum callback interval is 54.2ms across the final log timing cases. This qualifies operation
feedback and completion, without a 60-FPS or physical-input claim. The same source passes the
complete catalog and manager/picker/outliner regressions: another **620 measurements and 60
warmups**, plus variable-row correctness. All timing runs execute separately from other checks.

- [Final logs, source-store invariants and all samples](evidence/p11-logs-2026-10-25.jsonl).
- [Catalog regression](evidence/p11-logs-catalog-regression-2026-10-25.jsonl).
- [Manager/picker/outliner regression](evidence/p11-logs-collection-regression-2026-10-25.jsonl).
- [Source, asset, machine and tool record](evidence/p11-logs-source-2026-10-25.json).

The [append failure](evidence/p11-logs-append-baseline-2026-10-25.jsonl) preserves the delivered
deep-reactive store's 50k feedback p95 **534.1ms** and maximum callback interval **570.9ms**.
Replacing proxy-backed history copies with raw snapshot publication reduces the final observed
feedback to **23.5ms**, using the same workload and limits. The earlier
[fixture failure](evidence/p11-logs-fixture-baseline-2026-10-25.jsonl) preserves an incorrect
duplicate oracle; it was corrected to use the backend's repeat_count contract.

`npm test` passes **261 UI cases**. App `check` and Golden UI `check:browser` report zero errors
and warnings. Owning formatting and source-size checks pass. The default locked app build is
complete in **3m15s**, rebuilding the frontend. Executable SHA-256 is
`652448bb3e017be5d39f2d5bbd68c9ee0d49f6a2f2b96ee5e7461508e8dcff02`.
Root/Golden Core Cargo formatting and whitespace checks pass. No Rust runtime or protocol
declarations changed.

This closes P11's historical-log checkpoint: **2/6 complete (33% rounded)**. Dashboard viewport
awareness, tree keyboard navigation, graph keyboard operations and complete composed workflows
remain. Native usability and full-product throughput/snapshot timing retain P17/P16 ownership.
