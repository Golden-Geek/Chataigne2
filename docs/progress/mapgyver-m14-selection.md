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

Matrix Layer background-down and idle Escape select the Matrix media owner; Cell layout selects
its Layer owner without leaving that Layer. Explicit backend owners clear retained Layer/Cell
hints, while owned Cell descendants resolve to their containing Cell. Both queries preserve
bounded visible pages and independent off-page definitions without authoring or history changes.
Unlocked nested Matrix/Composition owner selection now preserves the edited domain instead of
following the nested media's outer Composition placement.

Golden's public `selectedEditorItem` matches the current selected identity against bounded visible
and independently queried definitions. Matrix controls update immediately on a new pointer choice
or deselection; an old cached definition cannot restore handles. Cell selection follows raw
authoritative pages, so parameter projection copies cannot restore the previous selected Cell.

## Applicable editor inventory

| Editor / owner | Current scope | Exact remaining work |
| --- | --- | --- |
| Golden `Viewport2D` | Browser verified primary-down/idle Escape, child/text/tool isolation, navigation and camera lifetime | Complete independent-editor, capture/cancel and app adapter acceptance |
| Screen Warping / Screen | Pointer-down Surface selection, background-owner callback, idle Escape and selected-only DOM handles verified | Hover contour/native overlay agreement; actual workbench/Inspector, overlapping hits, off-page and active-gesture Escape checks |
| Surface Source / Surface | Existing crop editor uses Golden viewport | Explicit crop-control selection/deselection, owner behavior, hover and selected-only native/DOM controls |
| Composition / Composition media | Root/nested explicit owner clears old hints without first-child selection; shared clear and immediate pointer choice; denied-edit/natural Escape focus browser checks pass; unlocked nested owner retains edited content | Actual Inspector/native/independent-editor and snapshot fencing; unselected Move markers, hover and selected-only presentation remain unfinished |
| Matrix Layer spatial / Matrix media | Shared clear selects Matrix owner; explicit no-child backend projection, immediate controls and cached-definition clearing; denied-edit/idle-Escape DOM checks pass | Hover, actual adapter/Inspector/native checks, independent-editor and active-gesture behavior |
| Matrix Cell spatial / Layer | Shared clear selects Layer without leaving it; backend resolves owned descendants and explicit no-Cell focus; bounded off-page selection and immediate control clearing; DOM checks pass | Hover, actual adapter/Inspector/native checks, active/outgoing/off-page interaction acceptance |
| Golden graph canvases | Existing graph gestures select nodes; idle Escape currently only attempts gesture/connection cancellation | Reconcile background/idle Escape, selected-only control/hover policy and owner adapters; preserve multi-selection, locked nodes and ports |
| Dashboard, curve and watcher projection canvases | Specialized Golden canvases must be reviewed | Resolve applicable authored-object/point selection versus read-only projection, then verify specialized tool/focus behavior; no exemption claimed |
| Media Preview / Matrix grid | Preview is not the spatial placement editor; grid has its own selection | Confirm applicable owner/child manipulation paths, including read-only preview and virtualized selection; no qualification claimed from classification |

This inventory retains the complete gate. DOM callback fixtures do not establish actual
Inspector transactions, native pixels, GPU parity, all editor selection policies or hover.

## Validation

- `cargo test --locked -p MapGyver --features native-qualification --lib -- --test-threads=4`:
  689 pass,8 ignored. New Matrix cases verify media/channel/Layer owner clears, owned Cell
  descendants, bounded independent off-page focus, nested edited-domain retention and unchanged
  history/project data. Prior Composition/Screen cases retain their acceptance scope.
- Strict all-target MapGyver Clippy passes with `native-qualification` and `-D warnings`.
- `npm run check:mapgyver`:0 errors,0 warnings, including the typed expanded app fixture.
- Matrix/spatial Node suites:59 pass, including immediate owner clearing while a backend reply
  is held, bounded geometry and reader/session/project/history fencing. Golden viewport/selection
  suites:14 pass, including cached-definition clearing, off-page identity and capture/admission.
- The previous36 Composition/spatial Node cases retain their unchanged source scope.
- The prior40 Screen unit passes retain their unchanged source scope.
- Golden viewport browser runner: passes new down-before-release and idle Escape/text/tool
  checks plus existing zoom anchoring, intermediate wheel frames, pan/camera/reduced-motion tests.
- Expanded app browser runner: passes real `SurfaceHandles`, `CompositionPreview`, Matrix
  `RectLayoutControls`/`SpatialViewport` and Golden
  viewport selection, unavailable-source/invalid geometry, denied-edit selection, selected gizmo
  clearing, owner callbacks, natural idle Escape focus, keyboard and pan. Both Matrix modes
  verify down-before-up selection, refused-edit selection and no stale cached handles after clear.
  These are control/callback fixtures, not full Matrix adapter/Inspector or native GPU credit.
- `npm run lint --workspace mapgyver-ui` and scoped shared Prettier: pass.
- Production UI build and ordinary/native-fixture Cargo build: exit0. The desktop build
  explicitly rebuilt its embedded UI. Root/Core formatting and scoped diff checks complete delivery.

The initial Screen background assertion targeted the zoom toolbar; inspecting the actual hit
identified the fixture error. The corrected point is the gap between Surfaces. No product defect
or native failure is inferred from that coordinate mistake. The first new Rust fixtures omitted
the required document identity and created a Reference instead of an owned nested Composition;
corrected fixtures pass without relaxing document validation. Rust query behavior changes here;
wire DTOs/generated contracts remain unchanged and their existing tests pass.
The first Matrix test compile used a channel-reference field on a `MediaId`; the corrected
fixture uses its typed UUID. No runtime API or acceptance requirement changed to make it pass.

Logs live under `target/m14-selection-*-2026-11-14.log` and
`target/m14-composition-*-2026-11-14.log`.
`target/m14-matrix-*-2026-11-14.log` holds the current Matrix validation cycle. Latest desktop
build log SHA256:
`f045c082c60ce7721e6d8ff99d5dd91d9948d174cf2af329742625c023734b37`.
Built ordinary app SHA256:
`ee948abc4d4e87c400079eb043f7d6c1428fa98c7f421013e8aa7b3a3df8bf16`;
native fixture SHA256:
`ea9d734b63dce51ee54af59403226e2be76548005bb5d88f5d8df0473fbfd67b`.
These replace the debug binaries from the historical M10 build receipt; refresh coherent
qualification inputs on return. Existing Video evidence retains its original source/build scope.

Next deliverable: Surface Source crop-control selection and owner/clear behavior through the shared
contract. Then continue graph, hover and the full native/Inspector/independent-editor inventory above. No completed slice
or passing software suite substitutes for whole-gate acceptance.
