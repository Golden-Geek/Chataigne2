# Code Navigation

jCodeMunch is an optional local MCP service for navigating the apps, Golden crates and UI
packages. It owns a disposable source index outside the checkout; it is not part of the apps,
builds, protocol generation or acceptance gates. Cargo, rust-analyzer and scoped native searches
remain authoritative for compiler behavior and anything outside the index.

## Configuration

- [.codex/config.toml](../../.codex/config.toml) registers the server for this trusted project.
  Codex starts it in the checkout, with a file watcher and strict freshness. The watcher lives
  with the MCP process; no Windows service, scheduled task or Git hook is installed.
- [.jcodemunch.jsonc](../../.jcodemunch.jsonc) raises the local index cap to 12,000 files and
  excludes dependencies, build output, generated contracts, old submodules and evidence archives.
  Authored source and tests remain eligible, including uncommitted changes.
- User configuration is `%USERPROFILE%/.code-index/config.jsonc`. Only this checkout is added
  to `trusted_folders`; the existing whitelist policy remains enabled. The `counter` tool
  surface exposes six entry tools, with the full action catalog available on demand.
- Source caches and the local ONNX model live under `%USERPROFILE%/.code-index/`.
  AI summaries and anonymous usage sharing are disabled. Embeddings use the bundled
  `all-MiniLM-L6-v2` model locally, without an API key.

The initial verified setup uses `jcodemunch-mcp 1.108.428`, isolated by uv on Python 3.12.
The installed executable is `%USERPROFILE%/.local/bin/jcodemunch-mcp.exe`; that directory must
be on the MCP client's PATH. Restart the Codex client after adding this server, then start a
new session in this checkout so its tools are discovered.

## Installation and maintenance

From the repository root:

```powershell
uv tool install --python 3.12 'jcodemunch-mcp[watch,local-embed]==1.108.428'
jcodemunch-mcp download-model
codex mcp get jcodemunch
jcodemunch-mcp config --check
jcodemunch-mcp index . --no-ai-summaries
```

On another machine, add the absolute checkout path to the user's `trusted_folders` list
before indexing. Do not disable the whitelist. Set `tool_surface` to `counter` and
`share_savings` to `false` in that user configuration.

Older installations may override `.svelte` as `typescript` or carry an outdated explicit
language list. Remove that override or map it to `svelte`, and include `svelte` in any explicit
language list. In this version, parser extension/language defaults and the advertised tool
surface are read globally; the project config check alone does not establish that they apply.
After changing parsing defaults, request `index_folder` with `incremental=false` and
`force_reparse=true` through MCP to rebuild unchanged files too.

Resolve the checkout before using a repository ID. If indexing reports both local and Git
identities for the same path, inspect the IDs and source roots, then delete only the duplicate
generated index with `jcodemunch-mcp delete-index <exact-id>`. Source files are unaffected.

## Agent workflow

1. Read `jcodemunch_guide` once, then call
   `order(action="resolve_repo", args={path: "."})`. Use the returned ID.
2. Choose the owning layer from [AGENTS.md](../../AGENTS.md#code-exploration-policy).
   Call `menu(query="...")` for action examples or `route(task="...", repo="...")`
   for a suggested action. Check the live schema; examples in upstream prose may lag it.
3. Use `search_symbols` with a bounded `file_pattern` and small `max_results`, then
   `get_symbol_source` or `get_file_outline`. Use `semantic=true` for conceptual searches.
   The first semantic search computes missing embeddings; `embed_repo` can precompute them
   explicitly with `allow_state_change=true` on `order`.
4. Use indexed references and importers to find candidates. Verify Rust traits, macros,
   generated DTO consumers and Svelte dynamic references with native tools before editing.
5. Let the watcher refresh edits. After large changes or branch switches, re-index once
   from the CLI if needed. If a result remains stale, use `index_file` or batched
   `register_edit` through `order(..., allow_state_change=true)`, then query again.

Use `rg` for known literals, Markdown, generated contracts and shader languages without
index support. Excluded or unsupported files, incomplete import extractors, conditional Rust
compilation and dynamic references mean an empty query is not proof that code is absent.
Read current source before patching, and run the owning behavioral checks after changes.

## Initial verification

On 2026-11-09, the rebuilt index contained 3,890 files and 56,697 symbols, including 210
native Svelte components, without file-cap truncation. An MCP client verified the handshake,
checkout resolution, Rust symbol search and source retrieval, Svelte outlines, and automatic
refresh after a source edit. The temporary watcher fixture was removed afterward.

The optional full semantic-cache warm-up runs separately and resumes already cached vectors.
Its local progress log is `%USERPROFILE%/.code-index/chataigne2-embedding-warmup.log`; the
last record reports `semantic_search_check` after completion. Initial warm-up can take tens
of minutes on this workspace. Until it finishes, prefer ordinary symbol search, since a
semantic query may attempt to compute all missing vectors and exceed the MCP tool timeout.

Upstream documents installation and licensing in its
[README](https://github.com/jgravelle/jcodemunch-mcp#readme) and
[client guide](https://github.com/jgravelle/jcodemunch-mcp/blob/main/CLIENTS.md).
The installed release is free for personal/non-commercial use; for-profit company or paid
client work requires its commercial license. Installation does not purchase a license.
