# Implementation status

This is the only current execution ledger for the
[implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md). When asked to
“implement the plan”, start there and resume from this ledger. Historical ledgers and their
pause/branch/push instructions are [archived](../archive/2026-10-18/README.md).

Last reconciled: 2026-10-19. Source baseline: `49d65b0fe71f66b73d338ebe026a572a53c7dcf6`,
branch `codex/builtin-mapping`. At audit start the only local modification was the user's main
plan. Its complete contents, including the uncommitted clarification, survive in the
[previous plan](../archive/2026-10-18/plan/Chataigne2_Codex_Implementation_Plan.md).

## Current handoff

- Current batch: P03 batch 7 bounded UDP native worker/generation ownership verified locally.
- Delivery: the user authorized milestone commits/pushes and requested merging
  `codex/builtin-mapping` into `main` on 2026-10-19. Commit the verified P01/P02/P03-through-batch-7
  milestone, preserve unfinished shared-stream baseline work outside that commit, and continue
  on `main`. Earlier no-commit/no-push notes below describe historical actions only.
- P00 resumed check: same HEAD and prior verified P01/P02 diff; no unrelated edits or live jobs found at resume.
- Scope selected by the user: **foundation and readiness**. Full lighting/video/projection
  authoring is deferred; current features and completed Mapping remain preservation contracts.
- Runtime/source implementation under the new P tasks: **P01 and P02 verified locally; P03 in progress**.
- Next implementation action: continue P03 at shared stream parser/retained-byte/service/receipt
  bounds, moving UDP parsing/interface discovery off the node and replacing its 120 Hz service
  with worker notifications. Continue the full module/channel audit (including Sound Card
  stop/join, event-drain, admission/recovery, and configuration bounds) and generator time accounting.
  Preserve the lifecycle/replacement, bounded wake, and HTTP graph/callback failure regressions.
  Preserve the accumulated P01 repair and its regressions.
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

| Task                        | Implementation | Local validation   | External qualification / evidence                                                                             |
| --------------------------- | -------------- | ------------------ | ------------------------------------------------------------------------------------------------------------- |
| P00 Baseline                | VERIFIED_LOCAL | PASSED             | HEAD `2e1f392551ee6d4b9a5859bc3c62945e18811fe4`; clean after consolidation committed during reconciliation    |
| P01 Edit failures/history   | VERIFIED_LOCAL | PASSED batches 1–8 | Applied-prefix/atomic/history rejection, callback/teardown preservation, and next edit/tick/save verified     |
| P02 Schedule/actor failures | VERIFIED_LOCAL | PASSED batches 1-3 | Dependency order, generation fencing, typed errors, disconnect history, and later-save recovery verified      |
| P03 Bounded I/O/lifecycle   | IN_PROGRESS    | PASSED batches 1-7 | Worker/lifecycle/HTTP/UDP native ownership verified; shared stream, module, and generator service bounds open |
| P04 Audio order/types       | NOT_STARTED    | PENDING            | Legal interleaving and invalid deserialization established from source                                        |
| P05 Transport trust         | NOT_STARTED    | PENDING            | Missing admission verified; exploit not demonstrated                                                          |
| P06 Dependencies            | NOT_STARTED    | FAILED baseline    | Four Rustls advisories on audit date; refresh DB before repair                                                |
| P07 DMX streaming           | NOT_STARTED    | PENDING            | Refresh/filter/service gaps; protocol loopback qualification needed                                           |
| P08 Formula backend intents | NOT_STARTED    | PENDING            | UI polling/identity and mutation policy verified; corruption not reproduced                                   |
| P09 Graph document          | NOT_STARTED    | PENDING            | Ordered-map mutation costs source-derived                                                                     |
| P10 Canvas                  | NOT_STARTED    | PENDING            | Projection diagnostic and crossing-edge reproduction; mounted performance open                                |
| P11 Workspace UX            | NOT_STARTED    | PENDING            | Collection/keyboard gaps source-reviewed; native usability open                                               |
| P12 Boundaries/gates        | NOT_STARTED    | PENDING            | App policy leak, omitted graph tests, oversized files, unsafe protocol conversions                            |
| P13 Formula execution       | NOT_STARTED    | PENDING            | Historical profiles retained; production parallelism not mandated                                             |
| P14 Production time         | NOT_STARTED    | PENDING            | Clock helper unwired, no target-frame play, starvation/latency policy gaps                                    |
| P15 Assets                  | NOT_STARTED    | PENDING            | Audio path/cache identity exists; project asset boundary unimplemented                                        |
| P16 Product scale           | NOT_STARTED    | PENDING            | Historical full-product gates remain open; 20k/50k/mounted canvas required                                    |
| P17 Platforms/artifacts     | NOT_STARTED    | PENDING            | Windows history retained; exact current app/native/platform checks required                                   |
| P18 Named devices           | NOT_STARTED    | PENDING            | Exact authorized ASIO4ALL/VB-Audio route must be verified first                                               |

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
