## jCodeMunch usage policy

jCodeMunch MCP is available. Use it for code exploration before using native file search or file reads.

Start every code-navigation task with:

1. `resolve_repo { "path": "." }`
2. If the repo is not indexed, call:
   `index_folder { "path": ".", "extra_ignore_patterns": [...] }`
3. If the repo is unfamiliar, call `suggest_queries`.

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
