# Implementation status

This is the only current execution ledger for the
[implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md). When asked to
“implement the plan”, start there and resume from this ledger. Historical ledgers and their
pause/branch/push instructions are [archived](../archive/2026-10-18/README.md).

Last reconciled: 2026-10-18. Source baseline: `49d65b0fe71f66b73d338ebe026a572a53c7dcf6`,
branch `codex/builtin-mapping`. At audit start the only local modification was the user's main
plan. Its complete contents, including the uncommitted clarification, survive in the
[previous plan](../archive/2026-10-18/plan/Chataigne2_Codex_Implementation_Plan.md).

## Current handoff

- Current batch: documentation consolidation **complete**; ready for “implement the plan”.
- Scope selected by the user: **foundation and readiness**. Full lighting/video/projection
  authoring is deferred; current features and completed Mapping remain preservation contracts.
- Runtime/source implementation under the new P tasks: **not started**.
- Next implementation action: **P00**, reconcile the then-current HEAD/diff/toolchain, then P01.
- Known current failures: reproduced edit-prefix undo loss; advisory gate fails on Rustls 0.23.42.
  Other findings below include source-confirmed risks with explicit reproducer work still needed.
- No source implementation, device opening, commit, push, or remote publication was performed
  by the documentation consolidation. The historical source-edit pause is superseded as active
  guidance; a future “implement the plan” instruction starts the scoped work above.

## Task ledger

Implementation states: `NOT_STARTED`, `IN_PROGRESS`, `IMPLEMENTED`, `VERIFIED_LOCAL`.
Check outcomes: `PENDING`, `PASSED`, `FAILED`, `UNAVAILABLE`, `SKIPPED_WITH_REASON`.
External qualification is separate from implementation. A historical pass is not a current pass.

| Task                        | Implementation | Local validation | External qualification / evidence                                                       |
| --------------------------- | -------------- | ---------------- | --------------------------------------------------------------------------------------- |
| P00 Baseline                | NOT_STARTED    | PENDING          | Reconcile new checkout before source work                                               |
| P01 Edit failures/history   | NOT_STARTED    | FAILED baseline  | Valid add + invalid Folder edit reproduced mutation without undo                        |
| P02 Schedule/actor failures | NOT_STARTED    | PENDING          | Source-confirmed; production reorder/overload regression needed                         |
| P03 Bounded I/O/lifecycle   | NOT_STARTED    | PENDING          | HTTP stop/join, UDP admission, payload weights, engine/lifecycle bounds, generator time |
| P04 Audio order/types       | NOT_STARTED    | PENDING          | Legal interleaving and invalid deserialization established from source                  |
| P05 Transport trust         | NOT_STARTED    | PENDING          | Missing admission verified; exploit not demonstrated                                    |
| P06 Dependencies            | NOT_STARTED    | FAILED baseline  | Four Rustls advisories on audit date; refresh DB before repair                          |
| P07 DMX streaming           | NOT_STARTED    | PENDING          | Refresh/filter/service gaps; protocol loopback qualification needed                     |
| P08 Formula backend intents | NOT_STARTED    | PENDING          | UI polling/identity and mutation policy verified; corruption not reproduced             |
| P09 Graph document          | NOT_STARTED    | PENDING          | Ordered-map mutation costs source-derived                                               |
| P10 Canvas                  | NOT_STARTED    | PENDING          | Projection diagnostic and crossing-edge reproduction; mounted performance open          |
| P11 Workspace UX            | NOT_STARTED    | PENDING          | Collection/keyboard gaps source-reviewed; native usability open                         |
| P12 Boundaries/gates        | NOT_STARTED    | PENDING          | App policy leak, omitted graph tests, oversized files, unsafe protocol conversions      |
| P13 Formula execution       | NOT_STARTED    | PENDING          | Historical profiles retained; production parallelism not mandated                       |
| P14 Production time         | NOT_STARTED    | PENDING          | Clock helper unwired, no target-frame play, starvation/latency policy gaps              |
| P15 Assets                  | NOT_STARTED    | PENDING          | Audio path/cache identity exists; project asset boundary unimplemented                  |
| P16 Product scale           | NOT_STARTED    | PENDING          | Historical full-product gates remain open; 20k/50k/mounted canvas required              |
| P17 Platforms/artifacts     | NOT_STARTED    | PENDING          | Windows history retained; exact current app/native/platform checks required             |
| P18 Named devices           | NOT_STARTED    | PENDING          | Exact authorized ASIO4ALL/VB-Audio route must be verified first                         |

For each implementation batch append a compact row under “New implementation evidence” with
task, source/tree, changed ownership, invariant, exact commands, environment/features, outcome,
evidence path, limitations, and next task. Keep substantial raw measurements in the existing
qualification artifacts and link them. Do not grow another chronological execution plan here.

## Preserved completed baselines

| Baseline                     | Disposition and evidence boundary                                                                                  |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| September T00–T14            | Prior fixes retained. New queue/canvas/advisory findings are new scope, not deletion of old passes.                |
| T15 reusable ownership       | Completed; public consumer checks recorded. Preserve dependency boundaries.                                        |
| T16 ordinary audio artifacts | Implemented and Windows-qualified at its recorded source. Hardware/native/platform gates remain open.              |
| T17 readability              | Partial at prior stop. The old 43-file count was dated; use the refreshed inventory below.                         |
| T18 profiling                | Real kernels and test-only workers measured; actual production commit/cancellation/full-tick benefit unresolved.   |
| T19 product                  | Open. Load/idle, synthetic projection, and outliner tests do not qualify a mounted canvas or every live operation. |
| Mapping corrective R00–R08   | Complete; N01–N24 passed at `ae49424e`. No remaining corrective phase.                                             |
| Mapping historical Phase 11  | Superseded authoring contract. Its old “in progress” handoff is historical only.                                   |
| July multiplex recovery      | Completed for the July slice; retain behavior and requalify current product.                                       |

Use the [archive index](../archive/2026-10-18/README.md) for all original matrices, exact SHAs,
commands, raw evidence references, and historical remote observations. Direct feature-branch
pushes did not trigger required CI; recorded `NOT_APPLICABLE_WITH_REASON` is not “CI passed”.

## October audit observations

These observations were made during the 2026-10-18 audit of the source baseline above.
They are carried evidence from that session, **not checks rerun by this documentation edit**.

| Check                                                                                                                                      | Outcome                           | Scope / limits                                                                        |
| ------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------- | ------------------------------------------------------------------------------------- |
| `cargo test --locked -p golden_engine -p golden_runtime -p golden_graph -p golden_transport_server -p golden_persistence -p golden_script` | PASSED: 548, 7 ignored            | Includes integration/doc tests; does not establish full app/native readiness          |
| `cargo test --locked -p golden_audio --no-default-features --features analysis,playback`                                                   | PASSED: 132 unit + 21 integration | Includes 12 managed/playback cases; no native devices                                 |
| `cargo test --locked -p golden_audio --no-default-features`                                                                                | PASSED: 99 unit + 6 integration   | Subset/alternate feature evidence; do not add overlapping tests into a unique total   |
| `cargo test --locked -p chataigne_alchemist`                                                                                               | PASSED: 179                       | No current full-app or complete state-machine claim follows                           |
| `npm test`                                                                                                                                 | PASSED: 136                       | Audio 27, Golden UI 17, app 92; root omitted Golden Graph UI                          |
| `npm exec -- vitest run --root packages/golden-graph-ui`                                                                                   | PASSED: 13                        | Separately invoked; P12 adds it to root coverage                                      |
| `npm run check`                                                                                                                            | PASSED                            | Zero Svelte errors/warnings; audio generated binding check passed                     |
| `npm run build`                                                                                                                            | PASSED with chunk warning         | Largest client chunk 1,419.69 kB minified / 430.08 kB gzip                            |
| `python -m unittest discover -s tools/qualification/tests`                                                                                 | PASSED: 63                        | Qualification parser/tool tests, not product qualification                            |
| Root and Golden Core `cargo fmt --all --check`                                                                                             | PASSED                            | Read-only formatting checks                                                           |
| `cargo deny --locked check advisories`                                                                                                     | FAILED                            | Rustls 0.23.42: RUSTSEC-2026-0285, 0346, 0348, 0349; then-current fixed floor 0.23.46 |

The default application feature-tree inspection found no Rustls Brotli feature. Advisory 0348
requires that feature; no application exploit or custom-session-secret logging path was established.
Advisory sources: [0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html),
[0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html),
[0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html),
[0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html).

### Reproduced and diagnostic evidence

- **P01:** an stdin-compiled probe linked to the current validated engine artifact queued valid
  AddNode then invalid SetParam on the root Folder. Observed `before_nodes=1 after_nodes=2
undo_len=0`, `ParamEditTargetMismatch { edit_index: 1, ... }`. The executable was a disposable
  `target/audit-engine-partial-batch.exe`; create a durable owning regression during P01.
- **P10:** a direct call to the real visible-edge selector omitted a crossing wire with both
  endpoints offscreen. The later regression must cover route bounds, not only endpoint boxes.
- **P10:** a Node diagnostic using actual effective-node projection, ID map, and spatial index
  construction measured six retained iterations after two warmups: 10k 2.97–6.51 ms, 50k
  10.41–19.39 ms, 100k 26.09–50.94 ms. This excludes routing/Svelte/DOM/paint; no retained
  benchmark script or browser p95 is claimed. Reproduce under P10/P16 before using as a gate.
- **P14:** eight default 128-frame output queue blocks at 48 kHz represent 21.3 ms capacity;
  a 2,048-frame input bridge target represents 42.7 ms. These are arithmetic capacity/target
  observations, not measured command-to-audible or round-trip latency.

No physical device, hotplug, native usability, browser exploit, full native application build,
long-running current-product soak, or cross-platform qualification was performed by this audit.

## Historical measurement cautions

- Latest September workbench-root `virtual-outliner-final20` evidence was 288 ms p95 at 10k
  and 2,109 ms at 100k, with 41 mounted rows and one Long Task. Earlier better samples do
  not replace it. No mounted graph-canvas capacity follows from that harness.
- The 100k-lane unchanged-input probe requested every lane. Roughly 95–107 ms was requested
  evaluation cost, not idle-engine time; accepted effects did not replay.
- Test-only parallel speedups excluded the complete generation/commit boundary. Smaller real
  workloads spent only part of their tick in kernels; use full-tick benefit for decisions.
- Authored reload/root/Constant identity checks did not prove every generated descendant UUID
  stable. Client-observed save/edit overlap did not alone prove server capture concurrency.
- Mapping's superseded 125-second construction observation was followed by recorded repairs;
  do not reopen it by copying only an intermediate “remaining cost” paragraph.

## Documentation consolidation checks

- PASSED: 159 local links/anchors across 44 Markdown files, including the archives. Two existing
  toolchain-manifest links were corrected during cleanup.
- PASSED: one file in `docs/plan/`, one file in `docs/progress/`, 19 matching P-task headings/
  ledger rows, all 16 historical findings mapped, and an acyclic dependency graph.
- PASSED: all nine archived documents retain their text apart from archive notices and the
  T18 evidence heading. The archived user-edited plan retains the exact 61-added/3-removed
  line diff present at task start; no historical evidence was discarded.
- PASSED: refreshed source inventory, 1,324 scanned files and 46 over 1,000 lines.
- PASSED: configured Prettier check on all 19 maintained/modified Markdown files. Archived
  evidence payloads were preserved rather than reformatted; the new archive index was checked.
- PASSED: root and Golden Core `cargo fmt --all --check`; documentation whitespace/diff scope
  checked. Runtime tests were not rerun for documentation-only changes.

This completes documentation reconciliation only. No P01–P18 source fix or media foundation is
claimed implemented. The next implementation session starts at P00 and preserves this handoff.

## New implementation evidence

None yet. P00 is the next implementation action. The
[maintained source-size inventory](../reference/source-size-inventory.md) is a reference,
not another execution ledger.
