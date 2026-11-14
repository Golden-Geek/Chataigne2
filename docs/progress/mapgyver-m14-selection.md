# M14/1 editor selection and hover

Updated 2026-11-14. Golden owns reusable input/capture/selection dispatch; app adapters resolve
their edited owners and shared Inspector selection. M14/1 remains **Active, unaccepted**.
Overall acceptance stays88/106 (83%); no new fixed gate is closed by this slice.

The user's2026-11-14 instruction selects M14 before full M10/4 qualification. Every M10
requirement remains unfinished and release-blocking; only execution order changed.

## Delivered behavior

Golden `Viewport2D.onselectionclear` replaces the old background-click callback. It dispatches
unconsumed primary-down and idle Escape, preserving pan capture, tool-owned keys, editable
fields and toolbar controls. Its consumer supplies the owner; Golden contains no MapGyver policy.

Screen Warping selects a Surface on ordinary pointer-down without starting geometry edits.
Keyboard/accessibility clicks remain supported; physical mouse-up cannot repeat the selection.
Background-down and idle Escape call the existing Screen-owner selection path. Alt-drag and
corner editing remain on their existing capture/admission path. Existing selected-only handles
are verified in the DOM fixture, including unavailable-source and invalid geometry.

## Applicable editor inventory

| Editor / owner | Current scope | Exact remaining work |
| --- | --- | --- |
| Golden `Viewport2D` | Browser verified primary-down/idle Escape, child/text/tool isolation, navigation and camera lifetime | Complete independent-editor, capture/cancel and app adapter acceptance |
| Screen Warping / Screen | Pointer-down Surface selection, background-owner callback, idle Escape and selected-only DOM handles verified | Hover contour/native overlay agreement; actual workbench/Inspector, overlapping hits, off-page and active-gesture Escape checks |
| Surface Source / Surface | Existing crop editor uses Golden viewport | Explicit crop-control selection/deselection, owner behavior, hover and selected-only native/DOM controls |
| Composition / Composition media | Existing adapter uses spatial viewport | Backend query currently falls back to its first child; preserve explicit owner/no-child selection, wire shared clear, then verify immediate controls, hover and native agreement |
| Matrix Layer spatial / Matrix media | Existing spatial viewport and bounded backend selection | Wire shared clear, reconcile backend selected-row fallback, pointer-down selection independent of edit admission, hover/native/DOM checks |
| Matrix Cell spatial / Layer | Existing spatial viewport and bounded Cell lookup | Explicit owner/no-Cell behavior, shared clear, hover and selected-only controls, active/outgoing/off-page cases |
| Golden graph canvases | Existing graph gestures select nodes; idle Escape currently only attempts gesture/connection cancellation | Reconcile background/idle Escape, selected-only control/hover policy and owner adapters; preserve multi-selection, locked nodes and ports |
| Dashboard, curve and watcher projection canvases | Specialized Golden canvases must be reviewed | Resolve applicable authored-object/point selection versus read-only projection, then verify specialized tool/focus behavior; no exemption claimed |
| Media Preview / Matrix grid | Preview is not the spatial placement editor; grid has its own selection | Confirm applicable owner/child manipulation paths, including read-only preview and virtualized selection; no qualification claimed from classification |

This inventory retains the complete gate. DOM callback fixtures do not establish actual
Inspector transactions, native pixels, GPU parity, all editor selection policies or hover.

## Validation

- `npm run check:mapgyver`:0 errors,0 warnings, including the typed app fixture.
- `node --test apps/mapgyver/ui/src/lib/screens/tests/*.test.ts`:40 pass.
- Golden viewport browser runner: passes new down-before-release and idle Escape/text/tool
  checks plus existing zoom anchoring, intermediate wheel frames, pan/camera/reduced-motion tests.
- App Screen browser runner: passes real `SurfaceHandles` and Golden viewport selection,
  unavailable-source/invalid geometry, selected-only controls, owner callback, keyboard and pan.
- `npm run lint --workspace mapgyver-ui` and scoped shared Prettier: pass.
- Production UI build and ordinary/native-fixture Cargo build: exit0. The desktop build
  explicitly rebuilt its embedded UI. Root/Core formatting and scoped diff checks complete delivery.

The initial Screen background assertion targeted the zoom toolbar; inspecting the actual hit
identified the fixture error. The corrected point is the gap between Surfaces. No product defect
or native failure is inferred from that coordinate mistake. No Rust source/protocol changed.

Logs live under `target/m14-selection-*-2026-11-14.log`. Desktop build log SHA256:
`821a7a236a812954320aa8ad476c273dfd869641b6b1aedd17fdc9cc1b66c859`.
Built ordinary app SHA256:
`529347646ba5342a6d0183570feca482ab59f8827bdc6b1ca881b3f5c1117211`;
native fixture SHA256:
`583234846bb2f1cb16d519176df2f2f3b4f36d2eb16322f59abb0f716681b74a`.
These replace the debug binaries from the historical M10 build receipt; refresh coherent
qualification inputs on return. Existing Video evidence retains its original source/build scope.

Next deliverable: fix Composition's explicit owner/no-child selection at the backend query
boundary and connect its adapter to the shared clear contract. Preserve off-page lookup,
history, locked editors and current transport/Inspector selection; then continue this inventory.
