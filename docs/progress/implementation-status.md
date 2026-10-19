# Implementation status

This is the only current execution ledger for the
[implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md). When asked to
“implement the plan”, start there and resume from this ledger. Historical ledgers and their
pause/branch/push instructions are [archived](../archive/2026-10-18/README.md).

Last reconciled: 2026-10-19 on clean `main` at delivered `2c24b119`, matching fetched `origin/main`.
Initial audit source baseline: `49d65b0fe71f66b73d338ebe026a572a53c7dcf6`,
branch `codex/builtin-mapping`. At audit start the only local modification was the user's main
plan. Its complete contents, including the uncommitted clarification, survive in the
[previous plan](../archive/2026-10-18/plan/Chataigne2_Codex_Implementation_Plan.md).

## Current handoff

- Current batch: P03 batch 29 native outgoing completion logging verified locally.
  Milestone commit/push follows the checks below; reconcile delivery from Git on resume.
  Batch 28 serial retained catalog/passive ownership delivered in `2c24b119`.
  Batch 27 native connection/discovery ownership delivered in `2066f735`.
  Batch 26 shared TCP/WebSocket projection receipts delivered in `d2e8764c`.
  Batch 25 WebSocket server passive integration delivered in `86f380c8`.
  Batch 24 native ownership delivered in `b991a9ea`.
  Batch 23 WebSocket client native/passive ownership delivered in `e9230f86`.
  Batch 22 TCP server passive integration delivered in `5a2cb368`.
  Batch 21 native transport ownership delivered in `01158b6d`.
  Batch 20 HTTP applied-configuration admission/bounds delivered in `36e69aeb`.
  Batch 19 passive HTTP service delivered in `0edb6f48`.
  Batch 18 retained HTTP ownership delivered in `7836d1bf`.
  Batch 17 callback lifetime admission delivered in `8c2c5ff8`.
  Batch 16 worker input/generation isolation delivered in `3166e805`.
  Batch 15 native ownership and disabled completion service delivered in `1663106e`.
  Batch 14 was delivered in `0196c0f8`; batch 13 in `df0c7876`; batch 12 in `a617d313`. Separately committed
  footer UI parent `3b4bdc7b` and build-cache ancestor `5b81c44e` are preserved.
- Delivery: `f02205af3baa1c2ebf49527104364980c3a7b920` committed the verified P01/P02/P03-through-batch-7
  milestone and milestone delivery policy. `codex/builtin-mapping` was fast-forwarded into `main`
  and both remote branches were pushed successfully on 2026-10-19. Unfinished shared-stream
  baseline work was preserved and restored on `main`. Earlier no-commit/no-push notes below
  describe historical actions only; future verified milestones are committed and pushed.
- P00 resumed check: clean `main` and `origin/main` at delivered `2c24b119`;
  no unrelated changes or live owned jobs at resume. Previous turn: progress.
- Scope selected by the user: **foundation and readiness**. Full lighting/video/projection
  authoring is deferred; current features and completed Mapping remain preservation contracts.
- Runtime/source implementation under the new P tasks: **P01 and P02 verified locally; P03 in progress**.
- Next implementation action: continue P03 admission/disposal: shared stream script/command
  argument capture/materialization and disposal; HTTP request conversion/disposal; then Sound Card
  and remaining module/channel/generator ownership. Serial now has bounded retained discovery
  catalogs/options, subscriber/producer notifications and coarse recovery, native prepared input
  generations, passive callback/feedback receipts and accepted-prefix retirement. Preserve its
  applied-configuration admission, no-replay writes and destroy/undo wake fencing.
- TCP/WebSocket Server share one app feedback component with verified discarded/partial projection
  recovery and newer desired state retained during an in-flight projection. Preserve each protocol's
  target IDs, client-parameter factory, native limits, callback receipts and accepted-prefix lifecycle.
- P03 remaining scope includes shared stream script/command argument materialization, logging and
  disposal admission; HTTP request argument conversion/disposal; Sound Card stop/join, event drain,
  admission/recovery and configuration bounds; the remaining module/channel audit; and generator
  elapsed-time accounting. Do not claim the module audit complete from one stream repair.
- Preserve delivered HTTP/UDP/TCP/WebSocket native limits, original-destination cursors, prepared
  input and canonical callback lifetimes, passive receipts, applied-snapshot admission, accepted
  prefix retirement, disabled completions, rejected-envelope parser gaps, retry deadlines and
  destroy/undo wake fencing. WebSocket server also preserves per-peer grammar. Both servers use receipt-owned
  connection/warning/client projections, including stable IDs after partial failure. Detailed
  regressions and measurements remain under their implementation batches below.
- Focused native outgoing completion work passed; requested return to High for routine closeout
  and subsequent work. Actual effort remains unchanged/unobservable, with no supported control.
- Known current failures: the advisory gate's Rustls 0.23.42 failure has not been refreshed.
  The reproduced edit-prefix undo loss is fixed locally. Compound creation now uses prepared
  validation/commit; its reference/dashboard regressions and optimized package checks pass.
  Other findings below still need their owning checks.
- No source implementation, device opening, commit, push, or remote publication was performed
  by the documentation consolidation. The historical source-edit pause is superseded as active
  guidance; a future “implement the plan” instruction starts the scoped work above.

## Task ledger

Implementation states: `NOT_STARTED`, `IN_PROGRESS`, `IMPLEMENTED`, `VERIFIED_LOCAL`.
Check outcomes: `PENDING`, `PASSED`, `FAILED`, `UNAVAILABLE`, `SKIPPED_WITH_REASON`.
External qualification is separate from implementation. A historical pass is not a current pass.

| Task                        | Implementation | Local validation    | External qualification / evidence                                                                                         |
| --------------------------- | -------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| P00 Baseline                | VERIFIED_LOCAL | PASSED              | HEAD `2e1f392551ee6d4b9a5859bc3c62945e18811fe4`; clean after consolidation committed during reconciliation                |
| P01 Edit failures/history   | VERIFIED_LOCAL | PASSED batches 1–8  | Applied-prefix/atomic/history rejection, callback/teardown preservation, and next edit/tick/save verified                 |
| P02 Schedule/actor failures | VERIFIED_LOCAL | PASSED batches 1-3  | Dependency order, generation fencing, typed errors, disconnect history, and later-save recovery verified                  |
| P03 Bounded I/O/lifecycle   | IN_PROGRESS    | PASSED batches 1-29 | Native stream logging/serial ownership verified; broader admission/module/generator audit open                            |
| P04 Audio order/types       | NOT_STARTED    | PENDING             | Legal interleaving and invalid deserialization established from source                                                    |
| P05 Transport trust         | NOT_STARTED    | PENDING             | Missing admission verified; exploit not demonstrated                                                                      |
| P06 Dependencies            | NOT_STARTED    | FAILED baseline     | Four Rustls advisories on audit date; refresh DB before repair                                                            |
| P07 DMX streaming           | NOT_STARTED    | PENDING             | Refresh/filter/service gaps; protocol loopback qualification needed                                                       |
| P08 Formula backend intents | NOT_STARTED    | PENDING             | UI polling/identity and mutation policy verified; corruption not reproduced                                               |
| P09 Graph document          | NOT_STARTED    | PENDING             | Ordered-map mutation costs source-derived                                                                                 |
| P10 Canvas                  | NOT_STARTED    | PENDING             | Projection diagnostic and crossing-edge reproduction; mounted performance open                                            |
| P11 Workspace UX            | NOT_STARTED    | PENDING             | Collection/keyboard gaps source-reviewed; native usability open                                                           |
| P12 Boundaries/gates        | NOT_STARTED    | PENDING             | App policy leak, omitted graph tests, oversized files, unsafe conversions; recurring incremental native-link artifact gap |
| P13 Formula execution       | NOT_STARTED    | PENDING             | Historical profiles retained; production parallelism not mandated                                                         |
| P14 Production time         | NOT_STARTED    | PENDING             | Clock helper unwired, no target-frame play, starvation/latency policy gaps                                                |
| P15 Assets                  | NOT_STARTED    | PENDING             | Audio path/cache identity exists; project asset boundary unimplemented                                                    |
| P16 Product scale           | NOT_STARTED    | PENDING             | Historical full-product gates remain open; 20k/50k/mounted canvas required                                                |
| P17 Platforms/artifacts     | NOT_STARTED    | PENDING             | Windows history retained; exact current app/native/platform checks required                                               |
| P18 Named devices           | NOT_STARTED    | PENDING             | Exact authorized ASIO4ALL/VB-Audio route must be verified first                                                           |

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

P00: 2026-10-18, Windows x86-64, branch `codex/builtin-mapping`, HEAD
`2e1f392551ee6d4b9a5859bc3c62945e18811fe4`. Initial status showed the documentation
consolidation; it was committed externally during reconciliation and a subsequent status was
clean. No owning runtime sources had local edits. `rustc --version`, `cargo --version`,
`node --version`, `npm --version`, and `python --version` match the canonical manifest:
Rust/Cargo 1.97.0, Node 26.5.0, npm 11.17.0, Python 3.14.6. Historical checks remain
historical; P01's early-return paths remain present in `engine/src/engine/apply.rs`.
No devices or remote mutations used. Next: P01.

P01 effort: **XHigh requested; actual effort unchanged** (actual setting unobservable).
No supported client effort control is exposed; continue locally without nested sessions.
Trigger: coupled applied-prefix history, dirty/cache finalization, and atomic intent rollback.

P01 batch 1 (uncommitted on the P00 SHA): Golden engine request errors now converge on common
history/reference/context/UI finalization. Structural insertion/move/replacement/metadata
effects remain explicit when lifecycle completion fails; placement is validated before insert
or detach. Existing prepared-subtree rollback now restores project dirty nodes, and its owning
rejection tests assert that invariant. New regressions live in
`crates/golden_core/engine/src/engine/tests/edit_failures.rs`.

- Reproduced baseline: `cargo test --locked -p golden_engine edit_failures --lib`, one pass and
  one failure (`undo_len=0`, expected 1 after the valid prefix).
- Intermediate focused check: same command, four passed (prefix/history, redo, missing-reference
  warnings/dirty nodes, and context resolution). This predates the expanded lifecycle/placement patch.
- Expanded patch: `cargo check --locked -p golden_engine --tests` passed;
  `cargo test --locked -p golden_engine --lib` passed **461**, with 3 manual benchmarks ignored.
  This includes all seven new failure regressions and the strengthened duplicate rollback tests.
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed.
  Root and Golden Core `cargo fmt --all` passed. No devices opened or remote mutations performed.
- P01 remains open: compound creation currently applies initializers after live insertion;
  inspect and repair that validation/commit boundary, then qualify compound rejection, active
  sessions, lifecycle failures, next tick/save, and redo. Do not mark P01 complete from this batch.

Focused low-level design and verification complete; return to High requested for routine work.
Actual effort remains unchanged/unobservable. Compound intent rollback is a separate XHigh risk
subtask when resumed. No meaningful repair attempts failed; the baseline failure and corrected
test-construction type mismatch are not escalation-counter events.

The
[maintained source-size inventory](../reference/source-size-inventory.md) is a reference,
not another execution ledger.

P01 batch 2 (same uncommitted P00 tree, Windows x86-64): catalog creation and initializer/history
repair. **XHigh requested; actual effort unchanged/unobservable**; no supported control is exposed.
The previous goal turn made progress; no blocker audit applies.

- Ownership: `engine/persistence/duplicate.rs` prepares catalog subtrees; `ui_sync/creation.rs`
  uses the boundary for ordinary/dashboard creation. Available scalar values are validated
  detached. Context-dependent fields and live reference constraints are validated in the
  provisional live subtree before ready, publication, or history commit; missing fields reject.
- Invariants: explicit values win over init defaults; bound handles and reference caches are
  current before ready. Declared conditional children settle between initializer values.
  Declaration lookup supports hierarchy keys and full declaration-id fallback. Binding batches
  prevalidate all targets/values, and binding/wrap edits now retain undo history.
- Readability: extracted reconstruction/ready ownership to `engine/persistence/lifecycle.rs`;
  parent `mod.rs` is 927 lines. The read-only inventory now scans 1327 files, 45 over 1000;
  no other oversized rows changed. This maintains P12 without claiming P12 completion.
- Diagnostics retained: optimized unit runs returned 464 passed / 3 failed / 3 ignored,
  then 467 / 3 / 3 twice. They exposed missing reference-cache refresh on semantic no-ops,
  conditional/contextual declaration handling, and early return before declaration-id fallback.
  Each received a source-based correction. No actual effort change is claimed.
- Focused verification: `cargo test-fast --locked -p golden_engine intent_atomicity --lib -- --quiet`
  passed 9; `cargo test-fast --locked -p golden_engine dashboard --lib -- --quiet` passed 20.
  The wrap-only test also passed. These filtered suites overlap; do not sum them.
- Final optimized verification: `cargo test --locked -p golden_engine -- --quiet` passed
  **470 unit + 1 integration + 7 doctests**, with 3 manual benchmarks ignored.
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed on the patch.
  A subsequently added production rejection/projection/dirty/next-save regression passed 1
  under `cargo test-fast --locked -p golden_engine rejected_creation_preserves_production --lib -- --quiet`;
  it was not included in that optimized unit count. It verifies unchanged published nodes,
  history, clean document revision, and exact saved authored JSON after rejection.
- Remaining P01 acceptance: inject lifecycle edits to already-existing metadata/structure,
  followed by failure, and verify rollback plus production publication for those lifecycle writes. The current subtree
  checkpoint captures parameters, pending/publication state, and dirty nodes; it does not by
  itself prove rollback of every existing-node lifecycle mutation. Keep P01 IN_PROGRESS until
  that audit and any resulting repair pass. No devices, commits, pushes, or remote actions used.

Focused creation design/verification complete; return to High requested for routine documentation.
Actual effort remains unchanged/unobservable. Existing-node lifecycle rollback is a separate
coupled-state risk subtask for the next batch.

Final hygiene: root and Golden Core `cargo fmt --all`, maintained Markdown Prettier check,
and `git diff --check` pass. All validation handles from this batch completed successfully;
there is no outstanding build to restart on resume.

P01 batch 3 resumed at the same HEAD and prior P01 diff. Previous turn: progress.
Current focus is lifecycle writes to existing nodes followed by failure. **XHigh requested;
actual effort unchanged/unobservable** because no supported control is exposed. Establish a
durable reproducer before extending the transaction boundary; preserve the previous passes.

Batch 3 evidence in the same uncommitted tree:

- Reproduced with `cargo test-fast --locked -p golden_engine rejected_creation_rolls_back_existing
--lib -- --quiet`: rejected ready callback left an existing root rename and new sibling.
- Added `engine/provisional.rs`: one ordered journal owns structured original-tree edits and
  outer additions. Nested callbacks reserve order before application. Existing undo/redo state
  changes only on successful commit. `persistence/transaction.rs` owns checkpoint/abort handling;
  duplication, imports, and archive restoration share the boundary. External ingress and prior
  pending requests are excluded; rollback errors preserve both typed causes.
- The first journal regression run passed 10/11: rollback was repaired, while successful redo
  repeated ready-authored writes. `HistoryRestore` now distinguishes resource/subscription
  activation from replaying authored edits. Focused `intent_atomicity` then passed 12/12,
  including existing-node removal/move/replacement and nested callback ordering.
- Retained listener-index state on abort and strengthened production projection/clean-revision/
  next-save coverage with a lifecycle failure after existing-node edits. Current optimized
  `cargo test --locked -p golden_engine -- --quiet` passed 474 unit tests, one integration test,
  and seven doctests, with three unit tests ignored. Strict
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed. Both ran from
  `crates/golden_core` on Windows x86-64 with default features; no device/native qualification.
- Split `node/core/behavior.rs` into focused creation-context, inbox-dispatch, and via-target
  modules while preserving public reexports; the behavior contract is now 935 lines.
- P01 remains IN_PROGRESS: structured edits now have evidence, but opaque mutation/script
  callbacks and compound intents outside prepared-subtree scope still need acceptance coverage
  and any resulting repair. This batch does not claim complete generic rollback or release readiness.

P01 batch 4 (same uncommitted tree): blueprint registry ownership during history/rollback.
The focused coupled-invariant subtask continues the recorded XHigh request; actual effort remains
unchanged/unobservable.

- Extended the existing lifecycle rollback regression with blueprint metadata and reverse-index
  assertions. Its pre-fix run failed for `FailExistingRemove`: the original node was restored,
  but `blueprint_instance_meta` returned `None` instead of its original version/decl lookup.
- Add/remove history now carries detached blueprint metadata for every instance root in the
  subtree; replacement history swaps the live and detached instance metadata with node payloads.
  This also removes stale registry entries during undo of insertion and restores them on redo.
- Added repeated add/remove/replace undo/redo index checks. The first compile caught a test-only
  temporary slice lifetime; fixed with a stable array binding.
  `cargo test-fast --locked -p golden_engine intent_atomicity --lib -- --quiet` passed 13/13;
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed.
  `cargo test-fast --locked -p golden_engine -- --quiet` passed on the final formatted source:
  475 unit tests, one integration test, seven doctests; three unit tests ignored. Commands ran
  from `crates/golden_core` on Windows x86-64 with default features using the repository's
  optimized dev profile. Batch 3's test-profile pass is separate evidence.
- Remaining P01: opaque authored callback edits and atomic compound UI boundaries outside the
  prepared-subtree checkpoint, followed by the complete task acceptance audit. No devices or
  remote actions were used.

Next P01 source entrypoints: `engine/apply_param.rs` invokes mutable node callbacks/script hooks
without capturing a history step; distinguish runtime-only changes from persisted authored state
and reproduce the latter before changing the contract. `ui_sync.rs::apply_implicit_ui_edit_session`
groups history but does not reverse a failed operation. Extending that boundary must give one
outer owner to the checkpoint and coordinate nested prepared-subtree scopes (the current journal
correctly rejects nesting); do not add a second independent rollback system.

Focused journal/blueprint verification is complete. **Return to High requested; actual effort
unchanged/unobservable** for routine documentation/formatting. The next atomicity design remains
a separate coupled-invariant escalation if needed. Both root and Golden Core `cargo fmt --all`
completed; updated docs passed Prettier and the diff passed whitespace checks.

P01 batch 5 resumed at the same HEAD and accumulated P01 diff. Previous goal turn: progress.
No unrelated changes or live Cargo jobs found. The next focused subtask joins the outer compound
UI intent to the existing journal and preserves enclosing-session history on rejection.
**XHigh requested; actual effort unchanged/unobservable** because no supported control is exposed.

Batch 5 evidence:

- `cargo test-fast --locked -p golden_engine rejected_remove_stabilization --lib -- --quiet`
  reproduced a rejected UI removal that still deleted the original child and renamed the root
  from its callback. This is distinct from prepared-subtree creation rollback.
- Moved the shared publication checkpoint to `engine/transaction.rs`. Implicit compound UI
  operations now own one journal checkpoint even inside an existing edit session; nested
  prepared subtree insertion joins that journal. Persistence retains only its typed error adapter.
  Removed the old cleanup loop that ignored candidate-removal errors; inverse failures preserve
  both typed causes. Prior queued edits/inbox events remain outside the scope and retain order.
- The new removal regression checks original UUID/graph/history/dirty/publication and the next
  tick, plus enclosing-session undo or existing redo. Added prior-work isolation checks for both
  success and rejection. `cargo check --locked -p golden_engine` passed.
  `cargo test-fast --locked -p golden_engine -- --quiet` passed 477 unit tests, one integration
  test, and seven doctests; three unit tests ignored. Strict
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed. These commands
  ran from `crates/golden_core` in the existing Windows/default-feature environment. The public
  error variant's workspace consumers passed the separate root
  `cargo check --locked --workspace`. A transient Cargo lock belonged to an IDE workspace
  check; it was left intact.
- P01 remains in progress: opaque authored callbacks and remaining direct compound UI operations
  still need coverage. The single existing transaction boundary must own subsequent repairs.

Opaque callback audit note: do not use a sparse project-codec round trip as an in-place memento.
The node macro's `generated_project_encode_data` omits default-valued fields and its decoder
returns early for null data. Restoring a previously default-valued field after a mutable callback
therefore needs a complete authored-state capture or an explicitly reset-aware contract. Begin
that repair with a default-to-nondefault mutation followed by rejection; do not add a snapshot
that silently retains the changed value. Runtime-only fields remain outside authored history.

Batch 5 focused rollback/session verification is complete. **Return to High requested; actual
effort unchanged/unobservable** for routine integration and documentation. The remaining callback
and direct-intent invariant repairs require their own focused escalation; P01 remains IN_PROGRESS.

P01 batch 6 resumed the same HEAD and accumulated P01 diff; previous goal turn was progress.
No unrelated edits or live Cargo jobs found. Focus: complete authored snapshots for mutable
Rust/script callback edits, including a default-valued field, nested queued edits, rollback,
and undo/redo. **XHigh requested; actual effort unchanged/unobservable**; no supported effort
control is exposed. The sparse-codec limitation above remains a required regression.

Batch 6 implementation/evidence (same uncommitted Windows/default-feature tree):

- Added complete authored-data capture/restore hooks to the public Node contract and enum/via
  forwarding. The shared node-field generator now emits sparse project data and complete edit
  data from one declaration list. Extracted that generator to `support/macros/src/node_data.rs`;
  the oversized macro entrypoint shrinks. Automatic `impl_node` now exposes the same declared
  persisted data contract. Parameter capture reuses its typed record; script restore retains
  runtime resources and requests a deferred reload only for a data change.
- `engine/apply_opaque.rs` records Rust mutation, script property, and script method self-state
  before nested requests. Metadata and user role are included; transient runtime state is not.
  Identity/topology/cache bindings stay engine-owned. Publication refreshes parameter caches,
  value/constraint events, node projections, and the shared reference/context finalization.
- Unified `provisional.rs` into `edit_journal.rs` with explicit AppliedPrefix/Atomic modes.
  Ordinary history-enabled drains and nested lifecycle edits share one ordered collector;
  ordinary errors retain the applied prefix, while atomic errors reverse it. Prefix mode avoids
  the atomic scope's whole-tree membership capture. Session boundaries flush accepted steps
  before changing ownership; callbacks cannot replace their enclosing session.
- The initial probe fixture omitted `persist`, so that failure was not valid authored-state
  evidence; corrected it and an integer-width fixture error. A subsequent full run passed
  481 tests but caught another test assumption: queued callbacks are not synchronous ordinary
  drains. The corrected regression exercises actual recursive ready/stabilization application.
  The next full engine run passed 484 unit tests, one integration test, seven doctests, with
  three ignored; strict engine Clippy passed. This includes Rust/property/method defaults,
  atomic rejection, rejected callback prefix undo, nested lifecycle ordering, complete parameter
  settings/cache restoration, composed state/runtime preservation, and binding ownership.
- Final automatic-node coverage and generator hygiene/reload refinements compile with
  `cargo check --locked -p golden_engine --tests`.
  `cargo test-fast --locked -p golden_engine -p golden_core_macros -- --quiet` passed 485 engine
  unit tests, the engine public integration test, the macro generated-consumer integration
  test, and seven doctests; three unit tests ignored. Strict
  `cargo clippy --locked -p golden_engine -p golden_core_macros --all-targets -- -D warnings`
  passed. Root `cargo check --locked --workspace` passed, including transport, desktop,
  Alchemist, state-machine, and Chataigne app consumers of the Node contract.
  These new changes do not alter wire DTOs or project schema. No device or remote actions used.
- P01 remains IN_PROGRESS: direct UI intent atomicity and the full task acceptance audit are
  still required. Do not count this batch as completion of P01 or the foundation goal.

Batch 6 focused callback/journal verification is complete. **Return to High requested; actual
effort unchanged/unobservable** for routine integration and documentation. Later direct-intent
rollback repairs remain a separate coupled-invariant subtask.

P01 batch 7 resumed at the same HEAD and accumulated P01 diff; previous goal turn was progress.
No unrelated edits or live Cargo jobs found. Focus: direct UI edit admission and stabilization,
control-state/child-tree history ownership, production rejection/next-save invariants, and the
final P01 acceptance audit. **XHigh requested; actual effort unchanged/unobservable** because
no supported effort control is exposed.

Batch 7 evidence and implementation (same Windows/default-feature tree):

- Added `engine/tests/direct_intents.rs`. Its baseline command
  `cargo test-fast --locked -p golden_engine direct_intent --lib -- --quiet` failed both tests:
  parameter rejection retained a changed value/root label; control-mode redo generated a new
  expression-child UUID. The tests retain redo/enclosing-session coverage and next-tick checks.
- Direct value/constraint/control/metadata/move/curve/event edits and script configuration now
  enter the shared atomic boundary. Added the internal Rust `Edit::SetParamControlState` so
  control edits use normal ordered history; wire DTOs and generated TypeScript are unchanged.
  Control completion errors retain their scalar effect. History restores recorded control state
  while child trees are restored by their own history payloads. Smart text uses the same edit.
- Extracted control editing/materialization into `engine/controls/edit.rs`. Removed obsolete
  ad hoc control/constraint history helpers and the UI-only edit-drain loop.
- Event and effective-enabled callbacks now capture owned authored self-state in the active
  journal, rather than relying only on their queued edits. Captured self-state finalizes
  references/warnings/contexts once at journal flush. Checkpoints retain the exact existing
  context registry/cache, including aliases, on rejection.
- A full run passed 486 tests but failed two checks: the move fixture did not subscribe to its
  new ancestor route, and the production no-unrelated-snapshot guard failed. The latter also
  failed in isolation against the compiled test binary: prior startup notifications had been
  retained for the next tick. UI admission now settles prior work outside the new transaction.
  Stabilization moved to the engine transaction module, avoiding an engine-to-UI dependency. The move probe
  now uses an explicit subtree subscription; the macro correctly rejected a manual generated
  interest-depth override during fixture compilation, which was removed.
- Extended the production rejection/next-save test with an event callback that directly mutates
  a persisted field. Added repeated exact-tree control undo/redo and context-alias/cache checks.
  Current compiler checks pass; the latest full engine suite and strict engine/context Clippy
  are running. No device/native qualification, commit, push, or remote action was performed.

- The next full run passed 485 unit tests but exposed three established lower-level duplicate
  contracts: root observer notifications must remain deferred and context-free catalog copying
  must not snapshot unrelated nodes. Kept those contracts: generic checkpoint commit does not
  choose notification timing. UI admission now wraps duplication through notification delivery;
  lower-level Rust subtree methods remain deferred. A small internal error adapter lets engine
  and persistence callers share the same journal while retaining their typed original/rollback
  causes. Removed the newly obsolete fixed-error transaction wrapper rather than retaining an alias.
  Final checks on the revision after removing that wrapper:
  `cargo test-fast --locked -p golden_engine -- --quiet` passed 488 unit tests, one public
  integration test, and seven doctests; three unit tests ignored. Strict
  `cargo clippy --locked -p golden_engine -p golden_context --all-targets -- -D warnings`
  passed. Root `cargo check --locked --workspace` passed for current app/host/Alchemist/state
  consumers. Root and Golden Core `cargo fmt --all` completed; final documentation formatting
  and whitespace checks are recorded with the handoff. These are local Windows/default-feature
  checks, not device, mounted UI, scale, native runtime, or cross-platform qualification.

P01 final-audit follow-up: `history.rs::undo`/`redo` still pop their transaction before fallible
replay and use `?` before returning it to a history stack. Inspect payload ownership and partial
replay rejection before claiming history failure coherence. Also audit authored callbacks during
normal history activation/notification delivery; ready filtering alone does not prove every
replay path. These are source-confirmed next entrypoints, not new verified failure claims.

Batch 7 focused direct-intent verification is complete. **Return to High requested; actual
effort unchanged/unobservable** for documentation/integration. History replay/error ownership
requires the next focused coupled-invariant escalation. P01 remains IN_PROGRESS.

P01 batch 8 resumed at the same HEAD and accumulated P01 diff, without live jobs or unrelated
changes. **XHigh requested; actual effort unchanged/unobservable**: history payload ownership,
partial replay rollback, and authored notification replay form coupled state invariants. No
supported effort control is exposed. Reproduce the failures before repairing this engine boundary.

Batch 8 baseline: `cargo test-fast --locked -p golden_engine history_failures -- --nocapture`
reproduced four failures (0 passed): failed undo changed a surviving parameter from its runtime
value, missing-parent restoration left an orphan, ready replay repeated a persisted self write,
and replay notifications repeated authored callback/queued metadata changes. The parameter test
stopped at its undo assertion; redo was source-confirmed and awaits repaired-tree verification.

The repair adds per-step live-state/payload-transition recovery under the shared checkpoint,
retains rejected transactions on their source stack, validates restoration before taking payloads,
and preserves authored state during history callbacks while retaining runtime activation. Replay
notifications settle before the guard ends; pre-existing pending work stays outside the scope.
Disjoint history batching is retained. Replacement now has one stable identity, matching its sole
producer; obsolete dual-identity replay and its unused child-reparenting helper were removed.
`cargo check --locked -p golden_engine --tests` passed, and strict engine/all-target Clippy passed
after boxing the large scalar backup variant. Full package regressions are running; no pass for
the final tree is claimed yet.

Intermediate full-package run: **493 passed, one failed, three ignored**. All six new history
regressions passed, including both undo and redo failure directions, missing-parent retry,
failed batched-ready retry, runtime moves/replacements, and authored activation/notification
preservation. The remaining assertion measured all snapshots as a proxy for ready cost. Replay
now synchronously delivers its notifications, requiring one shared dispatch snapshot; the test
now separately asserts unchanged ready/lifecycle builds and exactly one dispatch build for the
whole batch. No per-node snapshot rebuilding was introduced. Root `cargo check --locked
--workspace` passed on this intermediate tree.

Final lifecycle audit also protects authored self-state during teardown when a journal retains
the payload for undo or rollback; runtime-only destruction still avoids serialization. A seventh
history regression checks successful removal/undo and atomic removal rejection through the next
tick/save. Both ready and destruction hooks continue to update their runtime-only counter.
The final tree passes `cargo check --locked -p golden_engine --tests` and strict all-target engine
Clippy. Final complete engine tests were then run as recorded below.

Final result (same HEAD plus accumulated P01 diff, Windows/default features):

- `cargo test-fast --locked -p golden_engine -- --quiet` passed **495 unit tests**, **one public
  integration test**, and **seven doctests**, with three manual benchmarks ignored. All seven
  new `engine/tests/history_failures.rs` regressions passed.
- `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` and
  `cargo check --locked -p golden_engine --tests` passed on the final source tree.
- Root `cargo check --locked --workspace` passed for the engine, transport, desktop, public
  facade, Alchemist, processor, state machine, and Chataigne app consumers.
- Root and Golden Core `cargo fmt --all`, documentation Prettier checks, and whitespace checks
  passed. The refreshed source inventory contains **1341 files / 44 oversized**; none newly
  exceed the size limit. No devices opened, commits, pushes, or remote publication.

P01 acceptance audit: batches 1–8 cover invalid-first/middle/last applied prefixes, lifecycle
completion failure, atomic compound/direct rejection inside and outside a session, redo
preservation, reference/context/cache/blueprint restoration, next edit/tick/save, complete authored
callback state, control-child identity, and failed undo/redo ownership/retry. **P01 is
VERIFIED_LOCAL.** This is Windows/default-feature local evidence; mounted UI, product scale,
native runtime, devices, and other platforms remain in their later owning tasks.

Focused history design/fix verification is complete. **Return to High requested; actual effort
unchanged/unobservable** for documentation/integration. P02's coupled schedule/generation and
actor concurrency work requires its own focused escalation. Source entrypoints are
`engine/runtime/mod.rs::mark_schedule_dirty`, `runtime_center.rs::run_tick` (old compiled order
plus appended new nodes), compilation admission/completion, and application actor calls with
`.expect("production control actor disconnected")`. P02 has only been read; no P02 source changes
or test pass is claimed. Next dependency-ready task: **P02**.

P02 batch 1 resumed at the same HEAD and accumulated verified P01 diff. Previous goal turn:
**progress** (P01 completed locally). No unrelated changes or live Cargo/Rust jobs found.
**XHigh requested; actual effort unchanged/unobservable** for schedule/generation concurrency;
no supported client control is exposed. Owning boundaries are Golden engine schedule resolution
and production runtime compilation/ordering. First reproduce dependency-only reorder and newly
scheduled prerequisite failures; actor overload/error propagation remains a required P02 subtask.

P02 batch 1 evidence (same uncommitted Windows/default-feature tree):

- Baseline `cargo test-fast --locked -p golden_engine runtime_center::tests::scheduling --
--nocapture` reproduced both production failures: dependency-only reorder and activation of a
  previously passive prerequisite each executed `[a, b]` instead of `[b, a]` (0 passed, 2 failed).
- Resolved schedules now carry an engine-owned revision. Compilation snapshots, installed
  generations, and the actual scheduled-update callback share it. A mismatched compiled order
  selects the engine's resolved canonical order; the old append-new-nodes path is removed.
  Admission compares the resolved revision, replacing retained-UI-event heuristics and the
  UI intent classification list. Idle ticks add only constant-time revision checks.
- Compiler completion checks project epoch, compiler-request sequence, pending invalidation,
  and captured schedule revision before installation. Project candidates resolve before capture
  and reject a changed schedule before cutover. Invalid schedule rates are validated before
  replacing the last usable schedule/revision.
- Compilation admission/completion/install ownership moved to focused
  `runtime_center/compilation.rs`. The runtime coordinator shrinks from 954 to 808 lines;
  the application facade shrinks to 919. Seven production regressions cover pre-applied and
  tick-preamble reevaluation, automatic compilation, newly inserted prerequisites, held stale
  completions, overlapping project epochs, failed resolution/recovery, and changed candidates.
- `cargo check --locked -p golden_engine --tests` and strict
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed.

Final P02 batch 1 validation:

- `cargo test-fast --locked -p golden_engine -- --quiet` passed **501 unit tests**, **one public
  integration test**, and **seven doctests**; three manual benchmarks remain ignored. All seven
  new production scheduling tests passed. The obsolete retained-event heuristic test was removed
  with that implementation; dependency-only automatic recompilation is verified through effects.
- Root `cargo check --locked --workspace` passed for current engine/host/facade/Alchemist/
  processor/state-machine/app consumers. Strict engine/all-target Clippy and test compilation
  passed before the complete package run. Root and Golden Core `cargo fmt --all`, five maintained
  docs' Prettier checks, and whitespace checks passed. Source inventory: **1344 files / 44
  oversized**; none newly oversized. No devices, commits, pushes, or remote publication.
- Evidence is Windows/default-feature local validation; no mounted UI, device, scale, native
  runtime, or cross-platform qualification is inferred. P01 regressions remain included and pass.

Focused schedule/generation verification is complete. **Return to High requested; actual effort
unchanged/unobservable**. P02 remains **IN_PROGRESS**: `golden_runtime` already exposes
`ControlError` / `ControlErrorKind::{Overloaded, Disconnected}` in
`runtime/control/src/control.rs`; application `call_engine`, transaction submission, and tick
paths still use `.expect("production control actor disconnected")`. Propagate these through
application/transport contracts and audit cancellation/completion semantics, then test saturated
admission, coherent errors, absence of false acknowledgement, and continued progress. This next
concurrency/error-boundary subtask requires its own focused escalation.

P02 batch 2 resumed at the same HEAD and verified schedule/P01 diff. Previous goal turn:
**progress**. No unrelated changes or live Cargo/Rust jobs found. **XHigh requested; actual
effort unchanged/unobservable** for actor admission/completion concurrency and application /
transport error contracts; no supported effort control is exposed. Reproduce the public
overload panic, then preserve typed rejection/cancellation without a false acknowledgement.

P02 batch 2 implementation (same uncommitted Windows/default-feature tree):

- Baseline `cargo test-fast --locked -p golden_engine application::tests::control_failures --
--nocapture` reproduced the public overload panic (0 passed, 1 failed). A full bounded actor
  queue made `apply_ui_transaction` panic on `ControlErrorKind::Overloaded`.
- Fallible construction and all actor-backed facade operations now return `ApplicationError`,
  retaining concrete actor, runtime, and graph-rejection causes. Graph rejection still owns its
  authoritative acknowledgement/history; admission failure has no fabricated acknowledgement.
  Save capture/publication, project replacement/status, queries, scripting, tick, lifecycle, and
  value APIs propagate actor errors. Consumers and existing regressions use the explicit results.
- `application/transactions.rs` owns admitted batches and completion. The public pending-batch
  API allows real WebSocket `Accepted` publication after admission, followed by an authoritative
  result or typed completion error. Runtime reply guards distinguish queued cancellation from a
  lost started-operation result; actor exit clears pending-depth metrics even after a panic.
- Protocol **0.7.0** adds shared HTTP/WebSocket actor categories. Updated the Rust DTO/codegen
  root, generated authoritative bindings through `golden_codegen ui-protocol` into a target
  artifact, and applied their diff with the standard editing tool. Golden UI exposes typed
  `ControlRequestError` values and retains HTTP status/body data. No hand-maintained TS enum.
- Split the transport coordinator (2395 lines) into focused HTTP, WebSocket, hub, delivery, and
  error modules; each is below 1000 lines. The facade is 746 lines; transaction admission is 226. New regressions exercise public saturation/continued progress, cancellation versus actor
  loss, shutdown cancellation, and real HTTP/WebSocket overload without false acceptance.
- Strict `cargo clippy --locked -p golden_engine -p golden_runtime -p golden_transport_server
--all-targets -- -D warnings`, owning test compilation, root workspace compilation, and
  `npm run check` passed. Root `npm test` passed 140 tests before the final HTTP error-helper
  extraction/test. Complete backend suites and final UI checks are pending; no final pass yet.

P02 remains IN_PROGRESS. After this batch's tests pass, audit saturated session cleanup/recovery
and save metadata publication after file commit. Current disconnect cleanup reports rejection
but does not yet retain a failed cleanup for retry. Do not equate typed propagation alone with
completion of the P02 recovery/continued-progress contract.

Final P02 batch 2 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p golden_engine -p golden_runtime -p golden_transport_server --
--quiet` passed: **503 engine unit + 1 public integration + 7 doctests**, **10 runtime unit +
  2 work-selection integration**, and **44 transport unit + 1 public headless integration**.
  Manual ignored cases: three engine, three runtime scale/selection, and one transport. Public
  overload, queued cancellation versus started-operation loss, shutdown cancellation, real HTTP
  and WebSocket saturation, no false acceptance, and next-request recovery all passed.
- Strict owning all-target Clippy passed. Root `cargo check --locked --workspace --tests` passed,
  including updated app/system consumer tests; the ordinary workspace check also passed.
- Final root `npm test` passed **141** (audio 27, Golden UI 22, app 92). `npm run check` reported
  zero Svelte errors/warnings and passed audio generated-binding verification. `npm run build`
  passed with the existing large-client-chunk warning (1419.99 kB minified / 430.26 kB gzip).
  Canonical protocol generation comparison found **zero binding mismatches**. Golden UI changed
  files use the established app UI Prettier configuration; docs use their normal configuration.
- Source inventory: **1357 files / 43 oversized**; the transport coordinator is now 607 lines,
  with focused modules at 395/369/450/683 lines. No newly oversized files. Formatting/whitespace
  checks are finished with the handoff. Loopback qualification only; no external services,
  hardware, commits, pushes, or remote publication.

Focused actor error-contract verification is complete. **Return to High requested; actual
effort unchanged/unobservable**. Keep P02 **IN_PROGRESS** for the explicitly retained recovery
audit. Source entrypoints: `ui_server/hub.rs` disconnect/session replacement cleanup currently
logs a rejected cancellation and drops it; `application/project_persistence.rs` calls actor
metadata publication after the file commit and returns the typed error without commit-stage
context. Reproduce/repair those cases and prove later operations remain coherent before P02
closeout. Their concurrency/persistence invariants require a new focused escalation.

P02 batch 3 resumed at the same HEAD and verified P01/P02 diff. Previous turn: **progress**.
No unrelated changes or live Cargo/Rust jobs found. **XHigh requested; actual effort unchanged /
unobservable** for connection/session lifetime fencing and committed-save publication failure.
A client-name retry can cancel a replacement connection's new session; design lifetime ownership
at the application/runtime boundary before repairing transport cleanup. No supported effort
control is exposed. Save-stage errors must distinguish a committed file from failed publication.

P02 batch 3 implementation (same HEAD plus accumulated uncommitted repairs):

- Baseline `cargo test-fast --locked --no-fail-fast -p golden_engine -p
golden_transport_server control_failures -- --nocapture` reproduced two defects: a closed
  connection stranded its edit session after rejected cleanup admission, and a committed save
  returned only the overload diagnostic without its durable identity. The prior overload test's
  mutex-poison failure was a cascade from the first regression, not a third independent defect.
- `UiClientLease` belongs to the production runtime boundary. Socket completion and hub eviction
  share a close signal; socket cleanup closes it before waiting for hub admission. The actor
  fences queued requests and finalizes accepted edits into undo history on its next turn. One
  active owner is retained; no retry queue accumulates. Identity binding shares the physical
  signal, anonymous edits finalize before named work, and a second different handshake closes
  the connection. Reused browser/edit IDs cannot transfer ownership to a replacement connection.
  Project cutover clears the previous owner. Failed ticks still publish their accepted prefix
  and released history state. The redundant hub client-name index was removed; peer replacement
  scans the live table, capped at 16 clients, so eviction cannot leave stale index entries.
- `ApplicationError::SavePublication` retains the committed path, request ID, generation,
  document revision, and typed actor cause after successful file commit. Rejected metadata
  admission leaves dirty/path state intact; the next save can commit and publish normally.
- Regressions cover named/anonymous disconnect history, closed queued work, replacement IDs,
  project cutover, failing ticks, handshake identity, simultaneous actor/hub saturation, late
  cleanup, durable save identity, and later-save recovery. The transport saturation test retains
  the hub entry while the socket close signal releases the actor session.
- Strict owning Clippy passed before the final physical-lifetime refinement. Final backend
  suites, strict Clippy, and root workspace/test compilation are running; no final pass claimed.
  Inventory: **1359 source files / 43 oversized**. No devices or external endpoints opened.

Final P02 batch 3 validation (same HEAD, Windows/default-feature local tree):

- `cargo test-fast --locked --no-fail-fast -p golden_engine -p golden_runtime -p
golden_transport_server -- --quiet` passed: **509 engine unit + 1 public integration + 7
  doctests**, **10 runtime unit + 2 work-selection integration**, and **45 transport unit +
  1 public headless integration**. The three engine, three runtime, and one transport manual
  qualifications remain ignored. All six new batch-3 engine tests and the saturated disconnect
  transport test passed, including physical closure before hub cleanup.
- Strict `cargo clippy --locked -p golden_engine -p golden_runtime -p
golden_transport_server --all-targets -- -D warnings` passed. Root `cargo check --locked
--workspace --tests` passed for the app, systems, and default-host consumers. Root and Golden
  Core `cargo fmt --all`, maintained-doc Prettier checks, and whitespace checks passed.
- P02 acceptance audit: production scheduling regressions exercise identical-membership
  dependency changes, new prerequisites during compilation, stale completions, replacement
  epochs, failed resolution, deterministic effects, and continued progress. Actor/facade/
  transport regressions establish typed overload/cancellation/disconnection without false
  acknowledgements, disconnect ownership/history recovery, and post-commit save recovery.
  **P02 VERIFIED_LOCAL**. UI/protocol were unchanged in batch 3; batch-2 frontend and codegen
  evidence remains scoped to those unchanged files. No broader platform/device/scale claim.
- Final source inventory remains **1359 files / 43 oversized**; no newly oversized source.
  No commits, pushes, remote publication, devices, or external endpoints were used.

Focused concurrency/persistence recovery verification is complete. **Return to High requested;
actual effort unchanged/unobservable**. Next dependency-ready task: **P03**, not yet implemented.
Source reconnaissance confirms `apps/chataigne/src/module/modules/protocol/http/transport.rs`
blocks stop on command send/join, admits an unweighted bounded request queue, collects uncapped
responses, and uses an unbounded event channel; its module drains until empty. UDP transport
also uses an unbounded outbound channel and drain loops. Start with a controlled slow-HTTP/full-
queue regression and use public `golden_io` admission/retirement APIs. Cancellation and resource
handoff require a fresh focused XHigh request before design/repair; routine integration stays High.

P03 batch 1 resumed at the same HEAD and accumulated verified P01/P02 diff. Previous goal turn:
**progress**. No unrelated edits or live Cargo/Rust jobs found. App HTTP owns request policy;
public `golden_io` owns reusable admission/lifetime mechanics. **XHigh requested; actual effort
unchanged/unobservable** for saturated HTTP cancellation, reply backpressure, and worker cleanup
ownership. No supported effort control is exposed. Reproduce actor-blocking shutdown first.

P03 batch 1 implementation (validation in progress):

- The old HTTP regression blocked for over 60 seconds behind a stalled request and a full queue;
  its owned test process was terminated. Its fixture originally joined the blocked stopper before
  asserting the timeout; this was corrected so future failures report promptly. This is a measured
  blocked baseline, not a completed failing-assertion run.
- Public `golden_io::BoundedWorker` composes existing weighted pending channels, cancellation,
  capacity waits, and a pre-reserved worker lifetime. The worker owns cleanup and its slot until
  exit; handle drop neither joins nor creates another cleanup thread. Full replies retain the
  result and wait, oversized replies return ownership, and worker death wakes event-driven owners.
  First owning test run: **20 passed**, including five new concurrency/lifetime regressions.
- HTTP moved client construction/destruction, blocking I/O, JSON/script-response preparation,
  retries, and reply backpressure to that worker. Bounds: 16 active/retiring worker lifetimes;
  256 requests / 8 MiB retained request weight; four replies / 32 MiB event weight; 1 MiB response
  bodies including unknown-length streams; bounded JSON expansion. Module turns retain/drain at
  most four responses, borrow parsed values, and use pending/death signals plus capped recovery
  backoff. Native in-flight calls retain their slot until the configured request timeout.
- Owning app/test compilation passed after two extracted callback methods were given the correct
  parent-module visibility. Current focused app tests, final I/O tests, and owning strict Clippy
  are running. No final batch pass or complete P03 acceptance claim yet.
- P03 still requires the complete module/channel audit, UDP/service bounds, engine external edits,
  lifecycle expansion, generator elapsed time, and further cancellation/reconfiguration semantics.
  Requested shutdown cancels queued HTTP work; remote effects already issued cannot be undone.
  Repeated reconfiguration, accepted-command cancellation reporting, and per-response graph
  materialization service budgets need their own evidence.
  Only localhost TCP fixtures were opened; no external endpoints or devices were used.

Final P03 batch 1 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p Chataigne2 http -- --quiet` passed **17 tests**, including six
  new HTTP transport/expansion regressions. Stalled/full shutdown returns promptly, retry
  cancellation releases capacity, declared and unknown-length oversized bodies reject before
  the next request succeeds, slow consumers preserve reply order/multiplicity, request bytes
  and complexity reject at admission, and repeated large JSON paths hit the retained limit.
- `cargo test-fast --locked -p golden_io -- --quiet` passed **20 tests**, including five new
  worker tests for weighted/item bounds, backpressure, cancelled queues/retries/idle waits,
  retained native-cleanup capacity, oversized replies, worker failure, and later progress.
- Root `cargo check --locked --workspace --tests` passed on the final source. Owning strict
  Clippy and formatter/doc/whitespace checks are recorded at final closeout below. Source
  inventory: **1364 files / 43 oversized**; no newly oversized source. HTTP coordinator is
  868 lines; worker/weights/callbacks and new tests are focused files below 1000 lines.
- Success and failure callback arguments are prepared on the HTTP worker and moved into script
  delivery. A compact failure reply retains the admitted request identity; diagnostics are
  capped before publication. The 32 MiB event-weight bound includes prepared failure arguments
  for the largest admitted request, avoiding an error-reporting capacity dead end.
- P03 remains **IN_PROGRESS**. This is local loopback/admission evidence, not full module,
  sustained load, native/device, or platform qualification. No UI/protocol changes, commits,
  pushes, or remote publication. The source/runtime constraints still open above remain tasks.

P03 batch 1 closeout: strict `cargo clippy --locked -p Chataigne2 -p golden_io --all-targets
-- -D warnings` passed on the final source. Both root and Golden Core `cargo fmt --all`, explicit
Rustfmt for the app's generated-registry-owned HTTP modules, maintained-doc Prettier checks, and
`git -c core.safecrlf=false diff --check` passed. **Return to High requested; actual effort
unchanged/unobservable**. The next focused concurrency/accepted-cancellation or lifecycle-bound
repair needs a fresh escalation; no further P-task is marked complete.

P03 batch 2 resumed at the same HEAD and verified accumulated P01/P02/P03 diff. Previous goal
turn: **progress**. No unrelated changes or live Cargo/Rust jobs found. App-owned HTTP owns
response materialization and completion policy; Golden owns generic snapshot lookup and worker
admission/lifetime contracts. **XHigh requested; actual effort unchanged/unobservable** for
resumable graph application, FIFO callbacks, and accepted request ownership across configuration
changes. No supported effort control is exposed. Preserve large arrays with continuations rather
than narrowing the accepted input to fit a service turn.

P03 batch 2 implementation (same HEAD plus accumulated diff):

- Baseline `cargo test-fast --locked -p Chataigne2
http_response_materialization_is_bounded_per_turn_and_preserves_large_lists -- --nocapture`
  failed its per-turn node-growth assertion (0 passed / 1 failed). A single 1024-element array
  materialized its entire graph in one module update.
- HTTP now retains one bounded FIFO of success/failure events and services its front response
  with a cursor. Each turn has 256 path/value/sibling units and a 64 KiB copy allowance; one
  larger already-bounded scalar may use a full turn. Large arrays remain accepted and resume
  without replaying prior units. Tail pruning scans bounded sibling prefixes with stable next
  markers; external marker removal restarts a bounded scan. Known missing paths still use the
  common detached-subtree planner. Survivor identities and unindexed children are retained.
- Callbacks wait for a subsequent snapshot after queued value work and cannot overtake prior
  events. Conflicting leaf/ancestor paths yield between turns, preserving repeated triggers and
  accepted mutation order. Array elements keep their no-description policy; scalar provenance
  remains available. Optional description in the common batch contract makes that policy explicit;
  the OSC consumer keeps its previous descriptions with `Some(...)`.
- HTTP worker preparation bounds value paths to 256 bytes per segment / 4096 total bytes and
  caps provenance to 4096 UTF-8-safe bytes. Service byte weights include scalar provenance;
  actor URL diagnostics use a bounded prefix. These prevent metadata/path copies from defeating
  the per-turn budget. The repeated-path expansion regression still reaches the 8 MiB bound
  using paths within those limits; a large scalar regression proves progress beyond 64 KiB.
- Generic `ProcessTreeSnapshot::find_child` now indexes exact aliases for parents wider than
  16 siblings, preserving the first sibling across all aliases. This removes repeated whole-
  sibling scans from response continuation. A public snapshot regression covers 16/17/20k
  siblings and collisions; no Chataigne policy was added to Golden.
- First service run passed 21 HTTP tests. Final app checks include the metadata/path refinements
  and external marker deletion. The full engine suite has passed 510 unit tests and its public
  integration; doctests/final diagnostics and final app validation are still being collected.
- Subsequent path/provenance/large-scalar refinements passed **23 HTTP tests**. A final source
  audit distinguished trigger-address creation from the first incoming pulse: the cursor now
  retains that pulse for a real Append edit after target creation. Existing trigger addresses
  still batch appended pulses within a turn. The regression now requires exactly two parameter
  events for two inputs; its final run, strict Clippy, and workspace checks are running.
- Inventory refreshed: **1366 source files / 43 oversized**, no new exception. The response
  service is a focused 300-line module; the HTTP coordinator remains 866 lines. Small formatter
  changes to the touched OSC consumer's tests keep that module consistently formatted.

P03 remains IN_PROGRESS. Reconfiguration still stops/drops the old HTTP handle, so accepted queued
requests can be cancelled without individual completion. Repair this ownership boundary next;
do not report reconfiguration as qualified by the service tests. Also audit queued graph-edit
failure versus cursor progress when checking engine/lifecycle failure paths. Remaining module/
channel, UDP, engine external-edit/lifecycle, generator, and broader qualification tasks remain.

Final P03 batch 2 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p Chataigne2 http -- --quiet` passed **23 tests** on the final
  source. New service tests prove bounded 1024-value creation/shrinkage, exact two-pulse delivery
  across target creation, input-order replacements, FIFO response/failure completion, survivor
  identity, external trim-marker deletion, preserved unindexed children, and progress for a
  100k-character scalar. Worker tests also cover path/provenance limits. The prior one-tick bulk
  expectation was replaced by bounded-turn completion; large arrays were not removed or truncated.
- `cargo test-fast --locked -p golden_engine -- --quiet` passed **510 unit tests, one public
  integration test, and seven doctests**, with three existing manual qualifications ignored.
  This includes alias lookup/collisions through 20k siblings and all P01/P02 regressions.
- Strict `cargo clippy --locked -p Chataigne2 -p golden_engine --all-targets -- -D warnings`
  passed. Root `cargo check --locked --workspace --tests` passed, including the adjusted OSC
  consumer. Formatter/doc/whitespace closeout is recorded below. Inventory: **1366 source files /
  43 oversized**; no new source-size exception. No UI/protocol changes or native/platform claim.
- Only local graph and localhost TCP fixtures were used. No external endpoints, devices,
  commits, pushes, or remote publication. P03 remains **IN_PROGRESS** for the explicit remaining
  ownership, module, engine/lifecycle, generator, and qualification work above.

Focused response-service/snapshot verification is complete. **Return to High requested; actual
effort unchanged/unobservable**. Next resume at HTTP `refresh_transport` / `stop_transport` and
the public `BoundedWorker` admission/lifetime boundary. Preserve accepted old-generation request
ownership across reconfiguration, including full retirement capacity and slow endpoints, before
claiming that contract. That focused concurrency repair needs a new escalation. Do not restart
the verified response-service or P01/P02 work.

P03 batch 2 closeout: root and Golden Core `cargo fmt --all`, explicit app HTTP/common/OSC
Rustfmt checks, five maintained-doc Prettier checks, and whitespace checks passed. No live
validation jobs remain. The overall foundation/readiness goal is still active.

P03 batch 3 resumed at the same HEAD and verified accumulated P01/P02/P03 diff. Previous goal
turn: **progress**. No unrelated edits or live Cargo/Rust jobs found. App HTTP owns generation
handoff and endpoint ordering; public `golden_io` owns graceful admission closure and worker
lifetime mechanics. **XHigh requested; actual effort unchanged/unobservable** for accepted
request ownership, completion order, and bounded reconfiguration/recovery. No supported effort
control is exposed. Do not drop the old lifetime merely because a new configuration was authored.

P03 batch 3 implementation (same HEAD plus accumulated diff):

- Baseline `cargo test-fast --locked -p Chataigne2
http_reconfiguration_preserves_accepted_requests_execution_and_completion_order -- --nocapture`
  failed (0 passed / 1 failed): the replacement executed ahead of older admitted requests.
- Public `BoundedWorker::finish_admission` closes a producer without cancelling its queue;
  receiver-side admission closure similarly preserves accepted entries for failure completion.
  Two new generic tests passed, including full command/reply mailboxes and a still-live producer.
- App `http/lifecycle.rs` owns at most three generations per module, with the existing shared
  16-slot native lifetime pool. Handoff retains old handles, closes their admission, and gates
  new request execution behind their completion. Both execution and callbacks stay ordered;
  the aggregate drain still admits at most four events to the module per turn. Identical settings
  reuse the live generation; failed/full reconfiguration rejects new requests while preserving
  old ownership and coalescing the latest authored setting through the existing backoff.
- Native cleanup no longer blocks request/result progress. An atomic completion marker is
  observed before draining the final queue, so racing the last publication cannot discard it.
  The native worker retains its slot through client destruction even after the consumer releases
  the drained handle. Gated cancellation uses a paired lock/notification to avoid a lost wake.
- Request execution catches worker panics, closes admission, and returns explicit failures for
  the started request and each queued request. Attempt counts retain possible endpoint effects;
  queued requests report that they were not sent. Startup failure uses the same closed-admission
  drain. Invalid settings and disabling preserve owners; re-enable resumes bounded completion
  delivery. Only destruction cancels the removed destination's lifetime.
- Final owning suites passed **31 HTTP tests** and **22 I/O tests**. Eight new HTTP cases cover
  real loopback execution/completion ordering, a full 256-command mailbox with failed new
  lifetime admission, repeated changes at the owner limit, identical-setting reuse, worker
  failure/recovery, gated stop, blocked native retirement, disabled backlog/re-enable, and
  invalid settings. Strict owning all-target Clippy passed; root workspace/test check is running.
- Inventory: **1370 source files / 43 oversized**, none newly oversized. HTTP coordinator is
  852 lines; lifecycle ownership and fence are 156/82 lines. No protocol/UI declarations changed.
  Only loopback and injected worker fixtures were exercised; no device, native artifact, or
  cross-platform qualification is inferred. No commits, pushes, or remote publication.

Final P03 batch 3 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p Chataigne2 http -- --quiet`: **31 passed**; no HTTP failures.
  `cargo test-fast --locked -p golden_io -- --quiet`: **22 passed**; no I/O failures. Loopback
  reproduction now preserves both old requests, delays replacement dispatch, and delivers all
  three callbacks in order. The full-mailbox case retains all **257** admitted commands while
  new native capacity is rejected, then recovers. Disabled output remains bounded to four replies
  plus one in flight and all 17 accepted requests complete after re-enable.
- Strict `cargo clippy --locked -p Chataigne2 -p golden_io --all-targets -- -D warnings` and
  root `cargo check --locked --workspace --tests` passed on the final source. Existing engine,
  frontend, protocol-generator, and platform evidence retains its prior scope; those suites were
  not rerun merely for unchanged source. No full product/device qualification is implied.
- Source-size inventory: **1370 files / 43 oversized**. All new/touched owning modules remain
  below 1000 lines. Formatting/docs/whitespace closeout is recorded below. No validation jobs
  remain. P03 is still **IN_PROGRESS** for the outstanding engine/lifecycle/module/generator
  requirements; P04-P18 retain their ledger states.

Focused HTTP handoff/worker-lifetime verification is complete. **Return to High requested;
actual effort unchanged/unobservable**. Resume at P03's queued graph-edit failure/cursor audit,
then remaining module/channel/configuration bounds. Preserve the verified HTTP service and
ownership repairs. The next coupled-state or lifecycle/concurrency repair requires its own
focused escalation; no further P-task or overall goal is marked complete.

P03 batch 3 closeout: root and Golden Core `cargo fmt --all`, explicit app HTTP Rustfmt,
maintained-doc Prettier, and whitespace checks passed. The overall foundation/readiness goal
remains active; no source checks or remote actions are pending for this verified batch.

P03 batch 4 resumed at the same HEAD and accumulated diff. Previous goal turn: **progress**.
Scheduled callbacks accumulate edits before stabilization; HTTP advances its response cursor
before those edits apply. A public callback/queue regression now checks an earlier rejected
request followed by HTTP materialization. Golden owns edit completion/disposition; HTTP owns
the response outcome and must never replay endpoint effects or already-applied pulses.
**XHigh requested; actual effort unchanged/unobservable** for coupled deferred completion,
accepted-prefix errors, structural deferral, and atomic rollback. No supported control is exposed.
The baseline regression is compiling; no failure or repair result is claimed yet.

P03 batch 4 baseline: `cargo test-fast --locked -p Chataigne2
http_rejected_graph_edits_report_failure_without_replaying_the_response -- --nocapture`
**failed (0 passed / 1 failed)**. The public callback/queue fixture discarded the HTTP subtree
behind an earlier invalid parameter edit, then incorrectly emitted `responseReceived`.
This models the shared scheduled-callback queue, without clearing or altering a produced edit.

The repair adds generic `EditQueue::completion` receipts. Tokens follow ingress coercion,
duplicate removal, structural deferral, and journal ownership. Superseded/discarded requests fail;
atomic success waits for commit and rollback fails it. Observed warning no-ops remain queued so
an earlier rejection cannot give them premature success. The focused ingress module replaces
the old inlined validator without adding a second coercion policy. HTTP now waits for each bounded
turn's receipt and reports graph-application failure without retrying the endpoint or pulses.
Owning test compilation passed; full engine/HTTP suites are running. No final pass is claimed yet.

The full engine suite now passes **517 unit tests, one public integration test, and seven
doctests**, with three existing manual qualifications ignored. Seven new tests cover applied
prefix/discard, atomic commit/rollback and prior queued work, observed no-ops, coalescing and
duplicate removal, invalid ingress/teardown, real scheduled-event structural deferral, and an
insertion whose ready callback fails after mutation. Existing P01/P02 regressions remain green.
The HTTP suite also includes a separate callback-delivery ownership regression: the module
currently pops a response before its queued custom event is applied. That baseline is compiling;
the graph fix alone must not be reported as covering callback publication.

P03 batch 4 callback baseline: `cargo test-fast --locked -p Chataigne2 http -- --quiet`
passed **33 tests / failed one**. Graph rejection and the partial-trigger regression passed;
`http_retains_callback_ownership_when_a_shared_queue_discards_delivery` reproduced permanent
loss of the queued callback (zero observed instead of one). This is a second reproduced defect,
not a failed fix attempt for the first graph-cursor issue.

HTTP now retains a shared `CustomEvent` until a receipt for that event alone confirms publication.
Discard/atomic rollback resubmits only the unpublished event, preserving FIFO ownership without
replaying graph values or endpoint effects. Later edit/subscriber errors cannot duplicate an
already-published callback. The app's existing callback-event builder is exposed within the app,
and its JSON wrapper moves prepared arguments instead of serializing a second body copy.
Snapshot requirements now follow the response stage: receipt waits, terminal acknowledgement,
failure delivery, and callback-only turns use no tree snapshot. A new direct callback regression
checks pending receipts and repeated calls without a snapshot; the delivery regression also checks
an error after publication. Final HTTP and app-helper checks are pending.

Final P03 batch 4 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p golden_engine -- --quiet` passed **517 unit tests, one public
  integration test, and seven doctests**; three existing manual qualifications remain ignored.
  Its seven new completion tests and all prior P01/P02 regressions passed. Final formatting did
  not change that engine source.
- `cargo test-fast --locked -p Chataigne2 http -- --quiet` passed **35 tests** on the final app
  source. Four new cases prove explicit graph rejection, no replay of an applied trigger prefix,
  callback ownership across both earlier/later failures, and repeated receipt/delivery calls
  without a tree snapshot. Existing bounded-service, handoff, recovery, and loopback tests passed.
  `cargo test-fast --locked -p Chataigne2 script_api -- --quiet` passed the one owning app-helper
  test; callback payload shape is also exercised throughout the HTTP suite.
- Strict `cargo clippy --locked -p Chataigne2 -p golden_engine --all-targets -- -D warnings`
  and root `cargo check --locked --workspace --tests` passed on the final source. Root and Golden
  Core `cargo fmt --all`, explicit app HTTP/script-helper Rustfmt, maintained-doc Prettier, and
  whitespace checks passed. No protocol/UI declarations changed; unchanged I/O/frontend/platform
  evidence keeps its previous scope rather than inheriting a new qualification claim.
- Inventory: **1373 source files / 43 oversized**, no new size exception. Engine coordination
  is 796 lines; ingress is 147 and completion tokens are 76. HTTP coordination is 864 lines;
  response service/callback ownership are 344/152 lines. Receipts add work proportional to the
  bounded observed edit set and retain no graph or edit payload copies.
- Only local graph, injected worker, and loopback fixtures were exercised. No external service,
  device, native artifact, cross-platform, or mounted-product qualification is inferred. No
  commits, pushes, or remote publication. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

Focused deferred-completion/HTTP ownership verification is complete. **Return to High requested;
actual effort unchanged/unobservable**. Next audit P03's unbounded engine external edit sender/
drain in `engine/mod.rs`, its typed ingress in `engine/ingress.rs`, and lifecycle materialization
budgets before continuing the remaining app channels/UDP/generator requirements. Preserve the
verified completion tokens through any ingress API change. A new concurrency or coupled-lifecycle
repair needs its own focused escalation. No validation jobs remain for this batch; the overall
foundation/readiness goal remains active.

P03 batch 5 resumed at the same HEAD and accumulated diff. Previous goal turn: **progress**.
No live Cargo/Rust jobs were found. Source audit confirms that `Sender<Edit>` is unbounded and
drains until empty at multiple points per tick; its only production consumer is the passive
Sound Card wake path. Arbitrary boxed nodes and mutable closures have no enforceable retained-
payload weight at that boundary. Module workers already own their actual result mailboxes.
The replacement will carry reserved/coalesced node wake signals only; payload processing and
graph edits stay in node callbacks and public actor operations. It must not strand one-shot
worker completion on capacity rejection or callback absorption failure. Sound Card's bounded
delayed-wake helper also needs reservation/coalescing before its immediate-wake fallback can be
removed. **XHigh requested; actual effort unchanged/unobservable** for queue, generation/lifetime,
and callback-delivery invariants. No supported effort control is exposed. Lifecycle expansion
and the remaining module/channel/UDP/generator audits remain explicit P03 work.

P03 batch 5 baseline (Windows/default features, same HEAD plus accumulated diff):
`cargo test-fast --locked -p golden_engine --test public_graph_editing
external_edit_drain_has_per_turn_service_bound -- --nocapture` **failed (0 passed / 1 failed)**.
One drain consumed all 1024 accepted events rather than leaving a bounded suffix owned. Its
public regression now exercises the replacement API: 512 reserved worker wakes require two
256-entry ticks, followed by an idle tick. Arbitrary cross-thread `Edit` injection has been
removed, not retained behind guessed weights or a compatibility sender. The P02 failed-tick
publication fixture now queues its authored edits through the existing application actor call;
its history/publication assertions are unchanged.

Implementation now provides per-node reservations, coalesced queued signals, a separate bounded
worker-service turn before controls, and a receipt-held acknowledgement stage for deferred edits.
Signals arriving during callback/application waits are retained. Absorption failure or a discarded
edit receipt re-arms readiness; payload/cursor recovery stays in the module. Removed/replaced
sources close and restored IDs receive fresh sources; engine ingress closes before node drop.
Sound Card consumes the generic wake path, releases its own source on destruction, reuses an
unchanged source, and uses one process-wide reserved/coalesced retry timer without immediate-poll
fallback. Configuration/startup-dependent snapshots preserve its former inbox requirements.
Core/app test compilation passed after ordinary migration wiring fixes. Full engine, Sound Card,
and strict Clippy checks are running; no final test result is claimed yet. Sound Card fixtures
use their existing test-only null backend, and new timer fixtures use only Folder nodes. Native
stop/join fallbacks, event-drain budgets, and replace-node lifecycle ownership remain separate
P03 audit work; this wake repair does not qualify the whole audio module.

The first full engine run passed **519 tests / failed one**, with three existing ignores. The
new fixture incorrectly used the UI's latest-scalar projection to count three input occurrences;
source review confirms `ui_coalescable_param_value_event` deliberately retains one scalar value
and every trigger edge. The corrected fixture checks the final scalar state and three real trigger
edges independently. No engine event-coalescing policy was changed to satisfy that assertion.

A subsequent focused design audit separated worker service from timed update ownership:
`Node::on_worker_wake` and `worker_wake_requires_tree_snapshot` now have the standard enum
forwarding. Worker service has zero scheduled delta and leaves replay/coalesced update baselines
untouched. A continuously re-armed passive node and an explicitly woken periodic node run beside
scheduled work with separate budgets; the fixture asserts that the scheduled node still receives
exactly its one-millisecond delta. This prevents a worker notification from advancing generator
time before the scheduled pass. Sound Card now implements the dedicated hook. Its first **36-test**
run passed before that hook separation; final engine/app suites and Clippy are running on the
revised source. App/core test compilation passed. No physical device was opened by these fixtures.

P03 batch 5 focused validation passed **520 engine unit tests, three public integration tests,
and seven doctests**, with three existing manual qualifications ignored. The final public race
fixture uses timeout-bounded channel rendezvous with a real producer during callback execution;
its second wake survives acknowledgement and the following tick becomes idle. Core tests also
prove 512 signals drain in two 256-entry ticks, live/retiring capacity rejects and recovers,
callback-absorption and deferred-application failures re-arm correctly, three trigger edges survive
coalesced notifications, and worker service cannot advance a periodic node's scheduled clock.
The app's **37 Sound Card tests** passed, including three timer cases and a real null-backend
startup/destroy/undo case proving that the passive module resumes through its reserved source and
re-registers after restoration. No physical endpoint was selected. Focused all-target Clippy passed;
final checks are pending after the fixture refinements below.

An additional public-consumer probe using `golden_engine::node` attribute macros failed to compile
with E0433 (emitted `golden_core` crate path). The notification race fixture now implements the
public `Node` trait directly, without adding a crate-alias shim. Direct lower-crate attribute-macro
support remains an unqualified P12 build/consumer audit item; the app's canonical `golden_core`
facade usage compiles. Do not infer that the macro path was repaired by the passing trait fixture.

The HTTP preservation run passed **34 / failed one**: the disabled-backlog fixture received
Windows `WouldBlock` (10035) from its accepted stream before headers arrived. A controlled test
accepting from a nonblocking listener and delaying headers 20 ms reproduced it deterministically
(0 passed / 1 failed). The owning helper now explicitly sets blocking stream mode before its
existing three-second read timeout; production HTTP behavior is unchanged. Its full suite now
includes the delayed-header regression and is running. **Return to High requested; actual effort
unchanged/unobservable** for this routine fixture and final lint/workspace/doc closeout.

Final P03 batch 5 validation (same HEAD plus accumulated diff; Windows/default features,
2026-10-19):

- `cargo test-fast --locked -p golden_engine -- --quiet` passed **520 unit tests**, both public
  integrations then present, and **seven doctests**; three existing manual qualifications remain
  ignored. The final `cargo test-fast --locked -p golden_engine --test public_graph_editing --
--quiet` passed **all three public integrations**, including the timeout-bounded real-producer
  race added afterward. No production engine behavior changed after its full suite passed.
- `cargo test-fast --locked -p Chataigne2 sound_card -- --quiet` passed **37 tests**, including
  passive null-backend startup/destroy/undo and the three bounded timer cases. Test defaults select
  the null backend; no physical device or native-route qualification is inferred.
- Final `cargo test-fast --locked -p Chataigne2 http -- --quiet` passed **36 tests**. The new
  delayed-header case now passes, as does the 17-request disable/backlog/re-enable case that exposed
  the Windows stream-mode assumption. All previous HTTP admission, FIFO, trigger, handoff, failure,
  and callback-publication regressions remain green. Production HTTP code did not change here.
- Final strict `cargo clippy --locked -p golden_engine -p Chataigne2 --all-targets -- -D warnings`
  and root `cargo check --locked --workspace --tests` passed. Maintained docs and source formatting/
  whitespace closeout follows below. Unchanged UI/protocol/platform checks retain their previous
  scope; no new wire declarations or persisted schema were introduced.
- Inventory is **1378 source files / 42 oversized**, with no new exception. Standard formatting
  brings the touched Sound Card integration coordinator from 1006 to 975 lines. The new generic
  wake registry/service are 292/55 lines; engine coordination is 773 lines. New runtime and test
  modules all remain below 1000 lines, with conventional test locations.
- Engine ingress now owns only fixed-size, reserved/coalesced notifications. Module mailboxes
  still own payload bytes and occurrence recovery; a budget-limited drain must re-arm its source.
  Cold timer-registration rejection is explicit startup admission failure, not accepted worker
  work. Full module/native recovery, stop/join fallbacks, event budgets, and replacement lifecycle
  semantics still need their P03 audit; this batch qualifies only the stated wake/timer contracts.

P03 remains **IN_PROGRESS**. Resume at `engine/apply_tree/lifecycle.rs`'s unbounded materialization
loop and `apply_tree/replace.rs`'s payload lifecycle ownership, then the remaining module/channel,
UDP, and generator work. Preserve the verified P01/P02 and HTTP ownership repairs. The next
coupled lifecycle repair needs its own focused escalation; High remains the default, actual effort
unchanged/unobservable. No commits, pushes, remote service calls, physical device opening, or remote
publication were performed. All owned validation jobs are terminal; the overall foundation/
readiness goal remains active.

P03 batch 5 closeout: root and Golden Core `cargo fmt --all`, explicit HTTP/Sound Card Rustfmt
checks, five maintained-doc Prettier checks, and whitespace checks passed. All owned validation
processes have finished. No further source action is pending for this verified batch.

P03 batch 6 resumed at the same HEAD and accumulated diff (167 status paths). Previous goal
turn: **progress**. No live Cargo/Rust jobs or unrelated edits were found. The engine owns this
repair: lifecycle materialization has an unbounded nested drain, and replacement currently swaps
payloads without the outgoing destroy or incoming attach/init/ready pipeline. Bound callback-
generated expansion before changing handoff; preserve known detached-subtree batch scaling,
P01's committed-prefix/atomic contracts, history replay, and fresh worker-wake generations.
**XHigh requested; actual effort unchanged/unobservable** for these coupled lifecycle/history
invariants. No supported control is exposed. Other module/channel/UDP/generator work remains P03.

P03 batch 6 baseline: the new public lifecycle fixture reproduced both gaps: a ten-generation
init chain ignored a two-pass limit, and replacement emitted no destroy/init/ready callbacks.
The original baseline session was no longer observable after context rollover; the bounded
rerun failed both assertions and supplies the baseline evidence. The focused public tests now
pass five cases (including both undo/redo failure directions). Broader validation is pending.

Batch 6 implementation:

- Added engine-owned `LifecycleLimits` (64 passes, 65,536 generated requests/new nodes by default)
  and typed exhaustion errors, including the existing UI rejection boundary. Nested structural
  drains share the admitted scope. Known detached inputs are not capped; generated trees and
  user-item forests reserve work before coercion/insertion. Rejected suffixes fail receipts and
  cannot silently continue on the next tick. Ordinary prefixes remain undoable; atomic rollback
  keeps its existing inverse journal/publication rules. These limits do not preempt callback code.
- Replacement now destroys only the outgoing root payload, fences its waker/subscriptions, and
  runs incoming attached/init/ready (deferred ready for project load). Retained children stay live.
  History swaps destroy the outgoing runtime and re-ready the incoming payload without re-init;
  both atomic-journal and failed-history rollback track replacement roots for reactivation.
  Both rollback paths deduplicate restored IDs; multiple replacements of one root activate
  the final restored payload once, covered by a runtime-start counter regression.
- Public regressions cover pass/work admission, full generated forest reservation, explicit wide
  trees, suffix failure receipts, subsequent-tick stability, retained child IDs, stale wake handles,
  and repeated failed history activation with retry. Added unit checks for project-load expansion
  and atomic replacement rollback, plus a null-backend Sound Card replacement/undo smoke test.
  Those owning-unit/app checks are running; no final batch/P03 acceptance claim yet.

Batch 6 final-source engine verification passed: `cargo test-fast --locked -p golden_engine
-- --quiet` from Golden Core ran **522 unit tests**, **five public lifecycle integration tests**,
**three public graph-editing integration tests**, and **seven doctests**; three existing manual
benchmarks remained ignored. This includes the final repeated-root rollback activation counter.
Sound Card/HTTP preservation, strict Clippy, and workspace/test compilation are still pending.

Returning to **High requested; actual effort unchanged/unobservable** after the coupled engine
invariants passed. The app preservation and formatting/lint closeout remain routine validation.

Final P03 batch 6 validation (same HEAD plus accumulated diff; Windows/default features,
controlled loopback/null backend, no physical devices):

- Golden Core `cargo test-fast --locked -p golden_engine -- --quiet`: **522 unit tests,
  eight public integration tests, seven doctests passed**; three existing manual benchmarks
  ignored. The new checks include ordinary-prefix undo/redo, known wide inputs, generated
  tree/forest preflight, project-load augmentation, failed history activation/retry, retained
  children, stale worker generations, and repeated-root atomic rollback with one restart.
- Root `cargo test-fast --locked -p Chataigne2 sound_card -- --quiet`: **38 passed**,
  including repeated live null-runtime replacement/undo/redo, in addition to prior timer/wake
  tests. Root `cargo test-fast --locked -p Chataigne2 http -- --quiet`: **36 passed**;
  error logs are intentional injected admission/application/recovery failures.
- Root `cargo clippy --locked -p golden_engine -p Chataigne2 --all-targets -- -D warnings`
  and `cargo check --locked --workspace --tests` passed. All owned build jobs reached terminal
  results; long builds were observed through their original sessions without restarting.
- Root/Golden Core formatters and checks, explicit app Sound Card Rustfmt, maintained-doc
  Prettier, and `git diff --check` passed. Inventory: **1380 source files; 42 over 1000 lines**;
  the new budget module/public suite introduce no exception.
- No project schema or wire DTO changed. No GUI, physical device, commit, push, or publication.
  P03 remains **IN_PROGRESS** for the broader module/UDP/generator work; P04-P18 states stand.

Next confirmed source gap: `apps/chataigne/src/module/modules/protocol/stream/udp/transport.rs`
uses unbounded `mpsc::channel` commands and assigns unit weight to variable-size events through
the item-bounded `pending_channel`; retained packet bytes therefore lack an effective bound.
It drains both commands and received packets until empty, resolves/binds in `spawn` on its caller,
and joins in `stop`. Review its module
ownership/recovery and received-value processing before choosing the next bounded worker shape.

P03 batch 7 starts at the same HEAD and accumulated diff (169 status paths); batch 6's jobs are
terminal. Previous work is progress. UDP transport/module owns this next repair; generic
admission/cancellation/lifetime contracts stay in `golden_io`. **XHigh requested; actual effort
unchanged/unobservable** for endpoint generation, accepted-command ownership, and retirement
races. No supported effort control is exposed. Establish a controlled baseline before repair;
preserve command/script behavior, explicit overload outcomes, endpoint recovery, and loopback-only
qualification. No physical endpoints, commit/push, or publication are authorized by this work.

Batch 7 source audit (no runtime repair yet): UDP also runs at 120 Hz, parses received bytes on
the node, and enumerates interfaces on the caller. `StreamingModuleBase` caches parser settings
and owns a bounded incoming-message queue, so preserve that shared stream contract while moving
IO preprocessing to its worker. `golden_io::BoundedWorker` already provides weighted admission,
reserved worker lifetimes, cancellation independent of mailbox capacity, and graceful admission
closure. It currently exposes only blocking `WorkerContext::receive`; unsolicited UDP input
needs a bounded nonblocking/timed command service API before reusing it. Avoid copying the
HTTP-specific generation model blindly: UDP needs explicit socket configuration ordering,
fair receive/send turns, failure/reconnect backoff, and retained command outcomes across changes.

Batch 7 reconciliation confirmed the same HEAD/169-path starting diff and no live Cargo/Rust
process at resume. The first Cargo baseline filter matched zero tests because app module paths
are generated (`app::module_modules_protocol_stream_udp`); inspected the built binary's test
list and reran the exact owning prefix. Zero tests are not validation evidence.

Added `WorkerContext::drain_commands` for bounded nonblocking service beside unsolicited native
input. Golden Core `cargo test-fast --locked -p golden_io -- --quiet` passed **24 tests**,
including FIFO/budget behavior through graceful closure and explicit cancellation; worker-thread
assertions are acknowledged back to the parent so a detached panic cannot masquerade as success.
UDP has not consumed the new API yet; the native worker repair follows its controlled baseline.

Batch 7 controlled baseline: the exact UDP transport test prefix ran **two regressions, both
failed**. A gated native setup prevented the caller receiving its handle until release; an
impossible 65,536-byte UDP datagram was admitted instead of rejected. Gates were released and
baseline workers reached terminal cleanup. The first repair compile check (`cargo check
--locked -p Chataigne2 --tests`) passed. A subsequent test build caught an incorrect test-only
module path before execution; that fixture now uses the bound socket directly. No alias/private
crate import was introduced. The corrected owning test suite is running in session 9908.

Batch 7 implementation (validation pending):

- UDP consumes `WorkerContext::drain_commands` through `BoundedWorker`: 256 command items/1 MiB,
  64 event items/512 KiB, retained allocations charged by capacity, 16 command/receive operations
  per native turn, and four events per node drain. Payloads above IPv4 UDP's 65,507-byte maximum
  and host strings above 1024 bytes fail admission explicitly. Bind/DNS/socket setup and
  destination resolution live on the reserved worker; stop/cancellation never joins native work.
- An app-owned `UdpTransports` state machine owns active/retiring generations. Configuration
  changes and disable fence admission and preserve accepted sends/results; a new bind waits for
  old socket release. Only destruction abandons the destination. Async Ready controls connected
  state; failures close admission, resolve queued sends explicitly, and use 250 ms-to-5 s
  reconnect backoff. Native lifetimes share a 16-slot pool, including blocked cleanup.
- Regressions cover blocked setup/cancel, byte admission, reserved lifetime saturation, 32 ordered
  sends across graceful closure, failed-bind completion, same-port reconfiguration, reconnect
  deadlines, and continuous receive pressure alongside bounded owner turns and outgoing sends.
- P03 remains incomplete: UDP's existing 120 Hz node service, interface enumeration, parser work,
  shared parser line retention, structural queue weight, and graph-application cursor still need
  the next coupled repair. This native ownership batch does not qualify those existing paths.

Batch 7 final-source owning checks passed: Golden IO **24 tests**, app UDP **10 tests**,
including public script `sendText` FIFO delivery and disabled rejection. The standalone native
baseline's two failures are now green. The earlier nine-case pass predates the script test and
test cleanup refinement; the ten-case rerun supplies final-source app evidence. Strict
`golden_io`/app Clippy passed before that last fixture addition and is rerunning alongside
workspace/test compilation and HTTP/Sound Card preservation. Inventory refreshed to **1383
source files / 42 above 1000 lines**, with unchanged oversized rows.

Final P03 batch 7 validation (same HEAD plus accumulated diff; Windows/default features,
controlled loopback and null backend):

- Golden Core `cargo test-fast --locked -p golden_io -- --quiet`: **24 passed**. Root
  `cargo test-fast --locked -p Chataigne2 module_modules_protocol_stream_udp -- --quiet`:
  **10 passed**, including final fixture cleanup and script send/disabled-admission preservation.
  The initial zero-match filter is excluded from evidence; the inspected owning prefix was used.
- Root HTTP **36 passed** and Sound Card **38 passed**, preserving the previous bounded
  completion, retry/wake, lifecycle replacement, and timer contracts. HTTP error logs are
  intentional injected failures. No hardware or external endpoints were opened.
- Root `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` and
  `cargo check --locked --workspace --tests` passed on final source. All owned jobs are terminal.
  Root/Golden Core formatters, explicit UDP Rustfmt, maintained-doc Prettier, and diff checks
  are the final closeout checks. Source inventory is **1383 / 42 oversized**; no new exception.
- Local datagram order and explicit failure/admission are qualified; remote UDP delivery is not
  an acknowledgement contract. Native calls that cannot be interrupted retain their lifetime
  slot until return, as established by controlled blocked-setup/cancellation/saturation tests.
- No wire DTO, project schema, GUI, physical device, commit, push, or publication change.
  P03 stays **IN_PROGRESS** for the shared stream/node and wider module/generator audits;
  P04-P18 states remain unchanged.

Returning to **High requested; actual effort unchanged/unobservable** after native command,
generation handoff, cancellation, and recovery checks passed. The next streaming/parser/cursor
work needs its own focused effort audit; do not treat this return as completing P03.

Batch 7 closeout: root and Golden Core `cargo fmt --all`/checks, explicit UDP Rustfmt,
six maintained-document Prettier checks, and `git diff --check` passed. No owned jobs remain live.

Milestone delivery reconciliation: `main` is an ancestor of `codex/builtin-mapping` (87 commits
behind), including the local outliner-windowing commit ahead of `origin/main`; the requested
merge can fast-forward without discarding that work. The unfinished shared-stream baseline
tests and their formatting are excluded from the verified milestone. Its controlled run
executed 22 tests: 18 passed and four reproduced open retained-byte, parser-line, receipt,
and ordered-value defects. These failures are not evidence against the verified batch-7 tree
and must be repaired in P03 batch 8 after switching to `main`.

P03 batch 8 resumed on `main` at delivered `f02205af`, with only nine shared-stream WIP paths
restored and no live owned jobs. The controlled baseline ran 22 tests (18 passed, four failed),
reproducing retained string capacity, unbounded unterminated lines, premature queue acknowledgement,
and an ordered return to the snapshot value being omitted. Owning app-common received-value and
streaming layers must preserve HTTP's verified cursor/receipt behavior. **XHigh requested;
actual effort unchanged/unobservable** for coupled cursor/receipt/ordering invariants; no supported
effort control is exposed. UDP worker parsing/notifications remain a later P03 batch.

Batch 8 implementation on `main` (validation pending):

- App-common `received_values` now owns one HTTP/stream message model, dynamic allocation
  accounting, and the proven HTTP receipt-driven graph cursor. Stream queues admit complete
  FIFO batches under 65,536-message / 16 MiB / 64-path-level limits, including active batches
  waiting for engine receipts. Overflow rejects the whole new batch with backend diagnostics.
  The obsolete incoming-value parameter-ignore set was removed: transport invalidation already
  compares only its bound connection controls, which are distinct from incoming value nodes.
- Shared planning compares repeated existing values against its ordered prefix, fixing `0 → 1 → 0`
  within one snapshot. Existing compatible targets can share a cursor turn; structural conflicts
  and newly created trigger addresses retain their fresh-snapshot/pulse rules. Turns retain the
  256 work-unit / roughly 64 KiB copy targets, allowing one admitted larger scalar rather than
  stranding it after path lookup consumes work units. Graph failure/discard abandons the suffix
  without replay and allows later batches to recover.
- Line parsing retains at most 1 MiB with capped geometric growth and chunk/delimiter scans,
  rejecting each oversized frame once through its delimiter while preserving surrounding frames.
  Grammar changes reject partial old lines explicitly. Raw/JSON preparation expansion, node-side
  parsing/callback preparation, UDP interface discovery/periodic scheduling, wider module bounds,
  and generator accounting remain open P03 work.
- The first repair check found one missed HTTP expansion-weight helper after extraction; the
  owning shared payload method replaced it. The next app test command compiled but failed linking
  an additional non-test app binary with 97 unresolved MSVC symbols in unrelated generated
  modules; no tests ran. Scoped `--bin Chataigne2` test validation follows. This diagnostic does
  not qualify the new implementation. Inventory refreshed to **1386 source files / 42 oversized**.

Batch 8 owning and preservation checks passed on final source (Windows/default features):

- Root `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 common::streaming -- --nocapture`:
  **33 passed**, including all four failing baselines, capacity retained through receipts, whole-batch
  admission, ordered repeated values, a 256 KiB scalar, 1000 list values over bounded turns,
  all 65,507 raw datagram bytes without truncation, applied-trigger-prefix failure, dropped-context
  failure, parser boundary/recovery, and configuration fencing. Test execution took 0.15 s;
  this is functional evidence, not production latency qualification.
- The resulting final-source app unit-test executable ran owning prefixes directly: HTTP
  **36 passed**, concrete stream modules **31 passed**, Sound Card module/commands **37 passed**
  plus the exact null-backend product-evidence test **1 passed**, and generic OSC **18 passed**.
  The `received_value` discovery filter matched zero tests and supplies no validation evidence.
  HTTP injected failures and nonexistent serial-port recovery diagnostics are expected. A default
  TCP-server fixture logged a port-9002 collision while all 31 concrete stream checks passed;
  this run does not qualify exclusive ownership of that default port or production endpoint behavior.
- Root `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings` and
  `cargo check --locked --workspace --tests` passed. The separate native app binary's earlier
  unresolved-symbol link is still being checked with an app-package-only clean/rebuild; no source
  workaround or global dependency-cache deletion was introduced.

Batch 8 native build closeout: `cargo clean -p Chataigne2 --profile dev` followed by root
`cargo build --locked -p Chataigne2 --bin Chataigne2` passed in 2m15s. The clean affected only
the app package's accumulated build artifacts; shared dependency caches and source were retained.
The earlier unresolved-symbol failure did not recur after that rebuild. No production source
workaround was needed. All owned jobs are terminal. Root/Golden Core Cargo formatting, explicit
app-module Rustfmt, maintained-doc Prettier, and diff checks passed; inventory remains **1386 / 42**.

Returning to **High requested; actual effort unchanged/unobservable** after focused graph/receipt
repair and validation. P03 remains **IN_PROGRESS** for preparation admission, worker notifications,
the wider module/channel audit, and generators. P04-P18 states remain unchanged. Commit and push
this verified batch on `main` before beginning the next milestone.

P03 batch 9 resumed at `9d8500b49091b08a3d1c978bd91b41016ff09411`, clean and tracking
`origin/main`; the previous batch was committed and pushed. The development watcher exists,
but no owned implementation job was live at resume. App-common stream preparation is the
owning boundary: the queue now bounds retained messages after parsing, while raw/JSON expansion
still allocates before those checks. Add input, decoded-value, path, and retained-expansion
admission before migrating UDP parsing/callback preparation to its worker. Keep ordinary bounded
parsing work at High; no concurrency/worker-generation repair is included in this first subtask.
Four controlled raw/JSON admission regressions are building before the repair.

Batch 9 controlled baseline ran four tests and **all four failed**, proving missing raw-frame,
JSON-input, JSON-depth, and retained path-expansion admission. The first repair
`cargo check --locked -p Chataigne2 --tests` passed. Owning preservation tests are building.

Concurrent checkout changes appeared during validation in `apps/chataigne/build.rs`,
`golden_core/support/codegen` (a new build-cache module and tests), and
`docs/guides/development.md`. These are unrelated work and must remain outside this milestone.
Validation on this shared checkout includes those build-cache changes; the stream repair does
not depend on their new API. Source scans also found remaining unbounded channels or native joins
in TCP/WebSocket, MIDI, MQTT, OSC, Buttplug, Joy-Con, keyboard/mouse, Ultraleap, OS/App Control,
Sound Card, and signal/metronome workers. This is source evidence for the remaining P03 audit,
not a claim that every match is a defect: string/path `.join` matches were excluded and each
native ownership path still needs its owning behavioral check before repair.

Batch 9 implementation and owning evidence (Windows/default features; shared checkout):

- `common/streaming/admission.rs` owns `PreparedMessages`: immutable received values and cached
  retained allocation weight. Both the parser and node queue use the same message/byte/path limits;
  queue admission no longer walks every message. Spare vector/string capacity is charged, and
  consumers cannot mutate admitted data without constructing and validating a new batch.
- Raw input above 65,536 bytes rejects before expansion. JSON input above 1 MiB rejects before
  whitespace scanning/decode; structural work/depth is checked before conversion or cloning.
  Paths support 64 levels / 4096 bytes, retained expansion stops at 16 MiB, and provenance uses
  UTF-8-safe prefixes. List limits derive from the value-storage budget: **100,000 list values**
  remain valid. Raw/JSON failures publish no partial frame. Individual rejected line frames preserve
  surrounding valid frames and later turns recover; the existing 1 MiB partial-line limit remains.
- The temporary serde JSON DOM and conversion scratch are additional allocations bounded by
  input/work/value limits, not included in a claimed 16 MiB process-RSS ceiling. Parser work,
  callback preparation, UDP interface discovery, and periodic service still need worker ownership.
- Root `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 common::streaming -- --nocapture`:
  **44 passed**, including all four failing baselines, boundary/recovery, large-list preservation,
  maximum raw UDP data, retained capacity, ordering, and receipt-failure tests. Execution took
  0.16 s; this does not establish production latency. The final-source executable also passed
  concrete stream **31**, HTTP **36**, Sound Card/null **38**, and generic OSC **18** cases:
  **167 owning/preservation tests** total. As in batch 8, the default TCP fixture logged a
  port-9002 collision without failing its owning suite; exclusive default-port qualification
  remains outside this evidence. Strict lint/workspace/native-build closeout follows.

Preservation commands used the built
`target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with each of
`module_modules_protocol_stream`, `module_modules_protocol_http`, `sound_card`, and
`module_modules_protocol_osc_generic_osc_module` as its filter and `--quiet`.

The concurrent build-cache work was committed separately as `5b81c44e` while batch 9 was
underway; `main` now contains that parent and the owned stream diff. Preserve that commit.
Root strict app Clippy passed; root/Golden Core formatter checks and explicit app-module Rustfmt
checks passed. The shared checkout and eventual milestone tree now both contain **1391 source
files / 42 oversized**, including the two separately committed cache sources and three new stream
preparation sources. No new source-size exception. Workspace/test compilation is running next.

Root `cargo check --locked --workspace --tests` passed on the final stream implementation plus
the separately committed `5b81c44e` parent. Native `cargo build --locked -p Chataigne2 --bin
Chataigne2` is the remaining check before this milestone is committed and pushed. Source and
wire formats outside the app-owned stream boundary are unchanged by this batch.

Batch 9 closeout:

- Native build produced and linked `target/debug/deps/Chataigne2.exe` at 08:58:19 local time,
  after the last stream runtime source change at 08:51:09. SHA-256:
  `78bf9a0bddd8e0cf6800317f99ee02543ad09bc1dbabf0560d140c734d7efdc7`.
  Cargo then failed its final replacement of `target/debug/Chataigne2.exe` with Windows
  `Access is denied (os error 5)`. The user's running app, PID 31696 from 08:55:29, holds that
  executable open. **The Cargo build command did not pass**; native compilation/linking produced
  its artifact, while executable replacement is **UNAVAILABLE** until that process releases it.
  The process was left untouched. This does not qualify a fresh native launch.
- Owning/preservation **167 tests**, strict app Clippy, and workspace/test compilation passed.
  Root and Golden Core `cargo fmt --all -- --check`, explicit app Rustfmt, maintained-doc Prettier,
  and diff checks passed. No new protocol DTO, project schema, device qualification, or source-size
  exception. All owned jobs are terminal; the user's watcher/app remain outside this work.
- Keep P03 **IN_PROGRESS**. The next worker-ownership repair must include configuration fencing,
  prepared callback delivery, event/exit notifications, reconnect deadlines, interface discovery,
  and bounded disposal of large prepared batches. Opaque cached weights remove the actor's
  admission scan; they do not qualify large-batch destruction or move parsing off the actor.
  Preserve the current receipt/ordering/maximum-input regressions. No effort escalation was
  needed for this ordinary bounded parsing subtask; audit the next coupled worker design separately.
- Commit and push this verified preparation milestone on `main`, preserving `5b81c44e`, before
  beginning the next batch. P04-P18 states remain unchanged.

P03 batch 10 resumed on clean `main` at delivered
`b93f18ada220aa4adfa4b6fbdf90104eaab08a11`, matching `origin/main`. The previous turn was
progress: stream preparation admission was implemented, verified, committed, and pushed.
No owned jobs remain live; the user's watcher/app are not implementation jobs. The next change
belongs in generic Golden IO notification/deadline primitives and app-owned UDP stream/runtime
adapters. **XHigh requested; actual effort unchanged/unobservable** for worker/configuration,
retry/exit, prepared callback, and payload retirement invariants. No supported effort control
is exposed. Preserve the nine verified P03 batches and avoid joining or cancelling the user's app.

Batch 10 implementation and intermediate evidence:

- Generic `golden_io` now provides payload-free `WorkerNotifier`, notified pending channels and
  `BoundedWorker::spawn_notified`. Readiness is published after enqueue, outside the queue lock;
  final sender exit also notifies, including a worker panic. `DelayedNotifier` moves the existing
  shared Sound Card timer into Golden IO: one host timer thread, 1024 weak registrations,
  coalesced earliest deadlines, and explicit cancellation. Sound Card retains its node-generation
  adapter and now consumes that generic timer. No app type entered Golden IO.
- `PayloadPool` reserves both items and retained bytes before sharing data. One background
  disposer holds the final strong owner, performs destructors outside its state lock, and releases
  capacity only after destruction. Blocked cleanup retains its reservation; destructor panic does
  not strand the pool. Consumer release is a short ownership notification, never a join or payload
  destructor. App stream preparation uses one shared 256-payload / 256 MiB pool; immutable
  message batches and UDP prepared packets remain registered until their consumers release them.
  Domain values copied into the graph and callback payload copies retained by the engine have
  their own engine lifetimes; this pool is not a whole-process RSS or event-history budget.
- UDP is passive: no scheduled `update()` or compiled periodic kernel. Native reply/exit wakes
  and shared retry deadlines drive bounded service. Bind/DNS, input parsing, callback arguments,
  bounded logging prefixes, and interface discovery execute on the native worker. Interface
  discovery runs at most once per second there, suppresses unchanged results, and rejects more
  than 256 choices / 64 KiB without replacing existing choices. The native turn still services
  at most 16 commands and 16 receives; a node drains at most four events into one retained batch.
  Reply admission is now 64 items / 32 MiB so a maximum raw datagram plus prepared callbacks fits.
- Processing settings are copied at the worker input boundary. Grammar/processing generations
  discard old partial lines explicitly; logging-only changes do not change the grammar generation.
  Processing changes update the live socket's shared settings without rebinding. Already prepared
  results retain their interpretation through later settings changes, disable, and retirement.
  Only destruction abandons the destination; destroy/undo clears queued runtime input instead of
  resurrecting it in a fresh worker generation.
- Script callback receipt delivery moved from HTTP into app `script_api/callback_delivery.rs` and
  is reused by UDP. Each event retains its own publication receipt; rejection retries only an
  unpublished event. UDP emits prepared callbacks in FIFO order, then admits its prepared values
  to the existing bounded graph cursor. It never reparses received bytes or builds byte arrays
  in the node callback. Engine wake deliveries gate subsequent turns on their aggregate receipt.
- Golden IO passed **33 tests**, including reply/exit publication, panic/disconnect, coalesced
  deadlines, capacity recovery, background final drop, blocked cleanup, and destructor-panic
  recovery. The UDP baseline failed because its rule requested 120 Hz. The repaired owning suite
  passed **18 tests**, including passive live input, exact callbacks, processing generation fencing,
  same-socket updates, callback-publication retry, graceful sends, rebind/backoff, and destroy/undo
  stale-input rejection. Two additional maximum-input/passive-retry regressions are running.
  Strict root `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings`
  passed before those last two fixture additions; rerun its test targets at final closeout.

Final UDP source passed **20 tests**, including the maximum raw worker reply and passive retry
after a controlled occupied-port failure. The preservation run passed concrete streams **41**,
HTTP **36**, Sound Card/null **36**, and OSC **18**. Two Sound Card timer-only cases moved into
Golden IO coverage rather than disappearing; its engine-adapter timer test remains app-owned.
The shared stream run exposed **one failure of 44**: adding the disposer header after preparation
could exceed the 16 MiB cap at a tight boundary. Admission now reserves that header before
acceptance and charges it once throughout the payload lifetime; the owning rerun passed **44/44**.
Final-source preservation/lint/workspace/native closeout follows this correction.

Final-source owning evidence after the header correction: shared streams **44**, concrete
streams **41** (including all **20 UDP** cases), HTTP **36**, Sound Card/null **36**, and OSC **18**
passed using the rebuilt app test executable. Together with Golden IO **33**, this is **208
owning/preservation tests**; UDP is included in concrete streams and not double-counted.
Root strict IO/app Clippy and `cargo check --locked --workspace --tests` passed. Controlled
HTTP failures, native capacity pressure, occupied-port recovery, and the existing TCP fixture's
default-port collision emitted diagnostics without failing those owning checks.

Native closeout hit a link failure with **496 unresolved anonymous MSVC/LLVM symbols** in the
app artifact, while the app unit-test executable linked and passed. As with batch 8's earlier
failure, an app-package-only `cargo clean -p Chataigne2 --profile dev` and fresh native build
were used to distinguish stale artifact behavior from a source/link defect. No runtime
source workaround, global dependency-cache removal, or user-process interruption is involved.
The fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` then **passed in 1m55s**.
The recurring incremental/native artifact behavior remains P12 tooling qualification evidence;
the clean build passes, while the stale-artifact reproduction has no source-level diagnosis yet.

Batch 10 closeout: root and Golden Core `cargo fmt --all` and their `--check` runs passed;
maintained-doc Prettier and `git diff --check` passed. All owned test/check/build jobs are terminal.
The required formatter also normalized the app module coordinator and missing helper newlines;
those changes have no intentional behavioral effect. Native checks use Windows/default features; endpoint tests use
controlled loopback and null audio only. Physical devices, other platforms, and full-product load
remain unqualified.

Separate concurrent UI work appeared after validation in `packages/golden-ui/components/app/`
(`AppHeader.svelte`, `AppFooter.svelte`, and new `EngineStatus.svelte`). It is preserved unstaged
and excluded from this milestone. The successful native build and source-size count describe the
verified milestone before those edits, not the newer combined working tree.

Focused passive-input/configuration/lifetime verification is complete. **Return to High requested;
actual effort unchanged/unobservable** because no supported control is exposed. P03 remains
**IN_PROGRESS**. In particular, `udp/mod.rs::queue_send_request` still performs synchronous output
collection and per-output packet admission; partial acceptance can precede a combined error. Audit
that outbound path before extending the passive-worker pattern to remaining stream modules.
Engine-retained event/history payload copies have separate lifetimes and are not covered by the
new stream disposal pool's budget. P04-P18 retain their prior implementation states.

P03 batch 11 resumed on `main` at delivered `e59ad7d5430e2649926fe6ece4e3e6c20c4a9396`.
Previous goal turn: **progress**, with batch 10 verified, committed, and pushed. The three
separate header/footer UI paths remain untouched. App-owned UDP owns frozen destination plans
and native send policy. **XHigh requested; actual effort unchanged/unobservable** for atomic
fanout admission and native cursor/retirement ownership; no supported effort control is exposed.
First repair replaces packet-per-destination queueing with one weighted whole-command envelope
and a bounded native destination cursor. Actor-side output-tree collection remains a separate
read-path repair; this batch does not qualify that traversal or all of P03.

Batch 11 implementation and owning evidence:

- `udp/outbound.rs` owns an immutable-after-admission command: one moved datagram, ordered
  destinations, outgoing-log policy, and cached allocation weight. The 1 MiB reservation includes
  payload capacity, allocated destination slots, and hostname capacity. Payloads over 65,507 bytes,
  hosts over 1024 bytes, oversized plans, and empty plans reject before native admission. A failed
  destination append invalidates the builder, so its earlier valid prefix cannot be submitted.
- Script/custom-event sends construct the entire plan before one native mailbox admission.
  Queue pressure rejects that command as a whole. The old per-output payload clone/admission loop
  and its partly-accepted combined-error result are removed. Native delivery remains best effort:
  a destination failure reports its index/count and continues later destinations without replay.
- The native worker retains one command cursor outside the 256-item / 1 MiB mailbox, adding at
  most one further 1 MiB envelope. A turn sends at most 16 destinations across command boundaries
  and receives at most 16 packets. Graceful closure finishes the current cursor before later
  commands and socket release. Terminal receive failure closes admission and uses the same cursor
  to drain accepted sends. Destruction retains explicit cancellation semantics.
- Outgoing logs now describe successful native sends. Their first-1024-byte prefixes are built
  on the worker and published through the existing weighted, four-event owner drain; the node
  no longer formats the complete datagram once per output.
- Six new tests cover a 10,000-destination plan retaining the original maximum-datagram allocation,
  exact capacity accounting, poisoned plan rejection, full-mailbox whole-fanout rejection,
  40-destination FIFO retirement spanning native turns, and an IPv6-destination failure on an IPv4
  socket preserving prior/later send multiplicity and log outcomes. UDP passed **26 tests**;
  the full concrete-stream filter passed **47** (including UDP), and shared streams passed **44**.
- Initial `cargo check --locked -p Chataigne2 --tests` caught two stale packet-loop references
  during cursor integration; both were repaired before test compilation. The subsequent
  `cargo test-fast --locked -p Chataigne2 module_modules_protocol_stream_udp -- --quiet` built the
  app test executable, but its companion native binary failed with **148 unresolved MSVC/LLVM
  anonymous/internal symbols**. That command is **FAILED**. Direct execution of its successfully
  linked test binary `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` passed the filters above.
  Strict Clippy, workspace compilation, and fresh-native artifact closeout are still pending.
- Concurrent footer UI work was committed separately as `3b4bdc7b`; preserve that parent. The
  updated size scan includes it: **1406 source files / 42 oversized**, with three new outbound
  model/test files and one separately committed `EngineStatus.svelte`; no new size exception.

Actor-side `collect_outputs_recursive`, output-ID accumulation, and hostname extraction remain
open read-path work. The bounded native envelope does not qualify their scan, transient storage,
or cleanup costs. Do not replace that audit with an arbitrary small output count that prevents
large graphs, nor pin unbudgeted full snapshots per queued command. TCP/WebSocket/serial worker
ownership, the wider module audit, and generator elapsed-time accounting remain open after it.

Final batch 11 validation (Windows/default features, current source plus separate parent `3b4bdc7b`):

- Direct app test filters passed **47 concrete-stream tests** (including **26 UDP**) and **44 shared
  stream tests**, **91 distinct tests** total. The UDP filter also passed separately and is not
  double-counted. All six new outbound cases passed on controlled loopback; no physical endpoints
  were opened and no other platform or full-product throughput qualification is claimed.
- `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed (41.87s).
  `cargo check --locked --workspace --tests` passed (47.88s, including its build-lock wait).
- After the failed incremental companion link, `cargo clean --locked -p Chataigne2 --profile dev`
  removed only app artifacts (2809 files / 4.9 GiB). The fresh
  `cargo build --locked -p Chataigne2 --bin Chataigne2` **passed in 2m12s**. No source workaround,
  dependency-cache removal, or user-process interruption was used. Recurring anonymous-symbol
  incremental failures remain P12 tooling evidence; the successful fresh build qualifies this batch.
- Root and Golden Core `cargo fmt --all` / `--check`, maintained-doc Prettier, and `git diff --check`
  passed. All owned jobs are terminal. The separate UI commit remains preserved as the parent.

Focused whole-command admission/cursor/retirement verification is complete. **Return to High
requested; actual effort unchanged/unobservable** because no supported control is exposed.

Commit and push this verified milestone on `main`. Keep P03 **IN_PROGRESS** and resume the
actor-side output-tree preparation/cleanup audit before the remaining module and generator work.

P03 batch 12 resumed at clean delivered `95c64618376b6835841d8800ff1cbbaba0c72f1e` on `main`,
matching `origin/main`. Previous goal turn: **progress**, with batch 11 verified and delivered.
No owned jobs remain live at resume. Prepared output plans need a revision checked against
applied state before callbacks run; app callback-only invalidation would admit stale destinations
after a parameter edit followed immediately by a script send. Generic subtree change identity
belongs in Golden Engine, with UDP target policy and plan ownership in the app. **XHigh requested;
actual effort unchanged/unobservable** because no supported control is exposed.

Batch 12 ownership and intermediate evidence:

- Golden Engine now exposes opaque, engine-lifetime `SubtreeRevision` identities through process
  snapshots. Semantic change targets feed project dirty tracking and local revision updates in
  one pass. Move targets include both parents; deleted-node entries are removed. Opaque authored
  publication and uncaptured edit callbacks mark their target directly rather than depending on
  UI events; script configuration uses the same path. Snapshots copy the index once when already
  projecting the tree, then folds descendant revisions iteratively in linear time. Mutation marks
  only changed nodes, avoiding both ancestor walks and whole-map cloning. Parameter overlays use a fresh
  revision namespace; unchanged clones preserve identity. Ancestor enable state stays explicit.
- UDP now caches an immutable, pooled destination plan under its root and subtree revision.
  Script/custom-event sends check applied state before admitting a command. Repeated sends retain
  the shared plan and move one datagram; they do not rescan outputs or copy target strings. Native
  commands retain the old plan across later edits. Cache replacement, disable/destroy, and queue
  rejection cannot perform final target cleanup on the actor; the existing payload pool owns it.
- Cold preparation follows folder sibling/parent links with constant auxiliary space and a
  whole-snapshot node bound, eliminating recursive stack growth and output-ID accumulation.
  Host strings are length-checked while borrowed before trim/copy. Failed preparation is not
  cached, allowing unchanged settings to recover after pool pressure. Cold traversal and failed
  builder disposal still run on the actor and remain an explicit P03 repair.
- Initial core checking passed. Three initial revision tests passed before the subsequent opaque
  and metadata coverage was added. Initial app/test checking passed with one obsolete unused
  `child_string_param` helper warning; the unused helper was removed. Final engine and UDP tests
  are running. New app fixtures cover 20,000 nested folders, cache storage reuse, invalid-host
  recovery, and immediate endpoint edits preserving an already-accepted send's interpretation.

Do not qualify a cold builder by running a small cursor against a fresh full-tree snapshot on every
turn: that creates graph-size work per slice and can become quadratic. Finish its preparation and
failure cleanup with an explicitly bounded public read/runtime ownership boundary. The remaining
module/channel and generator elapsed-time audits are still required; P03 is not complete.

Before the final constant-time mutation / iterative snapshot-fold repair, the full engine run
passed **526 unit tests**, **8 integration tests**, and **7 doctests**; **3 existing unit tests
were ignored**. Final-source reruns include an additional 20,000-node deep revision-fold fixture.
That final core rerun and the owning UDP run are live; no final acceptance claim yet. The source
scan is **1412 files / 42 oversized**, with six new focused sources and no new size exception.

The final revision-fold source passed **527 engine unit tests**, **8 integration tests**, and
**7 doctests**, with the same **3 ignored** manual cases. The first UDP run passed 28/29: its
native failure fixture counted logs after only one four-event drain, so an interface-discovery
event could defer one expected log. The fixture now drains bounded turns to disconnection; the
owning rerun passed **29/29**. Concrete streams **50**, shared streams **44**, HTTP **36**,
Sound Card/null **36**, OSC/OSCQuery **25**, and the first-formula-tick cache regression **1**
passed; enumerating their test names confirms **192 distinct app tests**, with no overlap.
Strict engine/IO/app Clippy and workspace/test checking also passed before the final admission
preflight addition below.

Final ownership audit found that unavailable modules could prepare a plan before discovering
closed native admission. `PendingSender::is_disconnected` and
`BoundedWorker::is_accepting_commands` now expose cheap, explicitly non-reserving hints. UDP checks
them before preparation, releases cached data on closure/disconnection, and keeps its reusable
plan under capacity pressure. This prevents failed/retiring modules from pinning shared payload
slots. Existing IO fixtures now assert native closure before reply drain, cancellation/graceful
close, and full-versus-closed behavior; all **33 IO tests passed**. One further UDP regression
checks that unavailable native admission retains no plan. Final app/lint/workspace/native checks
are running or pending after this small IO/app addition; core engine source remains unchanged
since its full 542-test pass.

Final admission-preflight source passed **30 UDP tests**, including the new unavailable-transport
case, and strict `cargo clippy --locked -p golden_engine -p golden_io -p Chataigne2 --all-targets
-- -D warnings`. Final direct app preservation filters passed concrete streams **51**, shared
streams **44**, HTTP **36**, Sound Card/null **36**, OSC/OSCQuery **25**, and initial formula cache
**1**: **193 distinct app tests** (UDP is included, not counted twice). With engine **542** and IO
**33**, this batch records **768 passing tests** plus the same three ignored engine measurements.
Workspace/test compilation and native build closeout follow; no live-job completion is assumed.

Final `cargo check --locked --workspace --tests` passed (11.20s). Native default compilation
then reproduced the known incremental MSVC/LLVM anonymous-symbol failure, this time **204
unresolved externals**; that command is **FAILED**. With all owned jobs terminal and no live
native app/compiler observed, app-package-only `cargo clean --locked -p Chataigne2 --profile dev`
removed **4300 files / 7.0 GiB**. A fresh native build is running. No runtime source workaround,
dependency-cache deletion, or user-process interruption was used. Keep the recurring incremental
artifact defect open under P12; do not claim native success before the fresh command completes.

The fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` then **passed in 2m23s**.
Batch 12 closeout: root and Golden Core `cargo fmt --all` / `--check`, maintained-doc Prettier,
and `git diff --check` passed. All owned jobs are terminal. No source changes followed the final
admission checks; the app-package clean removed build artifacts only. Native validation uses
Windows/default features; endpoint fixtures use controlled loopback, missing serial endpoints,
and null audio. The existing default TCP fixture port collision and controlled injected errors
were diagnostics in passing suites, not exclusive-port/device qualification. Other platforms,
physical device routes, ignored measurements, and full-product scale remain unqualified.

Focused revision/cache/admission ownership verification is complete. **Return to High requested;
actual effort unchanged/unobservable** because no supported control is exposed. Commit and push
the verified batch on `main`. P03 remains **IN_PROGRESS** for cold preparation, failed-builder
cleanup, and the wider module/channel/generator audit. P04-P18 retain their implementation states.

P03 batch 13 resumed at clean delivered `a617d313cb3e70191d53033b1ea13921e5f47201` on `main`,
matching `origin/main`. Previous goal turn: **progress**, with batch 12 verified and delivered.
No owned jobs remain live at resume. Cold worker preparation needs a byte-budgeted snapshot
lease whose final owner stays off the actor, including ordinary engine-held Arc clones. Golden
Engine owns allocation accounting and a canonical snapshot lease pool; Golden IO owns reusable
shared-Arc lifetime mechanics. UDP preparation and target policy remain app-owned. **XHigh
requested; actual effort unchanged/unobservable** because no supported control is exposed.

### P03 batch 13: native cold UDP preparation and shared snapshot lifetime

- Golden IO can retain an existing immutable Arc, deduplicating its reservation without wrapping
  away ordinary raw owners. Its disposer alone checks external releases every 100 ms, keeps item
  and byte capacity through final cleanup, and survives the last pool handle. New regressions
  verify deduplication, original-Arc rejection, ordinary-owner capacity retention, and off-consumer
  final destruction. A final ownership review also replaced the collector's strong-count-only
  destruction with an atomic unwrap claim: a concurrent weak-reference upgrade returns the pool
  owner for retry instead of moving final destruction onto a consumer. A direct regression covers
  both a rejected claim and weak invalidation after success. This generic correction postdates the
  first app pass; final validation must use the corrected IO implementation.
- Golden Engine owns cached conservative allocation accounting for snapshot records, dynamic
  parameter/control/constraint/presentation/script/dashboard data, spare capacities, aliases, and
  revision indexes. Snapshot clones share overlays as well as base storage. The canonical
  384-allocation / 1 GiB snapshot lease reserves the outer view and each shared allocation, so
  ordinary clones cannot move final shared-data cleanup back onto the actor. No protocol or
  persistence format changed; these reservations do not claim whole-process RSS qualification.
- UDP submits preparation and send as one bounded native envelope. The first admitted command
  owns a frozen snapshot lease and constant-space traversal cursor; matching revisions share its
  pending or completed ticket. Preparation yields after 64 traversal steps, checks cancellation
  per step, validates borrowed hosts before copying, and seals the entire plan before sending.
  Builder failure and cancellation cleanup run on the worker. Async errors truthfully report no
  output sent; native setup failure resolves every admitted ticket. Transient failures may retry.
  Cache installation follows successful native admission, preventing stranded pending tickets.
- Pending envelopes reserve a conservative plan bound (at most the command budget less payload
  headroom); prepared envelopes charge actual retained capacity. Snapshot storage has its separate
  canonical reservation. The existing 256-item / 1 MiB command mailbox, single extra native
  cursor, FIFO fanout, graceful retirement, passive wake/recovery, and reply limits remain.
  Default output creation now checks only whether its manager is empty, preserving user folders
  without an actor-side search.
- New controlled tests cover a 20k-deep tree, bounded native traversal, cancellation, pending-ticket
  reuse, invalid preparation after blocked setup, and accepted old-configuration ownership while
  a replacement envelope is rejected. Engine tests cover dynamic spare capacity, COW overlays,
  every variable-sized DTO group, and a lease of a 50k-record immutable view. This is deterministic
  boundary evidence, not full-product scale or native interaction qualification.
- Intermediate compile failures were corrected: an accounting match initially assumed a widget
  enum was Copy; fixture migration left stale helper calls; a request destructure omitted
  `frame_kind`. Missing public error-field docs were also fixed. The subsequent combined
  engine/app test compilation passed. These diagnostics do not establish the separate final gates.
- PASSED `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
module_modules_protocol_stream_udp`: **34/34**. Final app preservation filters on that exact
  binary passed concrete streams **55** (including UDP), shared streams **44**, HTTP **36**,
  Sound Card/null **36**, OSC/OSCQuery **25**, and initial Formula cache **1**: **197 distinct app
  tests**. Controlled failures and an occupied default TCP fixture port were diagnostics in passing
  suites. No physical device or GUI automation was used.
- Source inventory: **1416 files / 42 oversized**, no new exception. Runtime source is frozen for
  final full engine/IO, strict Clippy, workspace/test, and native build checks; those commands are
  pending/running and are not yet acceptance evidence. The user's Cargo watcher was left running;
  validation waited for its native compiler to release the shared build lock.

The optimized `cargo test --locked -p golden_engine -p golden_io` completed successfully:
engine **530 unit + 8 integration + 7 doctests**, with **3 ignored**, and IO **35 tests**. This
binary predates the atomic ownership-claim addition; the fresh dev-profile engine/IO and app
runs below must qualify that final change. `cargo check --locked --workspace --tests` passed.
The first strict Clippy run rejected the new disposer callback's complex function type; named
erased-payload/callback types replace it. That lint rerun and the native/default build remain live.

The first native/default build passed without cache cleaning; a second UDP run also passed 34/34.
The final ownership review tightened atomic claims further: claim/reinsertion happens under the
registry lock, with destruction deferred outside it. This prevents weak revival plus concurrent
registration from creating a duplicate pool owner while a claim is in flight. The direct regression
also asserts that claiming alone never invokes a destructor. This last synchronization change
requires the fresh core/IO, app, lint, and native checks below; earlier passes are preserved evidence.

Final synchronization source passed `cargo test-fast --locked -p golden_io -p golden_engine`:
engine **530 unit + 8 integration + 7 doctests** and IO **36 tests**, with the same **3 ignored**
engine measurements. Focused shared-snapshot/Arc ownership verification is complete. **Return
to High requested; actual effort unchanged/unobservable** because no supported control is exposed.
The next Clippy pass found an engine revision-lease result type needing an alias and a test's
default-then-field initializer; both were mechanically corrected without behavior changes.
Routine final app/native/lint closeout remains. No further runtime design changes are planned.

The final `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
module_modules_protocol_stream_udp` passed **34/34**. All six preservation filters were rerun
against that rebuilt binary and passed the same **197 distinct app tests**. Together with
engine **545** and IO **36**, this milestone records **778 passing tests**, plus **3 ignored**
manual engine measurements; overlapping profiles/UDP subsets are not added again. The full
engine/IO pass preceded only the revision result-type alias and equivalent test initializer
cleanup; final Clippy recompiles those sources. The final native build and lint remain live.

Final `cargo clippy --locked -p golden_engine -p golden_io -p Chataigne2 --all-targets -- -D
warnings` and `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED**. No native-link
cache clean or runtime workaround was needed in this batch; the prior recurring artifact issue
remains open under P12. Workspace/test compilation passed before the mechanical alias/initializer
cleanup; final strict lint and native compilation cover those sources. All owned jobs are terminal.
Windows/default features were used. Controlled loopback, missing serial endpoints, and null audio
fixtures do not qualify physical devices, other platforms, ignored measurements, or full-product scale.

Commit and push this verified milestone on `main`. P03 remains **IN_PROGRESS** for the native
reply-backpressure fairness and broader module/channel/generator audits. P04-P18 retain their
implementation states. Preserve the user's running watcher and all earlier delivered work.

Closeout formatting evidence: root and Golden Core `cargo fmt --all` / `--check`, maintained-doc
Prettier, and `git diff --check` passed. Source inventory remains **1416 / 42 oversized**. No new
size exception, generated protocol drift, dependency change, or unrelated source edit is included.

P03 batch 14 resumed at clean delivered `df0c787621162dead9623751d989478cf09198ae` on `main`,
matching fetched `origin/main`. Previous goal turn: **progress**; batch 13 was verified, committed,
and pushed. No owned jobs are live. Golden IO owns nonblocking weighted reply admission and its
queue diagnostics; the UDP adapter owns native per-source reply continuations and fairness.
Current source blocks the socket loop inside `publish_weighted` when replies fill, including
unsolicited input/interface results. That can strand admitted output behind input backpressure.
**XHigh requested; actual effort unchanged/unobservable**, with no supported control exposed.

### P03 batch 14: duplex UDP reply backpressure

- Golden IO exposes one-attempt weighted reply admission returning the original event on `Full`
  or disconnection, plus read-only event queue metrics. Sequential blocking publication retains
  its existing cancellable behavior. All **38 IO tests passed**, including full/oversized ownership,
  independent command service under reply pressure, cancellation, and the previous lifetime cases.
- A controlled native regression with one reply slot filled by input reproduced the defect before
  the UDP change: the admitted cold 20k-deep output command timed out at its loopback receiver.
  `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
full_input_replies_do_not_strand` was **FAILED** as expected for that baseline; cancellation
  still terminated its worker. This is an observed starvation case, not only a source finding.
- The app-owned adapter now keeps one pending input, output-outcome, and interface result in a
  shared FIFO. Local bounds are 32 MiB, 64 KiB, and 64 KiB plus the fixed event envelope, respectively.
  Input/discovery backpressure pauses only those producers. Native cold preparation and successful
  unlogged sends can progress; a pending output outcome pauses the already-advanced destination
  cursor. Queue retry never repeats a native send. Retirement drains all results before exit;
  cancellation abandons explicitly cancelled work without joining. Oversized details report a
  bounded failure instead of stranding later results. Discovery errors are bounded before caching.
- Tests now cover exact input resumption, pending-source FIFO reuse, logged fanout under pressure,
  retirement, cancellation, and oversized-result recovery. `cargo check --locked -p Chataigne2
-p golden_io --tests` passed the adapter change. Final runtime and strict lint checks are live;
  no final acceptance claim yet. No engine, protocol, persistence, or UI source changed.

Final `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
module_modules_protocol_stream_udp` **passed 39/39**, including the previously failing cold-send
case, exact input resumption, source FIFO, logged fanout/retirement, cancellation, and oversized
reply recovery. Strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D
warnings` and `cargo check --locked --workspace --tests` **passed**. Direct preservation filters
on that binary passed concrete streams **60** (including UDP), shared streams **44**, HTTP **36**,
Sound Card/null **36**, OSC/OSCQuery **25**, and initial Formula cache **1**: **202 distinct app
tests**, or **240** with the **38 IO** tests. Prior full engine passes are not added to this batch.

The default native build **failed** with the known incremental MSVC/LLVM anonymous-symbol
artifact defect, **74 unresolved externals**. After all owned checks were terminal and no live
native app/compiler was observed, the resolved Cargo target was verified inside this workspace.
App-package-only `cargo clean --locked -p Chataigne2 --profile dev` removed **6849 files / 9.6 GiB**.
A fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` is running; no native pass is assumed.
No user process was interrupted, dependency cache removed, or runtime workaround introduced.
The recurring artifact issue remains P12 work. Source inventory: **1419 / 42 oversized**.

Focused duplex reply/admitted-send ownership verification is complete. **Return to High requested;
actual effort unchanged/unobservable** because no supported control is exposed. Only routine
native-build/documentation/delivery closeout remains for this batch; P03 is still **IN_PROGRESS**.

The fresh native build **PASSED in 1m53s**, completing the Windows/default-feature build gate.
All owned jobs are terminal. Source stayed unchanged after the final strict lint, workspace/test,
and runtime passes; the package clean removed build artifacts only. `cargo test-fast --locked
-p golden_io` passed **38 tests**. The **202 distinct app** preservation cases make **240 current
passing tests**, with no duplicated UDP/profile totals. Full engine/runtime/platform/device/scale
qualification is not inferred from these filters. The fixtures used controlled loopback, missing
serial endpoints, and null audio; occupied default-port/injected-error diagnostics occurred in
passing suites. No physical device or GUI automation was used.

Root and Golden Core `cargo fmt --all` / `--check`, explicit app UDP `rustfmt --edition 2021
--check`, maintained-doc Prettier, and `git diff --check` passed. No new source-size exception,
protocol-generation drift, dependency change, or unrelated source edit is included.
Commit and push the verified batch on `main`, then resume TCP/WebSocket/serial worker ownership,
the broader module/channel audit, and generator time accounting. P03 remains **IN_PROGRESS**;
P04-P18 retain their states, including the recurring P12 native incremental-artifact gap.

P03 batch 15 resumed at clean delivered `0196c0f88fbfa3306996ade41f692ddd00ff32f1` on `main`,
matching fetched `origin/main`. Previous turn: **progress**; batch 14 was verified and delivered.
No owned jobs are live. TCP client currently resolves DNS on the caller, owns an unbounded command
channel, joins on stop, and uses `write_all` on a nonblocking socket without a resumable offset.
The app owns TCP connection policy and native cursor/generation semantics; Golden IO supplies
bounded lifetime/admission/cancellation primitives. Shared stream reply continuations will be
reused by TCP and UDP. **XHigh requested; actual effort unchanged/unobservable**, with no supported
control exposed. TCP actor-side input preparation/passive service remains an explicit following
step at resume; passive owner service is now included, while worker-side input preparation remains
open. This batch establishes the native boundary and graceful generation ownership first.

### P03 batch 15: TCP client native ownership and completion service

- TCP client now uses Golden IO's bounded worker with 16 global native lifetimes, 256 commands /
  1 MiB, one extra 1 MiB write cursor, and 64 replies / 32 MiB; owner drains four events. Endpoint
  resolution/connect/reconnect, health checks, writes, outgoing logs, and native cleanup stay on
  the worker. Stop cancels without a join; blocked setup retains its lifetime reservation.
- Commands capture their connection generation. Partial writes retain their suffix and submit
  at most 64 KiB per step; `WouldBlock`/interruption never restart a prefix. A five-second absolute
  command deadline fails stalled output, and connection failure explicitly reports written bytes
  without replaying a command on a replacement socket. Work turns bound command steps and reads.
- Shared app stream reply continuations and wake/deadline adapters serve TCP and UDP. UDP keeps
  its app-owned limits/error policy; no private compatibility wrapper is retained. Its output
  plan sources were also formatted with the repository formatter. TCP nodes use passive owner
  service, preserve one retiring generation across endpoint edits/disable, and consume outgoing
  log completion events instead of formatting full payloads or declaring sends on the actor.
- Initial engine/app test compilation and strict app Clippy passed. The first concrete stream
  run passed **66/67**; root TCP disable timed out waiting for retirement. Inspection found that
  Golden Engine unconditionally skipped disabled-node worker callbacks. This is an owning engine
  contract gap, not an endpoint timeout. A public default-false `worker_wake_when_disabled` hook
  now permits bounded accepted-completion service. Enum dispatch forwards it for built-ins and
  consumer variants; TCP and UDP opt in only while lifecycle/accepted work remains. A regression
  checks default suppression, explicit opt-in, ending the opt-in, timed-clock separation, and
  removal fencing. Native/module tests also cover disabled graceful draining and endpoint handoff.
- Final core/app compilation passed the hook change. Full engine/IO and concrete stream runs are
  pending/live; no final verification claim yet. TCP worker-side input parsing, callback preparation,
  and grammar/connection-generation input fencing remain the next P03 boundary. TCP server,
  WebSocket, serial, Sound Card/channel audits, and generator time accounting remain open.
- Source scan: **1425 files / 42 oversized**, no new exception. Runtime/test files remain split
  into focused modules; no wire format, persistence schema, dependency, or UI source changed.

Retirement admission is now explicit at the generic IO boundary: `command_admission_closed`
observes a closed producer before its accepted prefix is drained. TCP/UDP stop new input and
reconnect/discovery work in that state, while retaining admitted sends/results. A connector that
returns after closure cannot install a new TCP connection. The existing IO command-turn regression
also checks that closure is visible before either queued command is consumed. This additive IO
query postdates the initial core run's compilation; final IO/app checks must cover it. No existing
Golden IO method changed behavior in this step.

After the disabled-completion hook, concrete streams **passed 70/70**, including TCP root
disable/re-enable, disabled draining of a saturated admitted prefix, endpoint handoff, idle
passivity, partial-write/deadline/generation cases, and all UDP regressions. The engine unit
suite passed **531** with **3 ignored**, and its **8 integration tests** passed; IO passed **38**.
The same command's **7 doctests failed** with `E0460` because the IO source changed during the
validation run and its newer artifact no longer matched the engine artifact. This was a
validation sequencing error; a source-settled doc-only rerun is queued. No doctest pass is assumed.

A final native-routing review added snapshot-based TCP endpoint admission, checked before moving
the payload to the worker. A parameter edit followed by a script call in the same batch must
reject pending configuration instead of sending on the prior endpoint. The saturated endpoint
handoff regression now includes that rejected call and still requires the old admitted prefix
to arrive exactly once at its original peer. This app-only change postdates the 70-test run;
final stream/lint/native checks must use the updated source. Runtime source is now frozen.

The source-settled IO rerun passed **38/38**, including pre-drain admission closure. The first
native/default build failed with the recurring incremental MSVC/LLVM artifact issue, **19
unresolved externals**. Final source still needs a fresh native build after the current app/doc/lint
jobs finish. No cache cleanup, user-process interruption, or runtime workaround has been performed
for that failure yet. Workspace/test compilation and the prior strict engine/IO/app Clippy run
passed before the final applied-snapshot admission addition; the latest lint rerun is pending.

Final applied-snapshot source passed **70/70 concrete stream tests** and strict
`cargo clippy --locked -p golden_engine -p golden_io -p Chataigne2 --all-targets -- -D warnings`.
The doc-only settled rerun `cargo test-fast --locked -p golden_engine --doc` passed **7/7**;
with **531 engine unit + 8 integration**, that is **546 engine tests**, plus the same **3 ignored**
manual measurements. Source-settled IO passed **38/38**. App preservation filters passed shared
streams **44**, HTTP **36**, Sound Card/null **36**, OSC/OSCQuery **25**, and initial Formula cache
**1**, making **212 distinct app tests** with concrete streams. Total current evidence is **796
passing tests**; overlapping profiles and UDP subsets are not counted twice.

All owned tests/lint/check jobs were terminal, and no live native app/compiler was observed.
The resolved Cargo target was verified inside the workspace before app-package-only artifact
cleanup. A fresh default native build is now running. The user watcher remains untouched; no
dependency cache was removed and no runtime workaround was introduced. Native success is pending.

The app-only cleanup command `cargo clean --locked -p Chataigne2 --profile dev` removed **4955
files / 8.1 GiB**. Focused native cursor/generation, applied-snapshot admission, and disabled
completion verification is complete. **Return to High requested; actual effort unchanged/unobservable**
because no supported control is exposed. Routine native-build/documentation/delivery closeout
remains; TCP worker-side input preparation is still the next dependency-ready P03 task.

The fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED in 2m14s**. The final
`cargo check --locked --workspace --tests` against settled source **passed in 32.97s**. All owned
jobs are terminal. Root and Golden Core formatting, explicit formatting of touched app modules,
maintained-doc Prettier, and whitespace checks passed. Source inventory remains **1425 / 42
oversized**, with no new exception. The package clean changed build artifacts only.

This milestone records **796 distinct passing tests** plus **3 ignored** manual engine cases;
earlier failure evidence is preserved above rather than relabelled. Validation used Windows/default
features, controlled loopback/missing-serial endpoints, and null audio. Occupied default-port and
injected-error logs were diagnostics within passing suites. No physical-device, other-platform,
ignored-measurement, or full-product scale qualification follows. No GUI automation or user-process
interruption was used. Commit and push this verified batch on `main`, then continue the remaining
TCP input boundary. P03 remains **IN_PROGRESS**; P04-P18 retain their states, including the P12
incremental native-link artifact issue.

### P03 batch 16: TCP worker input preparation

Resumed clean delivered `1663106e9742fc67dc900295fb5674ddddd8c108` on `main`, matching fetched
`origin/main`; no owned jobs are live. Shared stream input preparation/delivery belongs in app
common streaming code; TCP connection/parser generations remain transport policy. This batch
moves TCP parsing, callback JSON, and bounded input logs off the actor and preserves receipt-owned
delivery. Partial frames must not span grammar/processing or connection generations. Requested
**XHigh; actual effort unchanged/unobservable** because no supported control is exposed.

Shared UDP preparation/delivery moved into app common streaming modules and TCP now consumes the
same immutable retained input envelope. Live input settings update independently of socket identity;
TCP partial frames are fenced by parser and connection generations. Owner callbacks use individual
receipts, a bounded four-event batch, and snapshot-free callback service. The stale TCP warning
lookup now targets the bound connection folder. The first check found one old UDP test type after
extraction; it was corrected. Five native generation/configuration/retirement regressions and three
owner delivery/passivity/destroy regressions were added; the first settled stream test job is live.
No validation pass was claimed at that point. The full P03/module/channel/generator work remains open.

The settled stream run passed **77/77** in **2.39s** after **2m53s** compilation. This includes
five native input tests and three owner tests added here; the UDP callback receipt regression
moved to common streaming, so its count is not duplicated. Shared streaming passed **45**, HTTP
**36**, Sound Card **35** plus null-device product evidence **1**, OSC **25**, and initial Formula
cache **1**: **220 distinct app tests passed**. Direct executable filters were checked against
the compiled test inventory; unmatched/zero-test attempts were corrected and add no pass evidence.
`cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in **49.02s**;
`cargo check --locked --workspace --tests` passed in **21.29s**. Golden runtime source is unchanged
in this batch; previous engine/IO passes remain historical evidence, not new executions.

Focused input generation/receipt work is verified. **Return to High requested; actual effort
unchanged/unobservable** because no supported control is exposed. Default native build is live;
delivery remains pending that gate. Root/Core Cargo formatting, explicit app module formatting,
maintained-doc Prettier and whitespace checks have passed against the current source.

Review also identified a distinct remaining P03 lifetime boundary: callback JSON `Arc` copies
held by routing/replay can outlive a prepared input envelope's reservation. The shared stream
pool owns final envelope disposal, while downstream event copies currently follow engine event
lifetimes. Audit byte admission/final disposal at that handoff before expanding native input to
the next transports; do not infer a routing/replay byte budget from the envelope budget. Then
continue TCP server/WebSocket/serial, Sound Card/channel ownership, and generator time accounting.

The first default native build **FAILED** with MSVC `LNK1120`: **1081 unresolved externals**,
including compiler-generated `anon.*.llvm.*` and local function symbols. This matches the recurring
incremental app-artifact failure class retained under P12; tests/lint/checks are not relabelled as
native success. All owned jobs are terminal. Verify no live app/compiler and the resolved target
inside the workspace, clean the app package's dev artifacts only, then require a fresh native pass.

No live app, Cargo, Rust compiler, or linker process was observed before cleanup; the resolved
target was verified as `D:\Projects\Dev\Chataigne2\target`. The app-only command
`cargo clean --locked -p Chataigne2 --profile dev` removed **2231 files / 4.7 GiB**. Fresh default
native build is live. Dependency caches, compiler flags, and user processes were left alone.
Fresh source-size scan (`rg --files apps crates packages`, Rust/TS/Svelte/JS/MJS) is **1614 files /
42 oversized**; all touched source stays below 1000 lines (largest TCP node: **559**). This explicit
broader scan includes more files than the earlier inventory; it does not imply new size exceptions.

The fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED in 2m02s** against the
same settled runtime source. The failed incremental attempt remains evidence above; scoped app
artifact cleanup is recovery, not a P12 fix. All owned jobs are terminal. Final root/Core Cargo
formatting, explicit app formatting, maintained-doc Prettier, and whitespace checks pass. Milestone
evidence is **220 distinct passing app tests**, strict IO/app Clippy, workspace/test compilation,
and fresh default native build. Validation used Windows/default features, controlled loopback and
missing-serial endpoints, and null audio. Physical devices, other platforms, full-product scale,
and UI/native interaction were not qualified by this batch. Commit and push this milestone on
`main`, then audit the downstream callback JSON lifetime boundary. P03 remains **IN_PROGRESS**;
P04-P18 retain their states.

### P03 batch 17: callback JSON lifetime admission

Resumed clean delivered `3166e80574cdff1d1557f851ecf808222612a59b` on `main`, matching fetched
`origin/main`; no owned jobs are live. Previous turn: **progress**. Golden IO already owns atomic
final claims of externally cloned immutable Arcs. App module script policy owns callback JSON
construction and admission, so this batch adds a canonical app callback reservation and wires
native TCP/UDP preparation through it. Payload storage must remain charged after its input
envelope/delivery is gone and through ordinary event/routing/replay clones; final JSON destruction
belongs to its disposer. HTTP's response/callback envelope boundary is a following integration,
not evidence supplied by stream-only checks. **XHigh requested; actual effort unchanged/unobservable**
because no supported control is exposed.

`NativeCallbackPayload` now measures/reserves JSON on the preparing worker and binds origin/topic
without copying its JSON on the owner. The app-owned canonical pool uses Golden IO external-Arc
retention with **16,384 allocations / 256 MiB**, separate from stream envelopes/values (**256 /
256 MiB**). Each callback admits at most **32 MiB / 262,144 values / 64 levels**, including spare
vector/string capacity. TCP/UDP preparation now uses that typed payload and cached retained weight;
the old stream-local recursive JSON estimator was removed. Callback multiplicity, names, argument
shape, replay policy, and per-publication receipts remain intact. Seven lifetime/admission tests
cover raw event copies, replay/export, failure/retry, retained spare storage, per-payload limits,
and shallow binding. The first focused test job is live; no pass is claimed yet.

The focused run passed **7/7** callback tests in **0.10s** after **2m17s** compilation. Current
Golden IO verification passed **38/38**, including atomic final claims/Weak revival, externally
cloned Arc admission, producer/pool-owner exit, destructor thread ownership, panic cleanup, and
blocked cleanup capacity. Source-settled executable preservation filters passed script API **8**
(including the same seven callback tests), concrete streams **77**, shared streams **45**, HTTP
**36**, Sound Card **35** plus null product evidence **1**, OSC **25**, and initial Formula cache
**1**. Their compiled-name sets were checked for overlap: **228 distinct app tests**, **266**
including IO. Strict IO/app Clippy and workspace/test compilation are pending/live; native build
and final delivery are still required. This does not qualify unintegrated HTTP callback ownership.

Strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**47.52s**. Focused lifetime/publication work is verified; **return to High requested, actual
effort unchanged/unobservable** because no supported control is exposed. Workspace/native and
routine documentation/delivery closeout remain; other callback producers keep their own open audit.

`cargo check --locked --workspace --tests` passed in **48.84s**, including queue wait. Source is
frozen for the default native build. A fresh explicit source inventory (`rg --files apps crates
packages`, Rust/TS/Svelte/JS/MJS) is **1616 files / 42 oversized**; the new native payload module
is **119** lines and its separate test module **149**. No new size exception is introduced.
Maintained-doc Prettier and whitespace checks pass. Other producer integration remains open, in
particular HTTP's retained response/body/parsed-values cleanup and worker-prepared callback binding.

The default `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED in 1m35s**, including
queue wait, without artifact cleanup or flag changes this batch. All owned jobs are terminal.
Final root/Core Cargo formatting, explicit app-module Rustfmt, maintained-doc Prettier, and
whitespace checks pass. Evidence: **266 distinct passing IO/app tests**, strict IO/app Clippy,
workspace/test compilation, and default native build. Source inventory remains **1616 / 42**;
no new size exception, wire shape, persistence schema, dependency, or UI behavior changed.
Checks used Windows/default features, controlled loopback/missing-serial endpoints, and null
audio. Physical-device, other-platform, UI/native-interaction, and full-product scale qualification
remain separate. Commit and push this verified milestone on `main`, then integrate HTTP callback
and retained envelope ownership. P03 remains **IN_PROGRESS**; P04-P18 retain their states, including
the previously recorded P12 incremental-link artifact issue.

### P03 batch 18: retained HTTP result ownership

Resumed clean delivered `8c2c5ff866674b214670c96ee7b3a2120a01c52e` on `main`; previous turn:
**progress**, no owned jobs live. App HTTP owns response parsing, callback shape, and result policy;
Golden IO supplies retained immutable lifetimes. HTTP replies currently transfer body/parsed-value
containers and callback args to the owner. This batch retains their final cleanup off the actor,
prepares normal and graph-application-failure callbacks on the worker, and reserves callback JSON
through the canonical native callback pool. Result-admission exhaustion must produce an explicit
bounded rejection, never unbudgeted callback JSON or an automatic resend; close native admission
and report each remaining queued request without sending it. Requested **XHigh; actual effort
unchanged/unobservable**, with no supported control exposed. Broader P03 audits remain open.

HTTP now separates native `HttpOutcome` from typed retained owner replies. Bodies, request metadata,
and parsed values use a **256-allocation / 256 MiB** HTTP pool; normal, failure, and graph-application-
failure callback JSON is prepared/reserved on the worker through the existing native callback pool.
Owner selection/binding is shallow; raw callback argument vectors and owner-built failure payloads
were removed. Cached reply weights include callback reservations, container capacities, and parse
error storage. Ordinary HTTP response/JSON bounds still produce a prepared failure callback.
Result/callback resource exhaustion instead returns bounded identity/attempt feedback, closes native
command admission, and resolves the accepted suffix without further endpoint sends or unbudgeted
callback payloads. Owner warning service needs no snapshot and clears its result warning on recovery.

The first test-compilation check failed on fixture visibility and the new rejection's snapshot
match arm; both were corrected. `cargo check --locked -p Chataigne2 --tests` then **passed in
22.80s**. A final small owning pass renamed lifecycle feedback from failed to ended, since result
pressure can intentionally end a generation, and bounded parse-error log detail. Six retained/native
tests plus a node warning/recovery regression were added. The first settled HTTP test job is live;
no final pass is claimed yet. HTTP's existing periodic owner service remains a following P03 audit,
alongside TCP server/WebSocket/serial, Sound Card/channels, and generator time accounting.

The first settled HTTP run passed **39/43**; four failures were retained as evidence. One receipt
regression found that unconditional warning clearing inserted a second edit in a callback-only turn.
The large lifetime fixture retained all **257** completion envelopes in its collector, exhausting
the **256** global result slots and causing collateral parallel fixture rejections. The owning
fixture now consumes four-result batches and retains only path evidence; the production limit was
not enlarged. Warning clearing now checks committed metadata, and rejected-result warnings keep
their own application receipt before advancing (retrying only unpublished warning edits). The
second HTTP run is live against this correction; no final pass is assumed.

The corrected HTTP run **passed 43/43** in **0.23s** after **1m47s** compilation, including the
bounded collector, original callback-only receipt contract, graph failure preservation, and native
rejection/queued-suffix checks. Two focused tests were then added for warning publication discarded
by an earlier failed edit and for alternate-callback admission failing after normal callback
preparation; runtime behavior is unchanged by that addition. The final HTTP run, IO contract suite,
and strict IO/app Clippy are live/queued against settled source. No whole-suite/native success is
assumed yet. HTTP warning delivery retries only an unpublished warning and never emits a callback
outside its reservation or reissues a native request.

The **45/45** HTTP run passed in **0.23s** after **1m32s** compilation. Strict IO/app Clippy also
passed (**56.50s**, including queue wait), and IO passed **38/38**. A final queue-ordering review
then identified that warning clearing also needed an independent receipt: an earlier rejected edit
could otherwise discard the clear while a later callback retry succeeded. HTTP callback delivery
now waits for an observed clear only when a result warning exists; ordinary callback-only turns
keep their original shape. A focused earlier-failure/clear/callback regression was added, and the
**46-test** HTTP rerun is live. The prior passes precede that final runtime correction and are
not relabelled as final-source evidence. Full preservation/lint/workspace/native checks remain.

Final settled HTTP source passed **46/46** in **0.24s** after **1m55s** compilation. The strict
IO/app Clippy rerun passed in **1m10s**, including queue wait. IO's **38/38** pass is against unchanged
IO source in this batch, and supplies final-claim/Arc/disposer/retirement contract evidence. Focused
HTTP lifetime/generation/publication work is verified: **return to High requested; actual effort
unchanged/unobservable** with no supported control exposed. The broader preservation filters and
workspace/test check are live; default native build and delivery remain. Explicit source inventory
is **1618 files / 42 oversized** (same Rust/TS/Svelte/JS/MJS scope); no new exception is introduced.

Final preservation filters passed script API **8**, concrete streams **77**, shared streams **45**,
Sound Card **35** plus null product evidence **1**, OSC **25**, and initial Formula cache **1**.
With final HTTP **46**, their compiled-name sets contain **238 distinct passing app tests**; IO
adds **38**, for **276 current IO/app passes**. Earlier overlapping HTTP runs are not added again.
`cargo check --locked --workspace --tests` passed in **18.89s**. Root/Core Cargo formatting,
explicit owning app Rustfmt, maintained-doc Prettier, and whitespace checks pass. Runtime source
is settled; the default native build is live. HTTP's passive owner conversion remains the next
P03 boundary before the broader transport/module/channel/generator audit.

The default `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED in 1m36s**. All owned
jobs are terminal; no artifact cleanup or compiler flag changes were needed this batch. Final
root/Core Cargo formatting, explicit HTTP-module Rustfmt, maintained-doc Prettier and whitespace
checks pass. Milestone evidence: **276 distinct IO/app tests** (238 app + 38 IO), final strict
IO/app Clippy, workspace/test compilation, and default native build. Earlier failures and
intermediate passes remain labelled above. Inventory remains **1618 / 42 oversized**; the HTTP
node is **891** lines, response service **123**, prepared results **238**, and their native tests
**246**, with no new size exception. No wire shape, persistence schema, dependency, or UI source
changed. Checks used Windows/default features, controlled HTTP/stream loopbacks, missing-serial
fixtures, and null audio. They do not qualify physical devices, other platforms, full-product
scale, or UI/native interaction. Commit and push this milestone on `main`, then convert HTTP's
periodic owner to worker/deadline wakeups. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 19: passive HTTP service

Resumed clean delivered `7836d1bf0bad9d6c4b7b0c89a2e1944d0f051587` on `main`; previous turn:
**progress**, no owned jobs live. HTTP still selects a 120 Hz compiled kernel. This batch moves
owner service to worker/deadline wakes, generalizes the app's existing stream wake adapter into
module-common code, and signals resolved HTTP generations before potentially blocked native cleanup.
Preserve HTTP's current disabled policy: accepted results stay bounded/paused until enable explicitly
re-arms the owner. Destruction must discard pending runtime input so undo cannot revive it. Script
argument helpers will be split out before the node coordinator grows past its size guardrail.
Requested **XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

Review also identified an existing HTTP admission/configuration boundary for a following focused
step: script admission currently trusts the dirty flag rather than fencing an applied parameter edit
before callbacks dispatch; transport configuration reads cached handles and needs its own input-bound
audit. This is separate from notification ownership and must not be inferred fixed by passive service.

HTTP now uses passive worker callbacks and module-common `ModuleWake` notifications/deadlines;
TCP/UDP use the same moved adapter with no legacy alias. Native HTTP workers notify on result
publication, resolved/aborted generation phase, and producer exit. Phase notification precedes
potentially blocked client cleanup. Changes to transport inputs, auth enablement, or effective
enable re-arm the owner; retry deadlines replace periodic polling. Pending graph/callback work
re-arms bounded wake service, and callback-only service needs no process snapshot. Disabled
completions remain paused by the default engine policy; enable explicitly wakes the backlog.
Destroy now clears pending owner results and deadline state, fixing undo reviving queued runtime
input. Script helpers moved to a focused sibling file to keep the coordinator readable.

`cargo check --locked -p Chataigne2 --tests` passed in **19.91s**. The first HTTP run passed
**51/51** in **0.31s** after **2m05s** compilation, including five new passive/phase tests and
the original disabled/reconfigure/receipt/lifetime regressions. The undo test was strengthened
to assert that notifying the removed source fires no restored-owner callback; a settled rerun
is live. No runtime behavior changed after the first pass. Incidental formatting from recursing
through module-common was reverted through patches before that run; only owning files remain.
Full preservation, strict lint, workspace/test compilation, native build and delivery are pending.

The strengthened settled HTTP run passed **51/51** in **0.31s** after **57.03s** compilation;
the removed source fired **zero** callbacks on the restored owner. IO passed **38/38** in
**0.20s**, and strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings`
passed in **24.40s**. Preservation filters passed script API **8**, concrete streams **77**, shared
streams **45**, Sound Card **35** plus null product evidence **1**, OSC **25**, and first Formula
cache **1**. Their disjoint compiled-name sets plus HTTP contain **243 distinct app tests**; with IO,
**281** tests passed. Repeated HTTP profiles are not added twice. Source is settled. Focused wake/
lifetime work is verified: **return to High requested; actual effort unchanged/unobservable**
because no supported control is exposed. Workspace/test and default native closeout remain.

The final `cargo check --locked --workspace --tests` **passed in 19.48s** and default
`cargo build --locked -p Chataigne2 --bin Chataigne2` **passed in 1m43s**. All owned jobs are
terminal. Final root/Core Cargo formatting, explicit owning app Rustfmt, maintained-doc Prettier,
and whitespace checks pass. Source inventory is **1620 files / 42 oversized**; the HTTP coordinator
is **774** lines, script helpers **189**, shared wake adapter **35**, and passive tests **165**.
No new size exception, wire shape, persistence schema, dependency, or UI source changed. Evidence
is **281 distinct IO/app passes** (243 app + 38 IO), strict IO/app Clippy, workspace/test compilation,
and default native build. No artifact cleanup or compiler flag changes were needed this batch.
Validation used Windows/default features, controlled loopbacks/injected native gates, missing-serial
fixtures, and null audio; physical devices, other platforms, full-product scale, and UI/native
interaction remain separate qualifications. Commit and push this milestone on `main`, then finish
the HTTP applied-configuration admission and configuration-bound audit recorded above. P03 remains
**IN_PROGRESS**; broader TCP server/WebSocket/serial, Sound Card/channel and generator work remains,
and P04-P18 retain their states.

### P03 batch 20: HTTP configuration admission

Resumed clean delivered `0edb6f489d203af76c97f1788405d165dd42f057` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. App HTTP owns authored settings and
admission. This batch reads configuration from the applied process snapshot, compares bounded
active inputs before command/script sends, and captures inputs only for successfully configured
generations. Configuration validation remains an edit-time backend operation; admission compares
borrowed values without reparsing or cloning settings. Source input and native retained-capacity
limits must be checked before allocation/cloning at their respective boundaries. Inactive auth
fields must not retain unused credentials or cause transport identity changes. Requested **XHigh;
actual effort unchanged/unobservable**, with no supported control exposed.

Implemented bounded borrowed `HttpSettings` from applied snapshots, captured only after successful
generation configuration. Endpoint/auth edits fence script/command admission before parameter
callbacks dispatch. Auth-folder metadata uses the same effective-settings comparison: disabled or
inactive auth edits and enabling mode `None` do not unnecessarily close admission. Preparation
checks source limits before URL/header parsing and retains only active auth fields. Native manager
and spawn boundaries validate capacities/ranges before configuration clone or lifetime reservation.
ACK/log path descriptions are bounded. Settings validation lives in `http/configuration.rs`, keeping
the coordinator at 711 lines; new tests live under the HTTP feature's existing `tests/` directory.

Initial `cargo check --locked -p Chataigne2 --tests` passed in **18.06s**; the initial HTTP run passed
**58/58** after **2m20s** compilation. Adding the mode-None metadata regression exposed a test-fixture
setup error: `create_http_module` deliberately disables transport, so waiting without a configuration
edit timed out (**58 passed / 1 failed**, **1m38s** compilation). Corrected that test to activate a local
configuration before exercising admission; this was not a runtime failure. Corrected HTTP run passed
**59/59** in **0.30s** after **1m25s** compilation. Preservation filters passed **192** distinct tests:
script API 8, concrete streams 77, common streaming 45, Sound Card 35, OSC 25, initial Formula 1,
and null-device Sound Card evidence 1. HTTP plus preservation totals **251 distinct app tests**.
Initial strict Clippy found a now-test-only import and a needless test-helper borrow; corrected both.
Focused verification complete; requested return to **High**, actual effort unchanged/unobservable.
Final `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**24.86s** after the import/borrow cleanup. `cargo test-fast --locked -p golden_io` passed **38/38**
in **0.20s** (7.25s including lock wait); combined with the disjoint app suites, **289 distinct tests**
passed. `cargo check --locked --workspace --tests` passed in **19.76s**. Native
`cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **1m45s** including lock wait;
no linker repair or cache deletion was needed. Root/Core `cargo fmt --all`, explicit HTTP rustfmt,
maintained-doc Prettier, and whitespace checks pass. Inventory is **1622 source files / 42 oversized**;
HTTP settings and configuration tests are 259 and 323 lines. All owned jobs are terminal.

Evidence is the source and checked-in configuration regressions plus terminal commands recorded
here. These Windows/default-feature checks use controlled local HTTP fixtures, native injected
executors, existing stream/OSC loopbacks, missing-serial recovery, and null-device audio fixtures;
they do not qualify live endpoints, hardware, other platforms, native UI, or product-scale timing.
Commit and push this verified milestone on `main`, then continue TCP server/WebSocket/serial and
the broader module/channel/generator audit. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 21: TCP server native ownership

Resumed clean delivered `36e69aeb6d267f08760902eebf13c6dc8ad58bde` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. The app-owned TCP server transport
still performed actor-side resolve/bind, unbounded command admission, join-on-stop, nonblocking
`write_all`, and unbounded client/receive service. This batch moves setup/recovery and socket
cleanup to a bounded native owner, limits commands/replies/clients/service, and gives broadcasts
fixed-cohort per-client cursors without suffix replay. Requested **XHigh; actual effort
unchanged/unobservable**, no supported control exposed.

Source ownership remains under `protocol/stream/tcpserver`: transport admission and event weights
in `transport.rs`, native setup/duplex service in `transport/worker.rs`, broadcast cursors in
`transport/outbound.rs`, and tests under their owning `tests/` directories. The existing shared
`NativeReplies` continuation mechanism handles input/output/status independently. Module admission
also checks applied port/enabled state before using a previous transport, and background
Listening/Recovering events own connected status. The first compile caught an associated-source
visibility mismatch; corrected it to match the crate-visible event contract. Focused tests pending.

The first focused `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
module_modules_protocol_stream_tcpserver -- --test-threads=4` passed **15/15** in **0.95s** after
**2m11s** compilation. Added applied port/disable-before-callback routing regressions, and an owning
capacity-retry deadline check: failed worker admission schedules a 250ms retry instead of stranding
the module or requesting full snapshots every periodic turn. Expanded verification pending.

Expanded owning run passed **17/17** in **1.49s** after **1m54s** compilation. Disjoint compiled-name
preservation suites passed script API **8**, concrete streams **90** (including those 17), common
streaming **45**, HTTP **59**, Sound Card **35**, OSC **25**, initial Formula **1**, and null-device
Sound Card evidence **1**: **264 distinct app tests**, not an added sum of overlapping focused runs.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (3.13s including lock wait),
so **302 distinct app/IO tests** passed. Strict
`cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in **2m03s**
including build-lock wait. Focused verification complete; requested return to **High**, actual effort
unchanged/unobservable.

`cargo check --locked --workspace --tests` passed in **20.57s**. Native
`cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **2m08s** including build-lock wait;
no linker repair or cache deletion was needed. Root/Core `cargo fmt --all`, explicit TCP server
rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory: **1627 source files /
42 oversized**. TCP server module/transport/native worker/outbound cursor are **574/141/279/85**
lines. Tests are separate beside each feature. All owned jobs are terminal.

Evidence is the checked-in native/module admission regressions and terminal commands recorded here.
These Windows/default-feature checks use controlled TCP/HTTP/stream/OSC loopbacks, injected native
setup/cursor failures, missing-serial recovery, and null-device audio fixtures. They do not qualify
external devices/endpoints, other platforms, native UI, or product-scale timing. Commit and push
this milestone on `main`, then continue TCP server owner integration. P03 remains **IN_PROGRESS**;
P04-P18 retain their states.

This native milestone does not complete TCP server integration: passive owner wake/receipt delivery,
worker-prepared per-client grammar/callbacks, and accepted-prefix generation handoff across app
reconfigure/disable remain next. Existing module script functions/callbacks/templates/client folders
remain preservation contracts. Broader WebSocket/serial, module/channel and generator work remains.

### P03 batch 22: TCP server passive input and lifecycle integration

Resumed clean delivered `01158b6dc1c6a1971dc079a97a126543322d8f64` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Server input previously used one
actor parser across all clients, immediate callback JSON construction/publication, and cancellation
on ordinary reconfiguration/disable. This batch gives each native client a parser and generation,
prepares source-tagged values/callbacks/logs on the worker, retains lifecycle callbacks through their
publication receipts, and uses passive module wakes/deadlines. One retiring owner drains accepted
commands/results before a replacement starts; disabled service explicitly opts in for that prefix.
Destroy/restore must discard old runtime deliveries and fence the previous wake source. Requested
**XHigh; actual effort unchanged/unobservable**, no supported control exposed.

Initial compile caught one missed shared-input call site and two log-macro arms needing terminating
semicolons; corrected all three. Initial focused run passed **17/20** after **2m09s** compilation:
native per-client input/grammar/source isolation passed, while three old module fixtures assumed a
single tick completed port replacement. Those fixtures now service authoritative wakes until
configuration/retirement settles. Added passive idle, source-tagged graph input, processing changes,
discarded callback receipt recovery, destroy/undo source fencing, accepted-output reconfiguration,
and disabled input/output drain regressions. Expanded run passed **26/26** in **3.25s** after **1m38s**
compilation. Review then corrected connected status while waiting for a retiring generation,
treated intentional native closure as a clean disconnect, and strengthened exact lifecycle/input
callback counts plus disabled-processing/log-only grammar coverage. Final focused run passed
**27/27** in **3.25s** after **1m37s** compilation. Disjoint preservation suites passed script API
**8**, concrete streams **100** (including those 27), common streaming **45**, HTTP **59**, Sound Card
**35**, OSC **25**, initial Formula **1**, and null-device Sound Card evidence **1**: **274 distinct
app tests**. `cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.17s preparation),
for **312 distinct app/IO tests**. Initial strict Clippy identified a Listening address used only
in test builds after removal of derived equality; the module now reports the real bound address
in its connection-success diagnostic. Focused invariants verified; requested return to **High**,
actual effort unchanged/unobservable. Strict Clippy subsequently passed in **21.31s**, workspace/test
checking in **31.22s**, and native app build in **1m50s**. Final review then found same-endpoint input
updates bypassed the startup mode-capacity guard. Native updates now reject invalid mode storage
before changing the active plan, with a regression preserving its generation, parser and live
connection. Its focused run passed **28/28** in **3.27s** after **1m34s** compilation. Configuration
validation is now shared by the module and native startup, so invalid authored settings use the
configuration-error path rather than the capacity-retry path. Native input updates use the same
input validator. The settled owning run passed **28/28** in **3.24s** after **1m37s** compilation.
Repeating the disjoint preservation set after this last change passed **275 distinct app tests**
(concrete streams now **101**; other counts unchanged). With IO, **313 distinct tests** passed.
Final strict Clippy passed in **1m40s** and workspace/test checking in **1m18s**, both including
build-lock waits. Final native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in
**2m22s** including build-lock wait, without linker repair or cache deletion. Earlier passes remain
labelled above. Root/Core `cargo fmt --all`, explicit changed-feature rustfmt, maintained-doc
Prettier and whitespace checks pass. Inventory is **1630 source files / 42 oversized**. TCP server
module/transport/native worker are **702/214/337** lines; tests remain separate beside each feature.
All owned jobs are terminal.

Evidence is the checked-in native input, callback receipt, passive idle, lifecycle and undo
regressions, plus terminal commands recorded here. Checks use Windows/default features, controlled
stream/HTTP/OSC loopbacks, injected failure/receipt cases, missing-serial recovery, and null-device
audio fixtures. They do not qualify external hardware/endpoints, other platforms, product-scale
timing, or native UI interaction. Commit and push this milestone on `main`, then continue WebSocket/
serial and the broader module/channel/producer/generator audit. P03 remains **IN_PROGRESS**;
P04-P18 retain their states.

### P03 batch 23: WebSocket client native and passive ownership

Resumed clean delivered `5a2cb36842f11ca9a2b3e1bf3eece857acb2d1b1` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. WebSocket client resolution, unbounded
commands, join-on-stop and unbounded send/flush lived outside the reusable native boundary; its
owner also parsed/constructed callback JSON on a 120 Hz periodic path. This batch moves those
responsibilities to bounded native ownership and passive retained owner delivery. Requested
**XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

The existing locked Tungstenite 0.29 implementation owns framing, masking, HTTP handshake and
control messages through its public nonblocking API. The app's `BudgetSocket` bounds per-turn
read/write bytes and syscall counts, including library fragment/flush loops. Native lifetimes,
command/reply queues, frame sizes, write buffers and service turns are bounded. Frame cursors submit
once, then flush retained encoded bytes; connection generations reject the accepted stale suffix
without moving effects to a replacement. Setup polls both cancellation and canonical admission
closure, with bounded connect/handshake/control deadlines and reconnect backoff. Local invalid UTF-8
fails one frame without reconnecting. New input envelopes retain frame kind for text/binary callback
semantics; shared input validation replaces the server-only validator.

The app owner now reserves `ModuleWake`, uses passive receipt-driven input/graph service, captures
applied host/port/path before admission, and retains one old producer through accepted-frame drain.
Disabled service explicitly finishes that prefix; destroy/undo discards runtime input and fences
the previous wake source. Script methods, commands and app-owned snippets retain their contracts.
Shared stream envelope rejection now invalidates partial parser state, including callback admission
failure, so later bytes cannot join across a dropped preparation boundary. WebSocket mailbox
oversize feedback also invalidates that source's partial frame.

`cargo check --locked -p Chataigne2 --tests` passed in **21.05s** with the temporary empty test-module
import warning. The initial focused run passed **15/15** in **0.42s** after **2m13s** compilation.
The expanded run passed **20/21** after **1m35s** compilation: a Windows test peer inherited its
listener's nonblocking mode; made that fixture explicitly blocking. Added callback-complexity gap
recovery, oversized wire rejection, unavailable-endpoint recovery, and partial-frame reconnect
coverage. The next focused run passed **25/25** in **0.89s** after **1m26s** compilation. After adding
the mailbox gap guard and independent syscall-count assertions, **25/25** passed again in **0.89s**
after **1m23s** compilation; these overlapping runs are not summed.

Strict Clippy found the HTTP test callback closure inferred the library's fixed large error type.
The fixture now implements Tungstenite's public `Callback` trait, matching the library boundary
without an allowance. App fixtures now materialize the module disabled, configure their ephemeral
endpoint, then enable it, avoiding default-endpoint contact. Requested return to **High** after focused
invariants passed; actual effort unchanged/unobservable. Final focused/integration/lint/native
validation remains at that intermediate point. The settled focused run passed **25/25** in **0.79s**
after **1m27s** compilation. Disjoint compiled-name preservation suites passed script API **8**,
concrete streams **125** (including those 25), common streaming **45**, HTTP **59**, Sound Card **35**,
OSC **25**, initial Formula **1**, and null-device Sound Card evidence **1**: **299 distinct app tests**.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.13s preparation), for
**337 distinct app/IO tests**. Repeated and overlapping profiles are not added twice.

Final `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**1m38s** including build-lock wait; `cargo check --locked --workspace --tests` passed in **17.91s**.
Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **1m31s**, without linker
repair or cache deletion. Root/Core `cargo fmt --all`, explicit changed-feature rustfmt,
maintained-doc Prettier and whitespace checks pass. Inventory: **1638 source files / 42 oversized**.
WebSocket owner/transport/native worker/socket/cursor are **590/194/309/119/93** lines; tests are
separate beside each feature. All owned jobs are terminal.

Evidence is the checked-in transport, cursor, passive module and shared input regressions plus
terminal commands recorded here. These Windows/default-feature checks use controlled WebSocket/
TCP/UDP/HTTP/OSC loopbacks, injected native/receipt failures, missing-serial recovery, and null-device
audio. They do not qualify external endpoints/hardware, other platforms, native UI interaction or
product-scale timing. Commit and push this milestone on `main`, then continue WebSocket server,
serial, shared producer argument admission/disposal, Sound Card/channel and generator audits.
P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 24: WebSocket server bounded native ownership

Resumed clean delivered `e9230f86f75fe1a15493192f56563f46f8f18757` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Reconfirmed that the mapping branch
is already merged and the plan requires each verified milestone to commit/push to `origin/main`.
The next owning layer was app WebSocket server transport: actor-side resolution/bind, unbounded
commands/replies, sequential three-second handshakes, unbounded async send/flush and join-on-stop.
Requested **XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

The app now uses the locked Tungstenite 0.29 public nonblocking server handshake/framing API.
Shared app WebSocket code owns the finite codec configuration and socket byte/syscall budget;
client connection setup stays client-owned. The obsolete copying message helper/private client
socket aliases are removed. The server reserves at most 16 live/retiring native workers, with
256 commands / 8 MiB, a 1 MiB retained command bound before copying, 64 replies / 32 MiB and one
retained continuation per input/output/status producer. At most 128 active peers and pending
handshakes share capacity. Each native turn advances at most four accepts, interrupted handshakes,
output recipients and input messages; each socket direction has 64 KiB/four-syscall limits.
Handshake deadline is three seconds; idle HTTP peers cannot block valid upgrades or output.
Exact and wildcard paths retain existing semantics with bounded authored/request path storage.

Broadcasts capture native peer identities when each command begins and share message storage.
Each recipient keeps independent submit/flush state and a five-second deadline, rotating fairly;
WouldBlock never resubmits the encoded frame. Failures reject the original suffix explicitly
without moving it to replacements, including an identical public label. Invalid UTF-8 fails one
broadcast without reconnecting. Input and output reply pressure have independent continuations.
Ping/close remain serviced with application receive disabled. Successful server close flush is
normal disconnection, following the library's documented ConnectionClosed result.

Native cancellation/drop never joins the engine. Blocked setup keeps its reserved lifetime;
bind retries back off 250ms–5s. Native Listening/Recovering events own connection feedback.
Applied port/path/enabled values fence script/command admission before callbacks. Temporary native
capacity exhaustion retries after a 250ms owner deadline and defers snapshots until due. The owner
still uses periodic actor-side callback/parsing with a shared parser and explicitly cancels on
ordinary disable/reconfigure. Passive receipts, per-peer prepared input and accepted-prefix
retirement are the next batch; this milestone does not claim those gaps fixed.

The initial compile found overlapping mutable borrows in native handshake setup; staged the
library call before advancing state. The initial focused run passed **17/17** in **0.57s** after
**2m14s** compilation; expanded ownership/admission coverage passed **21/21** in **0.60s** after
**1m23s** compilation. The next run passed **22/23** after **1m40s** compilation: the occupied-port
app fixture bound loopback while the module binds wildcard, permitting a different bind outcome
on Windows. It now binds the same wildcard address and waits for the explicit warning before
asserting disconnected state. Strict Clippy rejected the large native handshake outcome enum;
boxed the established socket during native handoff. The corrected run passed **23/23** in
**0.42s** after **1m49s** compilation. Added explicit normal-close handling and strengthened that
test; the settled source passed **23/23** in **0.42s** after **1m39s** compilation. Overlapping runs
are not summed. Requested return to **High** after focused proof; actual effort unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **147**
(including those 23), common streaming **45**, HTTP **59**, Sound Card **35**, OSC **25**,
initial Formula **1**, and null-device Sound Card evidence **1**: **321 distinct app tests**.
These run the built `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with the recorded owning
filters and `--test-threads=4`; null-device evidence uses its exact name and one thread.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.14s preparation):
**359 distinct app/IO tests** total.

Final `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**1m45s** including lock wait. `cargo check --locked --workspace --tests` passed in **1m48s**
including lock wait. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in
**1m29s** including lock wait, without linker repair or cache deletion. Root/Core Cargo formatting,
explicit changed-feature rustfmt, maintained-doc Prettier and whitespace checks pass.
Inventory: **1649 source files / 42 oversized**; all new runtime/test files remain below 1000 lines.
All owned jobs are terminal.

Evidence is the checked-in native transport/cursor/peer/pressure and owner admission regressions,
plus terminal commands recorded here. Checks use Windows/default features, controlled local
WebSocket/TCP/UDP/HTTP/OSC loopbacks, injected setup/queue/receipt failures, missing-serial recovery
and null-device audio. They do not qualify external endpoints/hardware, other platforms,
native UI interaction or product-scale memory/timing. Commit and push this milestone on `main`,
then continue server passive/per-peer input/retirement, serial, shared producer argument
admission/disposal, Sound Card/channel and generator audits. P03 remains **IN_PROGRESS**;
P04-P18 retain their states.

### P03 batch 25: WebSocket server passive input, retirement and projection ownership

Resumed clean delivered `b991a9ea13ac2cc777951d90df3a1acc0fe4ee5f` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. The native server from batch 24
still handed raw frames to a shared actor parser and immediate callbacks on a 120 Hz periodic
path; ordinary disable/reconfigure cancelled accepted output. This batch completes its app-owned
native/passive boundary through existing public IO/engine and common stream contracts.
Requested **XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

Each native peer owns a parser and input generation. Grammar/processing changes invalidate that
peer's partial frame; logging-only changes preserve it. Prepared input retains graph values,
source-tagged text/binary callbacks, bounded logs and rejection counts. Preparation/complexity or
mailbox-envelope rejection invalidates only that peer's parser; another peer's partial line survives.
Connection/disconnection callbacks use canonical native payload reservations; disconnect reports
abandoned partial lines. Validated input updates reuse existing listeners/connections. Obsolete
immediate server lifecycle callback helpers and unused constants are removed.

The owner reserves a wake before native work and uses passive callback receipt/graph service,
producer exit notifications and coarse retry deadlines. One bounded native drain stays owned
through its callbacks; discarded publication retries only that callback. Ordinary reconfigure and
disable close admission, retain one old producer through its accepted frame/input/callback prefix,
and wait for producer disconnection before replacement. Disabled wakes finish that prefix without
restoring retired clients to the live projection. Destroy cancels producers, discards runtime input
and releases the wake; undo reserves a fresh source. Applied port/path/disable admission fencing,
handshake/peer limits, original-peer frame cursors and independent reply-pressure checks remain.

Review reproduced a projection failure: after discarding an edit batch, the runtime map retained
a client but its public client node/count never recovered without another peer event. The focused
`discarded_client_projection_reconciles` test failed **0/1** after **1m34s** compilation. A focused
`feedback.rs` now retains desired connection/warning state and a separate projection receipt.
Failure reconciles against a fresh snapshot, preserving committed child IDs rather than replaying
structural edits or trusting optimistic parameter-handle caches. Staging explicitly wakes service.
This adds no callback/frame replay. TCP Server has the analogous source pattern; its owning
failure reproduction/publication audit is the next P03 task, separate from historical passing
input/native/retirement evidence.

Initial `cargo check --locked -p Chataigne2 --tests` passed in **21.63s** with four obsolete callback
helper warnings, then those helpers were removed. Initial focused coverage passed **34/34** in
**0.81s** after **2m21s** compilation. Expanded per-peer envelope-gap/configuration-capacity coverage
passed **35/35** in **0.99s** after **1m31s** compilation; strict IO/app Clippy passed in **1m49s**.
Projection review then produced the separate failing regression above. The mechanical feedback
split's check caught helper visibility and an unused import; scoped helpers to their parent and
removed the import. Its first run passed **35/36** after **2m00s** compilation, with the projection
regression timing out because staging had not requested its own wake. Added that wake and partial
prefix/warning-set/clear coverage. Settled coverage passed **38/38** in **0.99s** after **1m59s**
compilation. Overlapping runs are not summed. Requested return to **High** after focused proof;
actual effort unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **162**
(including those 38), common streaming **45**, HTTP **59**, Sound Card **35**, OSC **25**,
initial Formula **1**, and null-device Sound Card evidence **1**: **336 distinct app tests**.
They run `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those owning filters and
`--test-threads=4`; null-device evidence uses its exact name and one thread.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.13s preparation), for
**374 distinct app/IO tests** total.

Final `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**2m07s** including lock wait. `cargo check --locked --workspace --tests` passed in **22.85s**
including lock wait. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in
**0.68s** with warm artifacts, without linker repair or cache deletion. Root/Core Cargo formatting,
explicit changed-feature rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory:
**1655 source files / 42 oversized**. Owner/feedback/transport/peer/worker are
**701/141/208/204/237** lines; all tests remain separate beside their feature. All owned jobs are terminal.

Evidence is the checked-in native input, passive delivery, projection and retirement regressions,
plus terminal commands recorded here. Checks use Windows/default features, controlled local
WebSocket/TCP/UDP/HTTP/OSC loopbacks, injected queue/edit/receipt failures, missing-serial recovery
and null-device audio. They do not qualify external hardware/endpoints, other platforms,
native UI interaction or product-scale memory/timing. Commit and push this milestone on `main`,
then continue TCP/other-owner projection publication, serial, shared producer admission/disposal,
Sound Card/channel and generator audits. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 26: TCP and shared server projection receipts

Resumed clean delivered `86f380c80eaf710dd764a5c9749004ea175b2b8f` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. The next owning layer was TCP
Server's analogous connection/client projection path. It cleared dirty state before the engine
applied its edits and trusted optimistic parameter caches. The focused
`discarded_tcp_client_projection_reconciles` regression failed **0/1** in **0.01s** after
**1m37s** compilation: the runtime retained its client but the public client node never recovered.
Requested **XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

App `common/streaming/server_feedback.rs` now owns one desired connection/warning state and one
in-flight projection receipt, shared by TCP and WebSocket Server. Small protocol adapters supply
target/warning IDs and client-parameter factories, preserving their existing metadata and public
boundaries. A discarded/partial projection reconciles against a fresh snapshot without replaying
structural edits, losing committed child identities or trusting optimistic handle values. Newer
desired state remains dirty while the previous projection is pending, then replaces its displayed
state after that receipt settles. Projection staging explicitly wakes service. Callback, graph
input and native frame delivery stay independent; projection retries do not replay those effects.
TCP now includes feedback receipts in passive snapshot/wake, disabled completion and destroy/undo
ownership. The duplicated immediate projection implementation is removed; the verified WebSocket
implementation is factored into the shared app component rather than copied into TCP.

`cargo check --locked -p Chataigne2 --tests` passed in **13.47s**. The corrective focused TCP run
passed **32/32** in **2.64s** after **1m57s** compilation. The compiled WebSocket consumer passed
**39/39** in **0.83s**. These include discarded/partial projection recovery, stable client IDs,
warning set/clear recovery, one in-flight projection and retention of newer desired connection,
warning and client state. Existing input, native cursor, pressure, admission, retirement and undo
regressions also pass. Requested return to **High** after focused proof;
actual effort unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **167**
(including those 71 server tests), common streaming **45**, HTTP **59**, Sound Card **35**, OSC **25**,
initial Formula **1**, and null-device Sound Card evidence **1**: **341 distinct app tests**.
They run `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those owning filters and
`--test-threads=4`; null-device evidence uses its exact name and one thread.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.14s preparation), for
**379 distinct app/IO tests** total. Overlapping focused/preservation runs are not summed twice.

`cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in **1m07s**
including lock wait. `cargo check --locked --workspace --tests` passed in **13.01s** including lock
wait. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **0.68s** with warm
artifacts, without linker repair or cache deletion. Root/Core Cargo formatting, explicit changed
feature rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory: **1658 source files /
42 oversized**. Shared feedback is **142** lines, each protocol adapter **40**; TCP/WebSocket owners
are **663/700** lines. Tests remain separate beside their feature. All owned jobs are terminal.

Evidence is the checked-in owning TCP/WebSocket feedback regressions and preserved native/input/
lifecycle suites, plus terminal commands recorded here. Checks use Windows/default features,
controlled local protocol loopbacks, injected queue/edit/receipt failures, missing-serial recovery
and null-device audio. They do not qualify external hardware/endpoints, other platforms, native UI
interaction or product-scale timing/memory. Commit and push this milestone on `main`, then continue
serial, shared producer admission/disposal, Sound Card/channel and generator audits. P03 remains
**IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 27: Serial native connection and discovery ownership

Resumed clean delivered `d2e8764c1d4a4119fccc06f1b5a198c8652edf44` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Serial connection lived in the
shared app serial file with unbounded command admission, join-on-stop, whole-buffer writes,
silent full-result drops and no original-connection fence for queued output. Discovery also joined
the actor behind enumeration. Requested **XHigh; actual effort unchanged/unobservable**, with no
supported control exposed. The app common layer now separates connection, discovery and metadata
through a conventional facade; no shell/Golden-private boundary or persisted schema changes.

The locked serialport 4.9 public API still owns native device handling. Connection work reserves
at most 16 live/retiring workers, with 256 commands / 256 KiB queued, 256 KiB retained per command
checked before copying, 64 replies / 2 MiB, and one retained input/output/status continuation.
Each turn services at most four command operations; write slices and read buffers are at most
8 KiB. Native calls request 20ms I/O timeouts, while blocked open/write/cleanup retains its slot
rather than joining the actor or allowing unlimited replacement threads. Configuration bounds
port retained capacity to 1024 bytes without NUL and rejects zero baud before reserving a worker.
DTR/RTS setup failures now enter recovery before publishing connected.

Write cursors keep confirmed offsets on the original connection with a five-second active-command
deadline. WouldBlock/Interrupted preserve the suffix. The pinned Windows COMPort implementation
returns WriteFile errors without exposing its byte count, so a timeout may hide partial effects:
these errors report an unconfirmed outcome and never retry the original frame. Generation tags
reject the queued old suffix after reconnect; reopen backs off 250ms?5s. Input/output pressure
retains FIFO results independently instead of silently dropping them. Native graceful admission
closure drains the admitted output prefix; explicit cancellation may abandon it. Cancellation is
checked before each additional operation after a blocked call returns.

Discovery uses one live/retiring bounded worker. Stop/last-unsubscribe cancels without joining;
blocked enumeration keeps its slot and cancelled/ownerless results cannot restore the catalog.
Enumeration errors preserve the last good list and recover on later scans. Registration identity
exhaustion rejects instead of reusing an identity. Temporary connection/discovery capacity retries
use coarse owner deadlines. Applied port, baud, DTR, RTS and enabled snapshot values fence new
script/command admission before callbacks. Outgoing logs describe queue admission and cap their
byte prefix. Test-only injected providers exercise these contracts without opening hardware.

Initial `cargo check --locked -p Chataigne2 --tests` passed in **21.31s** with temporary scaffold
warnings, subsequently removed. The first native-test compile caught an unnecessary Debug
requirement from `unwrap_err`; the fixture now inspects the error without adding production traits.
The next serial-filtered run passed **28/28** in **1.05s** after **2m13s** compilation. Expanded
nonjoining discovery/enumeration recovery coverage passed **30/30** in **2.26s** after **2m07s**
compilation. That broad name filter includes three unrelated serialization matches; they are not
summed into serial ownership proof. The exact owning suites contain **16 common serial tests** and
**11 serial-module tests**. Requested return to **High** after focused proof; actual effort remains
unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **170**,
common streaming **45**, common serial **16**, HTTP **59**, Sound Card **35**, OSC **25**,
initial Formula **1**, and null-device Sound Card evidence **1**: **360 distinct app tests**.
They run `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those owning filters and
`--test-threads=4`; null-device evidence uses its exact name and one thread.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (4.29s preparation/lock wait),
for **398 distinct app/IO preservation tests**. Overlapping focused runs are not counted twice.

`cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in **1m39s**
including lock wait. `cargo check --locked --workspace --tests` passed in **22.70s** including lock
wait. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **0.66s** with warm
artifacts, without linker repair or cache deletion. Root/Core Cargo formatting, explicit serial
feature rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory: **1669 source files /
42 oversized**. Serial facade/metadata/discovery/connection/worker/cursor are
**16/229/258/158/188/59** lines; module owner **585**. Runtime/tests are separate. All owned jobs are terminal.

The module still uses periodic actor-side discovery/input/callback processing and cancels on
ordinary disable/reconfigure. Bounded retained catalog/registration storage, options preparation/
disposal, producer-exit notifications/recovery, native input generations, passive receipts and
accepted-prefix retirement are the next serial work. These checks do not qualify main-thread
catalog budgets, physical line timing, external devices, other platforms, native UI interaction
or product-scale throughput/memory. Evidence is checked-in native cursor/pressure/lifecycle,
discovery and applied-admission regressions plus terminal commands above. Commit and push this
milestone on `main`, then continue passive serial and the broader producer/module/Sound Card/
generator audits. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 28: Serial retained catalog and passive service ownership

Resumed clean delivered `2066f735b44d6d32059ab100fb24420ee321baf8` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Requested **XHigh** for catalog/
subscription generations, producer-exit recovery, native input generations, receipt delivery and
accepted-prefix retirement; actual effort unchanged/unobservable, with no supported control exposed.
Ownership remains in app common serial/stream components and the concrete serial module.

Discovery now admits at most **1024 subscribers**, one live/retiring producer, **128 retained ports**
and **2 MiB** including prepared enum options. It bounds retained name/label/details/tag storage,
prepares and compares catalogs on the native producer, and publishes a header plus canonical retained
lease. Identical catalogs reuse the lease; failed enumeration or catalog admission retains the last
good options. Final payload disposal uses the shared bounded worker disposer. Subscriber callbacks
run outside the catalog lock, including startup-failure/producer-exit paths. Last unsubscribe
invalidates the epoch and cancels without joining; blocked old enumeration retains the single slot
and cannot restore a new subscription's state. Producer exit notifies owners and restarts through a
one-second deadline, avoiding failure-driven worker churn.

The connection worker now owns parsing, canonical byte/text callbacks and bounded incoming logs.
Input/connection generations discard partial frames across reconnect, grammar and processing changes,
including disable/re-enable without intervening bytes. Reply storage is **64 items / 32 MiB** for
prepared envelopes, with the existing independent input/output/status continuations. Output cursors,
original-device generation fencing, timeout-as-unknown-effect policy and native slot limits survive.
No protocol implementation, schema, Golden/app boundary or shell bootstrap changes were needed.

The module is **passive**: worker replies, receipts, configuration/metadata edits and coarse retry
deadlines drive service. A bounded native drain waits behind callback receipts; canonical delivery
retries only discarded publication. Ordinary disable/reconfigure closes admission and retains one
old worker until accepted output and pending input complete, including while disabled. Retiring
status cannot restore connected; destroy cancels and clears deliveries/subscriptions, and undo
reserves a fresh wake source. One isolated receipt owns each connected/warning/catalog projection;
only successful completion acknowledges its captured catalog/selection generation. Failed/partial
edits reconcile against a fresh snapshot and newer desired state remains pending. Queued enum
mutation retains the catalog lease rather than copying its option tree before application.
The now-unused actor parser state and immediate byte-callback helper were removed from shared
streaming code after all concrete streams moved to native preparation.

Initial app/test compilation caught a fixture NodeId constructor and consumption of the final reply
continuation; the corrected check passed in **24.12s**, with obsolete actor-helper warnings then
removed. The first expanded run passed **38/41** matched tests after **2m20s** compilation: failures
were two old polling/synchronous-retirement fixture assumptions and a fixture waiting for status
instead of admitted input. Fixtures now wait for the native prefix and asynchronous owner state.
Additional disabled-binary, callback-failure/undo and partial feedback coverage was added. The final
serial-filtered run passed **44/44** in **2.26s** after **2m04s** compilation. Three matches are unrelated
serialization tests; the owning suites contain **24 common serial** and **17 serial-module** tests.
Requested return to **High** after focused proof; actual effort remains unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **176**,
common streaming **45**, common serial **24**, HTTP **59**, Sound Card **35**, OSC **25**, initial
Formula **1**, and null-device Sound Card evidence **1**: **374 distinct app tests**. The command
runs `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with these owning filters and
`--test-threads=4`, checking compiled-name sets for overlap before summing.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.13s preparation):
**412 distinct app/IO preservation tests**. Overlapping focused runs are not counted twice.

Strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**29.43s**; `cargo check --locked --workspace --tests` passed in **23.38s**. Native
`cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **0.49s** with warm artifacts,
without linker repair or cache deletion. Root/Core Cargo formatting, explicit serial/shared-stream
rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory: **1676 source files /
42 oversized**. Serial connection/discovery/metadata/native-worker/catalog/discovery-worker are
**190/277/235/225/62/145** lines; module owner/projection are **589/121**. Runtime/tests stay separate.
All owned jobs are terminal. Fetched `origin/main` still matches the parent with no divergence.

These checks use deterministic injected ports/providers, missing serial endpoints and null audio.
They do not qualify physical device/line timing, other platforms, native UI interaction, or
product-scale latency/memory. Evidence is the checked-in catalog/subscription/native-input/passive
lifecycle/receipt regressions and terminal commands above. Commit and push this verified milestone
on `main`, then continue shared stream/HTTP argument and disposal admission, Sound Card/module/
channel ownership and generator elapsed-time accounting. P03 remains **IN_PROGRESS**; P04-P18
retain their states.

### P03 batch 29: Native outgoing completion logging

Resumed clean delivered `2c24b1197902232505d2fe75ba6966ea39854124` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Source audit confirmed shared
stream script/command helpers still clone unbounded arguments and recursively encode value trees
before native admission. That boundary remains open. This batch removes the remaining actor-side
outgoing byte-log formatting from Serial, TCP Server and WebSocket Server, completing native log
preparation alongside their existing cursor ownership. No Golden/app or persistence/protocol
boundary changes were needed.

Requested **XHigh** for coupled cursor/recipient outcomes, bounded reply continuations and accepted
retirement; actual effort unchanged/unobservable, with no supported control exposed. The three native
send APIs now capture logging policy explicitly with each admitted command. TCP's typed command
weight includes that policy, and a cheap closed-admission preflight rejects before copying a payload.
Size limits, queue limits, original-recipient ownership, write deadlines and no-replay behavior survive.

App common `streaming/outgoing.rs` formats at most **1024 payload bytes** on the native owner.
Serial publishes success only after its write cursor completes. Broadcast recipients now use typed
**Pending / Confirmed / Failed** states; one completion summary reports only confirmed members of
the original native cohort. A failed recipient or submitted-but-unflushed WebSocket frame cannot
count as confirmed. All-failed commands emit their existing explicit failures without a success log.
Messages describe native send completion rather than application acknowledgement by remote peers.

One output continuation retains a completed command's bounded log behind a full mailbox, pausing
further output while independent input still progresses. Ordinary retirement drains every admitted
completion result; explicit cancellation may abandon them. The actor only forwards the prepared
message, including while servicing a retiring worker. It no longer inspects/formats outgoing bytes
for these protocols. Admission/traffic feedback continues to say queued; completed output says sent.
TCP/WebSocket Client and UDP already owned their outgoing log preparation natively and are preserved.

The initial app/test check found two old test call sites missing the new explicit policy argument.
The first completion-test compile caught a missing fixture drain-state import; corrected source then
passed **11/11** completion-matched tests in **0.41s**, after **3m00s** preparation including a build
lock wait behind the remaining binary job. Six are newly added owning completion tests; the seventh
new test rejects serial timeout-as-success and runs in the common serial suite. Process handles were
observed through terminal state; observation timeouts did not trigger process replacement.

Owning suites pass common serial **27**, serial module **17**, TCP Server **34** and WebSocket
Server **41**: **119 distinct tests**. New regressions exercise confirmed recipient subsets,
partial/unknown effects, WebSocket flush ownership, silent logging policy, bounded payload prefixes,
full completion mailboxes, independent input, FIFO logs and accepted-prefix retirement without replay.
After final review added TCP's closed-admission preflight, its **34/34** owning suite passed again in
**3.14s**, following **2m28s** preparation. Requested return to **High** after focused proof; actual
effort remains unchanged/unobservable.

Disjoint compiled-name preservation filters passed script API **8**, concrete streams **180**,
common streaming **45**, common serial **27**, HTTP **59**, Sound Card **35**, OSC **25**, initial
Formula **1**, and null-device Sound Card evidence **1**: **381 distinct app tests**. They run
`target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those filters and `--test-threads=4`,
checking compiled-name sets for overlap before summing. Only the final TCP preflight changed after
that pass, covered by the final owning rerun above. `cargo test-fast --locked -p golden_io` passed
**38/38** in **0.20s**, with **2m18s** preparation/lock wait: **419 distinct app/IO tests**.
Overlapping focused runs are not counted twice.

Final strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed
in **12.17s** (earlier full pass **30.02s**). Final `cargo check --locked --workspace --tests` passed
in **7.53s** (earlier pass **22.68s**). Native `cargo build --locked -p Chataigne2 --bin Chataigne2`
passed in **0.49s** with warm artifacts, without cache deletion or linker repair. Root/Core Cargo
formatting, explicit affected-feature rustfmt, maintained-doc Prettier and whitespace checks pass.
Inventory: **1680 source files / 42 oversized**. Shared outgoing helper is **18** lines; Serial/TCP
Server/WebSocket Server owners are **585/654/691**. Their native cursor modules are **70/136/139**;
runtime/tests stay separate. All owned jobs are terminal. Fetched `origin/main` has no divergence.

Checks used deterministic injected serial ports and controlled loopback. They do not qualify physical
device/line timing, other platforms, native UI, or product-scale latency/memory. Evidence is checked-in
native cursor/logging/mailbox/retirement regressions and the commands above. Commit and push this
verified milestone on `main`. Next, repair shared stream command capture/materialization/disposal
(`common/streaming/commands` and `modules/protocol/stream/commands`), including unbounded script
string conversions, recursive value collection/JSON creation and command-envelope ownership before
native admission. Then continue HTTP argument conversion/disposal, Sound Card/module/channel
ownership and generator elapsed-time accounting. P03 stays **IN_PROGRESS**; P04-P18 retain their
states.
