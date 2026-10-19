# Persistence Transactions

`golden_persistence` owns app-agnostic destination coordination and durable file replacement.
`golden_engine::application::ProductionRuntime` owns project capture and generation-aware metadata
publication. Transport and desktop hosts select paths and report receipts; they do not sequence
backup, journal, temporary-file, rename, or clean/dirty state themselves.

The project document, version, and JSON codec are defined in `golden_persistence`. The
`golden_engine::engine::persistence` adapter maps engine node metadata and recovery errors in
`types.rs`; `mod.rs` extracts and applies records, `lifecycle.rs` reconciles loaded state,
`transaction.rs` adapts rollback errors, and `duplicate.rs` prepares detached subtrees. The shared
`engine/transaction.rs` owns publication/runtime checkpoints for atomic edit operations. None of
these engine adapters owns the file format or native path-selection workflow.

Multi-root duplicate actions prepare each detached subtree before mutation, then insert the
whole batch under one rollback checkpoint. Loaded-node attached/init/ready callbacks replay in
creation-context groups, so a paste does not rebuild a full process-tree snapshot for each root.
Each inserted root retains its insertion-time sibling anchors for a single undo transaction.
History replay batches independent same-parent additions: undo shares one destroy lifecycle
snapshot, while redo restores all roots before building one UI catalog snapshot and running one
ready lifecycle batch. The post-restore catalog snapshot is also passed to ready callbacks, with
a rebuild only if a callback changes structure. Disjoint multi-root removals, including roots
under different parents, share a destroy lifecycle pass and publish one UI graph transaction with
a final child-order patch for each affected parent. Their undo restores every root before one
shared catalog/ready snapshot; redo shares a destroy pass again. Nested removals or transactions
mixing edit kinds retain stepwise replay.

Blueprint instance metadata travels with detached add/remove history payloads, including
nested instance roots. Replacement history swaps that metadata with the detached node.
The registry therefore contains only live instances; rollback, undo, and redo restore both
the node identity and its blueprint declaration/override lookup.

`engine/edit_journal.rs` is the ordered history collector for both low-level applied-prefix
drains and atomic operations. Nested lifecycle edits share their owner's collector. Request
order is reserved before callbacks run, so an inner callback completing before its outer
insertion does not reverse undo order. Explicit session boundaries flush the accepted prefix
before changing the active session. Ordinary drains commit accepted effects even on completion
failure; they do not collect a whole-tree membership snapshot.

Atomic subtree operations retain edits crossing the original-tree boundary. Candidate-owned
descendants remain represented by their outer insertion. History and redo changes publish only
after success; rejection reverses the journal and restores the checkpoint. A rollback failure
retains both the original rejection and the inverse-edit error as typed causes. External ingress
and previously queued edits/inbox events stay outside the provisional scope and keep their order
on success or rejection. UI admission settles earlier accepted notifications before opening the
new checkpoint, then settles the operation's own edits/notifications inside it. Generic engine
stabilization lives in `engine/transaction.rs`; lower-level subtree APIs deliberately defer root
observer notifications. UI duplicate admission uses the same typed transaction helper to extend
its boundary through notification delivery. Nested prepared-subtree operations join the outer
journal. An existing user edit session retains accepted earlier steps on rejection.

Control-mode edits use the internal `Edit::SetParamControlState` pipeline. The scalar control
state and its owned child trees have separate recorded effects; history restores both without
recreating child UUIDs. Control editing/materialization lives in `engine/controls/edit.rs`.

`engine/history/atomic.rs` owns undo/redo admission and failure recovery, reusing the outer edit
checkpoint for publication, indexes, parameters, and pending-work isolation. Per-step backups
retain the actual live authored state and position, which can differ from recorded forward values
after runtime activity. Non-cloneable subtree/replacement payloads stay in their original history
steps; transition markers allow reverse recovery even when lifecycle completion rejects. Restore
parents and identities are checked before taking detached payloads. Rejected replay retains its
transaction on the original stack. Disjoint batch replay still shares lifecycle/catalog snapshots.

Undo/redo ready activation uses `NodeCreationContext::HistoryRestore`. Runtime callbacks and
subscriptions still run, while authored self-state is preserved and immediate authored callback
edits are discarded because history already replays the authoritative changes. Replay-generated
notifications settle under this rule before returning; earlier pending work remains isolated.
Provisional rollback reverses authored steps without invoking ready callbacks, then activates any
restored existing nodes in this history context. Replacement history swaps payloads at one stable
node identity and preserves live topology. Teardown also preserves authored self-state whenever
the payload is retained for undo or atomic rollback. Device effects already dispatched cannot be undone by
restoring authored state. The current P01 ledger records validation and remaining acceptance.

Mutable Rust callbacks and script properties/methods use `engine/apply_opaque.rs` to capture
owned authored data before nested queued requests are applied. Event-dispatch and effective-enabled
handlers share that capture while an authored journal is active. Reference/warning/context state
finalizes once per journal flush; rejected operations restore the prior context registry/cache.
`Node` exposes complete capture
and in-place restore hooks; the node macro generates these from the same persisted-field
declarations as project serialization, including defaults. Sparse custom codecs must provide
a complete capture. Composed nodes include both inherited and local state. Parameter capture
uses its complete typed data record; script restoration preserves runtime resources and defers
required reloads. Runtime-only fields are excluded. Identity, links, and effective-enabled caches
remain engine-owned; callbacks request their edits through `ProcessCtx`.

An authored callback effect remains undoable if its completion rejects. Atomic callers reverse
it with the rest of their journal. Undo/redo restores the recorded authored state without
executing the original callback again; cache, parameter events, and node projections are refreshed.

## Save transaction

1. The control actor captures an owned sparse document with its `ProjectGeneration` and authored
   history revision.
2. `PersistenceCoordinator` normalizes the destination through its nearest existing ancestor and
   assigns a monotonic request ID before JSON encoding.
3. Encoding runs outside the actor. Accepted saves for one destination commit in request order;
   different destinations use bounded concurrency.
4. One admitted transaction owns backup, recovery journal, temporary-file commit, target
   replacement, cleanup, and metadata publication as a single lease.
5. Path and saved revision publish only when generation and request ID still win. A later authored
   revision remains dirty even when an older captured revision was written successfully.

Dropped tickets cancel their queue position. Lexical aliases and existing-path canonical aliases
share the same destination identity; Windows identities are case-folded after canonicalization.

If the file commit succeeds but actor admission/completion prevents metadata publication from
being acknowledged, `ApplicationError::SavePublication` retains the committed normalized path,
request ID, project generation, document revision, and typed control error. The error does not
imply a file rollback. An admission rejection leaves the previous path/dirty metadata intact; a
lost completion may have published it. A subsequent save receives a newer request ID and can
publish normally under the same generation/revision fences.

## Replacement fence

Detached candidate preparation does not block saves. Immediately before live cutover, replacement
raises an exclusive generation fence. Active transactions finish, including metadata publication,
before the fence is granted. On successful cutover the coordinator advances to the new project
generation and invalidates every unstarted old-generation ticket. If replacement fails, dropping
the fence preserves the previous generation and releases its pending saves.

## Recovery

The file transaction preserves the previous complete target as a sibling backup and writes a
digest-bearing recovery journal before replacing the primary. Failures before target commit leave
the previous complete primary or backup; failures after target commit leave the complete new
primary. Recovery may retry, and a later save can always replace either valid state. Tests inject
failures after each backup, journal, temporary-file, target, and cleanup boundary.
