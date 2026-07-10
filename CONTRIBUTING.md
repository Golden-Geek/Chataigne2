# Contributing

## Choose the owner first

Put stable identity and authored primitives in the lowest appropriate `golden-*` crate.
Generic graph behavior belongs in `golden-graph`; formula and statechart policy belongs in
their domain crates. Runtime execution, IO, transport, persistence, and host work stay in
their named crates. Chataigne-specific modules and UI registrations stay under
`apps/chataigne`.

Do not add a compatibility path, cross-language protocol duplicate, app import in a Golden
package, or a second runtime/store for the same responsibility. Change the public boundary
when the existing boundary is wrong.

## Workflow

1. Add or update a characterization test before structural deletion.
2. Keep runtime and tests in separate files and source files below 1,000 lines.
3. Generate TypeScript protocol types through `golden-codegen`; never edit generated DTOs.
4. Update architecture docs and machine-readable benchmark evidence with architecture work.
5. Run `tools/check.ps1` (or the equivalent commands in CI) before committing.

Rust changes must pass formatting, warnings-as-errors Clippy, all workspace tests, release
scale gates, and both architecture validators. UI changes must pass TypeScript/Svelte
checking, Node tests, and real Chromium frame/graph gates.

Use `tools/supercommit.ps1` or `tools/supercommit.sh` for a focused repository commit.
