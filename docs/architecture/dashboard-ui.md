# Dashboard UI ownership

Golden UI renders app-agnostic dashboard pages and widgets. The backend owns authored
nodes, parameters, bindings and edit transactions. App UI registers product renderers through
`dashboard-node-widget-registry.ts`; Golden UI does not import app types or policy.

Start at `packages/golden-ui/components/panels/dashboard/DashboardCanvas.svelte` for composition
and widget templates. Its `canvas/` modules own distinct presentation lifetimes:

| Owner | Responsibility |
| --- | --- |
| `surface.svelte.ts`, `SurfaceContent.svelte` | Child layout, tabs/accordion view state, incoming drop previews and recursive rendering. |
| `camera.svelte.ts`, `page-view.ts` | Viewport measurements, pan/zoom, persisted camera state and Home/frame commands. |
| `free-layout.svelte.ts`, `ordered-layout.svelte.ts` | Move/resize/reorder gestures, previews, coalesced edits and interaction cleanup. |
| `interaction-geometry.svelte.ts`, `interaction-chrome.svelte.ts` | Transform targets, hit testing, insertion/ancestor decoration and move-preview publication. |
| `marquee.svelte.ts` | Surface presses and marquee selection. |
| `widget-content.svelte.ts`, `binding-drop.svelte.ts` | Live target rendering, generic controls and binding intents. |
| `context-menu.svelte.ts` | Capability-dependent menu presentation and explicit edit intents. |
| `geometry.ts`, `dom.ts`, `contracts.ts` | Pure coordinate calculations, DOM measurements and shared presentation types. |

Controllers receive live state through getter contexts. Methods that need a mutually dependent
controller defer access until use; constructors must not eagerly read a later controller.
Each owner disposes its event listeners, observers, animation frames and edit sessions.
`SurfaceContent.svelte` adds no wrapper, preserving direct-child layout and hit-test boundaries.

Shared CSS lives under `canvas/styles/`, namespaced by `.gc-dashboard-canvas`. It covers both
the root shell and descendants across recursive component boundaries, in the original cascade
order. Context-menu portal selectors keep their separate `gc-dashboard-context-menu` namespace.

Rendering uses the authored anchor and position directly. Changing an anchor does not cause a
mounted widget to rewrite its position. Viewport-dependent presentation must not repair domain
state or make authored meaning depend on whether a widget is mounted.

The decomposition preserves the pre-culling mounted layout. P11 still owns viewport-aware free
surfaces, logical offscreen geometry for framing/selection and interaction/focus retention; these
are not established by the split. See the [P11 contract](../progress/p11-workspace-qualification-contract.md)
and [comparison evidence](../progress/p11-dashboard-decomposition.md).
