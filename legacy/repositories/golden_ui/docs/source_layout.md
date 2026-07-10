# golden_ui Source Layout

`golden_ui` is the canonical UI package boundary. Even while it is still checked out under
`src-ui/src/lib/golden_ui/`, contributors should treat it as if it lived in its own package repo.

## Ownership Rules

- `src-ui/src/routes/` is the app shell.
- `src-ui/src/lib/assets/` is app-owned branding/assets.
- `src-ui/src/lib/golden_ui/` owns reusable UI logic and package-local docs.

## Package Boundary Rules

- Internal `golden_ui` code must not depend on `$app/*`, `$env/*`, or other app-only framework
  aliases.
- App shell code should import `golden_ui` through package exports such as `golden_ui` or
  `golden_ui/components/...`, not through `$lib/golden_ui/...`.
- Package-internal imports use package-local relative paths only.
- Product branding or app-specific panels do not belong in `golden_ui`.

## Folder Rules

- `components/`: visual components only.
- `components/app/`: workbench shell components only. No business logic beyond shell composition.
- `components/panels/<panel>/`: each dock panel owns its root component plus panel-local view
  helpers and presentation logic.
- `components/common/`: reusable UI widgets shared by multiple panels.
- `store/`: stateful coordination modules only. Keep one concern per file.
- `store/session/`: focused workbench/session helpers used by the thin `workbench.svelte.ts`
  facade.
  Canonical buckets:
  `selection.svelte.ts`, `warnings.svelte.ts`, `descriptions.svelte.ts`,
  `footer-hover.svelte.ts`, `history.svelte.ts`, `logger.svelte.ts`, and
  `commands.svelte.ts`.
- `transport/`: protocol transport clients and connection abstractions.
- `host/`: browser-vs-desktop bridges and host capability checks.
- `dockview/`: dock/panel framework integration.
- `generated/`: generated Rust-owned protocol bindings only.
- `style/`: shared styles, icons, and fonts.
- `utils/`: stateless helpers only.
- `debug/`: development-only instrumentation.

## File Rules

- If a concept grows children, convert it from `thing.ts` into `thing/` with a `mod`-style index
  file (`index.ts` or `thing.svelte.ts` plus child files).
- Do not keep god files. Anything above a few hundred lines should be challenged for split
  candidates.
- `workbench.svelte.ts` is a facade, not a feature owner. It may compose session stores, transport
  bootstrap, and intent queue flow, but stateful subdomains belong under `store/session/`.
- Keep top-level barrel exports small and intentional. Do not export whole folders just because
  they exist.
- Separate state derivation from rendering. Component files should not become hidden store layers.

## Current Refactor Priorities

1. Split transport/bootstrap and intent-queue orchestration out of `store/workbench.svelte.ts` so
   the remaining facade mostly wires stores together.
2. Keep `store/graph.svelte.ts` on the incremental path and avoid reintroducing whole-graph work
   inside event loops.
3. Move app-specific shell chrome and branding out of `components/app/` where it does not belong in
   the reusable package.
4. Move `golden_ui` out of `src/lib/` into a real package/submodule root once the superproject
   submodule relocation is scheduled.
