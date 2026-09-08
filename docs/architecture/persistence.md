# Persistence Transactions

`golden_persistence` owns app-agnostic destination coordination and durable file replacement.
`golden_engine::application::ProductionRuntime` owns project capture and generation-aware metadata
publication. Transport and desktop hosts select paths and report receipts; they do not sequence
backup, journal, temporary-file, rename, or clean/dirty state themselves.

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
