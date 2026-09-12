# Persistence Transactions

`golden_persistence` owns app-agnostic destination coordination and durable file replacement.
`golden_engine::application::ProductionRuntime` owns project capture and generation-aware metadata
publication. Transport and desktop hosts select paths and report receipts; they do not sequence
backup, journal, temporary-file, rename, or clean/dirty state themselves.

The project document, version, and JSON codec are defined in `golden_persistence`. The
`golden_engine::engine::persistence` adapter maps engine node metadata and recovery errors in
`types.rs`; `mod.rs` extracts and applies records, reconciles loaded lifecycle state, and delegates
duplicate-tree handling to `duplicate.rs`. None of these engine adapters owns the file format or
native path-selection workflow.

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
