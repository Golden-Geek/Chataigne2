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

## Large collections

`components/common/virtual-rows/VirtualRows.svelte` owns bounded row mounting, keyed focus
retention and logical reveal. Its parent owns the scroll element, domain filtering, selection
and drag/drop. `RowHeightIndex` retains keyed measurements across reordering and filtering;
variable rows use a prefix index for logarithmic height updates and viewport lookup. Layout
estimates are in rem; measured CSS pixels match the browser's scroll and rectangle APIs.

Manager, picker and outliner panels share the iterative projection in `panels/outliner/visible-rows.ts`.
It preserves logical depth separately from indentation when a caller hides an intermediate row.
Manager/picker openness lives outside mounted rows, is fenced by document context and node UUID,
and follows accepted or rejected backend metadata intents. Outliner openness remains panel state.
One focused row stays mounted outside the viewport; removing it transfers focus to its scroller.

`components/common/context-menu/ContextMenuRows.svelte` applies the same mechanism to creation
catalogs. Above 128 items, each menu layer uses a searchable, variable-height list; smaller menus
retain compact columns. Stable item IDs preserve row identity across reordered inventories.
The parent ContextMenu owns submenu/action policy and retires layers whose anchor disappears.
Use `npm run test:browser --workspace golden_ui -- --catalog` for its production qualification.

LoggerPanel projects retained history independently of mounted rows. `VirtualRows.followEnd`
keeps the live tail anchored through variable-height measurements; callers can supply bounded
`retainedKeys` for active DOM state. Logger native selection retains at most 64 record rows,
while Copy logs reads filtered logical entries. The session logger publishes readonly raw
history snapshots and applies queued replacements/appends to one detached buffer, avoiding
deep proxy work and mutation of previously published arrays. Run the production log cases with
`npm run test:browser --workspace golden_ui -- --logs`.

Run `npm run check:browser --workspace golden_ui` for the standalone fixture's types and
`npm run test:browser --workspace golden_ui` for production browser qualification. See the
[P11 contract](../../docs/progress/p11-workspace-qualification-contract.md) for scope and gates.

## Start Here

- Read [`docs/source_layout.md`](./docs/source_layout.md) for the UI filesystem and ownership rules.
- Read [the root README](../../README.md) for the app-shell boundary and development commands.
