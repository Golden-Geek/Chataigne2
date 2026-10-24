# golden_ui

`golden_ui` is the reusable core UI package for the Golden stack. It is a root JavaScript workspace
package under `packages/golden-ui/`, separate from app-local SvelteKit source.

## Ownership

- `golden_ui` owns the reusable workbench shell, dock/panel framework, session stores, transport
  clients, host bridges, generated protocol bindings, and shared styling primitives.
- `apps/chataigne/ui/src/routes/` is the thin app UI shell that composes the package.
- `apps/chataigne/ui/src/lib/assets/` owns product-specific assets and icon overrides.

## Package Rules

- Package internals must stay self-contained. Do not depend on `$app/*` or other app-only
  framework aliases from inside `golden_ui`.
- App shell code should consume `golden_ui` through package exports, not by reaching into package
  files through `$lib/golden_ui/...`.
- Package-internal imports now use relative paths only so the package is physically self-contained.
- Keep session/state logic in focused stores with a thin facade instead of growing
  `workbench.svelte.ts`; `store/session/` is the current split point for selection, warnings,
  descriptions, footer hover, history, logger, and command state.
- Keep transport wiring behind `transport/` interfaces. Session state should not know about a
  concrete websocket implementation.

Authoritative operation replies can request selection with `session.requestNodeSelection`, using
the committed node ID/UUID pairs. The focused selection store resolves one pending request from
projection events, checks the project root and persistent identities, and cancels it when the user
makes another selection. `session.selectionRevision` fences replies that arrive after a newer
selection intent; rendering and graph projection do not advance that revision. Apps should use
these contracts instead of polling for an unseen child or scanning labels to infer creation results.

## Start Here

- Read [`docs/source_layout.md`](./docs/source_layout.md) for the UI filesystem and ownership rules.
- Read [the root README](../../README.md) for the app-shell boundary and development commands.
