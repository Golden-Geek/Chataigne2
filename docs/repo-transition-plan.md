# Repository Transition Record

The split-repository transition described by earlier revisions of this document completed in Phase
1A. Chataigne2, `golden_core`, `golden_alchemist_core`, `golden_ui`, and
`golden_alchemist_ui` now live in one history-preserving monorepo.

Current ownership and paths are documented in [repo-map.md](repo-map.md). Exact imported source
revisions are recorded in [`product/source-imports.v1.json`](product/source-imports.v1.json), and
the original repository-level notices are preserved under [`docs/imports/`](imports/README.md).

The transition rules that remain architectural requirements are:

- app code consumes reusable Rust behavior through public workspace crates;
- app UI consumes reusable UI through public package exports;
- Rust DTOs remain the single source for generated TypeScript protocol declarations;
- root `Cargo.lock` and `package-lock.json` are authoritative;
- no nested repository, Git submodule, or path import into private internals is required;
- the complete Chataigne product remains runnable throughout later architecture phases.
