# P11 dashboard session barriers

The reusable session drains managed edits before project changes, save and history actions.
This closes the normal applied-receipt ordering gap between gesture queues and transport dispatch.
Dashboard acceptance still requires the fixed action/timing matrix.

## Ownership

`store/session/edit-operations.ts` owns `runBarrier`: close new managed admission synchronously,
cancel unconfirmed work, drain confirmed samples/history, then enter one serialized callback.
All prefixes settle before a failure propagates. Session-generation changes prevent waiting callback
entry. Successful drafts may remain through their graph-publication boundary; presentation-only
delay does not hold completed backend work. The existing 64-edit-record bound remains. No polling.
Callbacks perform the final operation without recursively entering a barrier or opening a gesture.

`WorkbenchSession.runEditBarrier` also waits for snapshot synchronization and raw intent drainage,
then validates the snapshot generation. Pipeline flush preserves queued requests and refuses a
synchronization/recovery gate. Project replacement drains managed edits before acquiring raw dispatch
suspension; acquiring suspension earlier could reject a confirmed tail or prevent history closure.

Undo/redo evaluate availability after preceding edits close. Their public UI entrypoints report
failure through the existing UI log, protecting callers that discard the promise. Save uses the same
barrier, capturing its history identity before the request. Later edits remain marked unsaved.
Captured host/snapshot identities guard global project path/clean-state updates. Existing-path save
uses the actual project-file workflow; native dialogs remain separate. The touched project-file
module was normalized with the repository formatter. Rust and generated protocol are unchanged.

## Evidence

The [114-group contract](p11-dashboard-barrier-contract.md) covers active/confirmed transforms,
atomic ordered moves, save, undo/redo, project/reparent ordering, multiple owners, late publication,
rejection, snapshot recovery and host replacement. Scenes retain 600 source widgets and three
anchors, using real session/component operations with controlled receipts, history and graph events.

The [regression evidence](evidence/p11-dashboard-barrier-regressions-2026-10-25.json) retains an old-save
probe: the current harness loads only the project-file module from `85c34e6f`, recording its override
hash. The first pending-prefix assertion fails because save enters early. Six initial service tests
exposed the absent barrier contract. Two later tests reproduced retirement rejection from public
undo/redo; those entrypoints now resolve after reporting failure.

The [final full evidence](evidence/p11-dashboard-barriers-2026-10-25.json) passes **599 correctness
groups / five sparse tiers** through 100k authored nodes, including all 485 preceding groups. It
records 136 source fingerprints, production asset hashes and no page errors. Root UI passes **369**: audio 27 / graph
52 / Golden UI 173 / app 117. Ten added unit cases cover lifecycle/retirement/failure, transport
drainage/recovery and history error handling. TypeScript and browser/app Svelte checks pass.
The final locked default build rebuilt the frontend and passed in **2m13s**, as did the preceding
build before history-error handling. Executable SHA-256:
`74040ccaf1ce633afb09e5d4110575a30282514e2ddd4638799439e96e99f47a`.
Source/report formatting, whitespace and both Cargo formatters pass. Inventory is **2,168 source
files / 27 oversized**; touched runtime/tests stay below 1,000 lines with no new exception.

Host replacement verifies retirement and absence of dispatch into the new host. Old-connection
history cleanup is explicitly simulated separately. Backend undo/codec and native disconnect/input
semantics retain independent evidence/owners. Project recovery uses the real transition/snapshot
controllers with a controlled provider. This run establishes no action-latency distribution.

## Remaining acceptance

Execute the fixed three-warmup / 31-sample dashboard action matrix at 1k/10k/20k/50k/100k authored
nodes, including exact dense picking, edits, navigation and framing. Preserve geometry, mounting
and timing gates. Staged/unknown outcomes remain unqualified by this applied-receipt matrix.
P11 stays **2/6 (33% rounded)**; whole-plan scope coverage stays **57% (57.41% exact)**. Dashboard
timing, tree keyboard, graph keyboard and composed workflow checkpoints remain open. Keep the full
P00–P18 goal active.
