# 2D document viewport

`ViewportAlignment` supplies presentation-only edge/center proximity hints. Callers freeze a
bounded candidate page and exclude the edited object at pointer-down. The shared helper indexes
candidate axes once and queries them by binary search, evaluates only the moving shape, and
retains an acquired guide through a slightly larger release radius. It solves coupled canvas
coordinates for rotated scale projections and ignores fixed resize edges and invalid geometry.
Use `{ centers: false }` for resize gestures: the moving edges can meet target edges or centers,
without the moving body's changing center unexpectedly holding a resize. `ViewportGuides`
paints noninteractive, screen-width dashed guides. Thresholds come from the current measured
viewport, so zoom/pan and non-square document axes retain the same screen-space proximity.
Neither helper owns document state, mutation semantics or history. Send the adjusted pointer
sample through the caller's ordinary authoritative geometry intent.

`Viewport2D` owns presentation-only camera state, pointer-anchored zoom, Space/middle-drag
panning, selection framing and Home. Callers supply the document aspect ratio and normalized
selection bounds; domain edits remain with the app's authoring operations. `ViewportHandle`
keeps handles round and constant in screen space independently of document aspect and zoom.

Zoom requests accumulate into one bounded animation, with the pointer's world point fixed
through every intermediate camera. Native geometry, overlay layout and hit testing all consume
that displayed camera; there is no separate CSS transition. Pan or domain pointer input freezes
the current camera. Frame and Home retarget that same displayed camera with a short 160ms
ease-in/ease-out transition; repeated shortcuts preserve an identical pending destination.
Wheel zoom can interrupt navigation while retaining its new pointer anchor. Resize, identity
replacement and disposal cancel pending work; identity replacement resets Home immediately.
Reduced-motion preference applies the same target immediately. Document owners include their
backend project generation in `identity`, so reloading the same UUID also cancels obsolete work.
`WorkbenchSession.projectGeneration` exposes that lifetime independently of full-snapshot refreshes;
ordinary snapshot receipts must preserve the camera of the same document.

The focused viewport registers Frame/Home with Golden's command router, which captures F/H
before local DOM events in the workbench. Local navigation also works in standalone hosts.
Editable fields, other viewports and held pointer edits do not claim those commands.

Editors receive unconsumed primary presses through `onselectionclear` and specialized key
input through `onkeydown`; editable controls and the navigation toolbar retain their own keys.
`onfocusout` lets an editor flush its input gesture when focus changes. Pan clicks and child
buttons do not become background selection.
`ViewportHandle.selected` exposes an optional pressed state and shared selection styling;
the app owns which document object or local handle is selected.

Contour and handle paint uses a short120ms transition, shared with native preview decoration.
Only color/fill/shadow changes are eased: camera, object coordinates, dimensions, stroke hit
geometry and pointer admission stay immediate. The common handle palette softens its fill and
uses a thin contrasting selected halo while keeping its screen-space size and opaque normal
border. Transparent rotation hit zones use `paint={false}`. Reduced motion disables easing.
`viewportContour` also reports keyboard focus independently of hover, without selecting or
editing an object. `Viewport2D`, `ViewportHandle` and `ViewportTarget` load `feedback.css`;
standalone contour hosts can import that public stylesheet explicitly.

For a contour with sibling controls, pass `hoverWithin` with a selector for their common
ancestor. The action resolves that hit area at mount and continues to paint the original
node. Moving between controls in the hit area keeps its hover state.

Layout spacing and handles use relative units. Measured content dimensions and camera offsets
use CSS pixels to match browser pointer coordinates; these are computed geometry, not fixed
layout sizes. Set `native` only for a native underlay aperture: its DOM background must stay
transparent while the native renderer supplies the black active area.

The isolated real-input browser regression runs from the repository root:

`node packages/golden-ui/components/viewport/tests/browser/run.mjs`
