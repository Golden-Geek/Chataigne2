# Imported source records

Phase 1A imported the formerly separate Golden repositories into this monorepo with their exact
Git histories. This directory preserves repository-level notices whose original locations no
longer exist after the import.

The authoritative source revisions are recorded in
[`docs/product/source-imports.v1.json`](../product/source-imports.v1.json). Package and crate source
now lives under `packages/` and `crates/`; the files here are provenance records, not build inputs.
