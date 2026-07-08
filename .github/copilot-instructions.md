## jCodeMunch usage policy

jCodeMunch MCP is available. Use it for code exploration before using native file search or file reads.

Start every code-navigation task with:

1. Choose the owning layer from the workspace repo routing table below.
2. `resolve_repo { "path": "<layer path>" }`
3. If the layer is not indexed, call:
   `index_folder { "path": "<layer path>" }`
4. If the indexed repo is unfamiliar, call `suggest_queries`.

Workspace repo routing:

This workspace is indexed as separate jCodeMunch repos for the app shell and reusable packages. Resolve the concrete owning path before searching; do not use the root `Chataigne2` index for `golden_core`, `golden_ui`, or alchemist package work.

| Layer | Resolve path |
| --- | --- |
| App shell, app-owned modules, app-owned UI, workspace tooling | `.` |
| `golden_core` | `submodules/golden_core` |
| `golden_alchemist_core` | `submodules/golden_alchemist_core` |
| `golden_ui` | `src-ui/src/lib/golden_ui` |
| `golden_alchemist_ui` | `src-ui/src/lib/golden_alchemist_ui` |

Use the returned repo id for planning, search, outlines, and reads. Run `.\tools\watch-jcodemunch.ps1 --status` if the repo map looks stale or ambiguous.

Use these tools by intent:

- Find a symbol by name: `search_symbols`
- Find text, comments, config keys, command names, or TODOs: `search_text`
- Understand repo structure: `get_repo_outline`, then `get_file_tree`
- Before reading a file: `get_file_outline`
- Read implementation: `get_symbol_source`
- Read a symbol plus relevant imports/dependencies: `get_context_bundle`
- Find usage: `find_references`
- Find importers: `find_importers`
- Estimate change impact: `get_blast_radius`

Avoid native file reads unless jCodeMunch cannot answer the question. When falling back, explain which jCodeMunch call failed or was insufficient.
