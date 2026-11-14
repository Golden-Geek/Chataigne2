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

Composition's backend query now honors explicit edited-owner selection with no selected child,
including stale off-page hints and nested owned Compositions. It no longer selects a first child
implicitly. Locked content and explicit retained hints for unrelated selection remain supported;
their complete independent-editor policy is still unaccepted. The adapter uses authoritative page
selection rather than projected geometry to retain its selected ID, and forwards background/idle
Escape through the shared viewport contract to the Composition media owner.

Golden's public `createViewportPointer` replaces the app-local helper. Composition and Matrix
select a primary hit before mutation admission and retain capture/cancel behavior. Selection
consumes the hit even when editing is refused, preserving viewport keyboard focus when the hit
control is replaced. The browser test reproduces and verifies this focus correction without
manually focusing the viewport before Escape. Physical clicks cannot reselect through the
keyboard/accessibility click fallback.

## Applicable editor inventory

| Editor / owner | Current scope | Exact remaining work |
| --- | --- | --- |
| Golden `Viewport2D` | Browser verified primary-down/idle Escape, child/text/tool isolation, navigation and camera lifetime | Complete independent-editor, capture/cancel and app adapter acceptance |
| Screen Warping / Screen | Pointer-down Surface selection, background-owner callback, idle Escape and selected-only DOM handles verified | Hover contour/native overlay agreement; actual workbench/Inspector, overlapping hits, off-page and active-gesture Escape checks |
| Surface Source / Surface | Existing crop editor uses Golden viewport | Explicit crop-control selection/deselection, owner behavior, hover and selected-only native/DOM controls |
| Composition / Composition media | Root/nested explicit owner clears old hints without first-child selection; shared clear and immediate pointer choice; denied-edit/natural Escape focus browser checks pass | Actual Inspector/native/independent-editor and snapshot fencing; check unlocked nested spatial-target navigation separately from the locked-query test; unselected Move markers, hover and selected-only presentation remain unfinished |
| Matrix Layer spatial / Matrix media | Consumes shared Golden selection-before-admission/capture helper | Wire shared clear, reconcile backend selected-row policy and immediate selection projection, hover/native/DOM checks |
| Matrix Cell spatial / Layer | Consumes shared Golden selection-before-admission/capture helper | Explicit owner/no-Cell behavior, shared clear, hover and selected-only controls, active/outgoing/off-page cases |
| Golden graph canvases | Existing graph gestures select nodes; idle Escape currently only attempts gesture/connection cancellation | Reconcile background/idle Escape, selected-only control/hover policy and owner adapters; preserve multi-selection, locked nodes and ports |
| Dashboard, curve and watcher projection canvases | Specialized Golden canvases must be reviewed | Resolve applicable authored-object/point selection versus read-only projection, then verify specialized tool/focus behavior; no exemption claimed |
| Media Preview / Matrix grid | Preview is not the spatial placement editor; grid has its own selection | Confirm applicable owner/child manipulation paths, including read-only preview and virtualized selection; no qualification claimed from classification |

This inventory retains the complete gate. DOM callback fixtures do not establish actual
Inspector transactions, native pixels, GPU parity, all editor selection policies or hover.

## Validation

- `cargo test --locked -p MapGyver --features native-qualification --lib -- --test-threads=4`:
  686 pass,8 ignored. New cases verify root/channel owner clears, stale off-page hints, retained
  locked content, no implicit first child, history preservation and an actual nested owned Composition.
- Strict all-target MapGyver Clippy passes with `native-qualification` and `-D warnings`.
- `npm run check:mapgyver`:0 errors,0 warnings, including the typed expanded app fixture.
- Composition/spatial Node suites:36 pass, including clearing an old off-page selection without
  losing visible content. Golden viewport suites:11 pass, including moved capture/cancel tests and
  new selection-before-admission/denied-edit/secondary/consumed/concurrent-hit cases.
- The prior40 Screen unit passes retain their unchanged source scope.
- Golden viewport browser runner: passes new down-before-release and idle Escape/text/tool
  checks plus existing zoom anchoring, intermediate wheel frames, pan/camera/reduced-motion tests.
- Expanded app browser runner: passes real `SurfaceHandles`, `CompositionPreview` and Golden
  viewport selection, unavailable-source/invalid geometry, denied-edit selection, selected gizmo
  clearing, owner callbacks, natural idle Escape focus, keyboard and pan. No native GPU credit.
- `npm run lint --workspace mapgyver-ui` and scoped shared Prettier: pass.
- Production UI build and ordinary/native-fixture Cargo build: exit0. The desktop build
  explicitly rebuilt its embedded UI. Root/Core formatting and scoped diff checks complete delivery.

The initial Screen background assertion targeted the zoom toolbar; inspecting the actual hit
identified the fixture error. The corrected point is the gap between Surfaces. No product defect
or native failure is inferred from that coordinate mistake. The first new Rust fixtures omitted
the required document identity and created a Reference instead of an owned nested Composition;
corrected fixtures pass without relaxing document validation. Rust query behavior changes here;
wire DTOs/generated contracts remain unchanged and their existing tests pass.

Logs live under `target/m14-selection-*-2026-11-14.log` and
`target/m14-composition-*-2026-11-14.log`. Latest desktop build log SHA256:
`5205dcf8508736cf6d2375c38519153dc59a6fcf3fb6905dcb6cfa87a05a6502`.
Built ordinary app SHA256:
`05c82270eb824aee22459f9cbb09b43230c400691bd9b726d63e8b966dde2c6e`;
native fixture SHA256:
`e21156ad8bd61ab3070e967ce91eb40bf6586915fd2393d84c510d118bd58e8b`.
These replace the debug binaries from the historical M10 build receipt; refresh coherent
qualification inputs on return. Existing Video evidence retains its original source/build scope.

Next deliverable: Matrix Layer/Cell background and idle-Escape owner selection through the shared
contract, with backend selection/content separation and immediate controls. Then continue Source,
graph, hover and the full native/Inspector/independent-editor inventory above. No completed slice
or passing software suite substitutes for whole-gate acceptance.
