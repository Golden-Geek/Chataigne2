# Phase 9: scale qualification and deletion

Phase 9 establishes release gates for 100,000 authored runtime values across dense, sparse,
and idle ticks. The graph UI builds and queries a 100,000-node spatial index in real
Chromium and renders only the visible viewport. Endpoint recovery and fifty-client
latest-wins queues run extended bounded soaks.

Release CI now runs the Rust workspace on Linux, Windows, and macOS. The primary CI job
continues to run architecture, protocol, persistence, runtime, TypeScript/Svelte, Node, and
real-browser gates.

The old root Rust application, old `src-ui`, imported repositories, old protocol output,
Tauri/build assets, compatibility baselines, and submodule-aware tooling were deleted.
`tools/check_workspace_architecture.py` rejects obsolete roots, gitlinks, and legacy Cargo
features so a dual runtime cannot quietly return.

Measurements and gate definitions are recorded in
`benchmarks/phase9/final-qualification.v1.json`. Deleted repositories are traceable through
`archive.md` and Git history rather than retained production source.

