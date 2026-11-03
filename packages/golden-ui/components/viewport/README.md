# 2D document viewport

`Viewport2D` owns presentation-only camera state, pointer-anchored zoom, Space/middle-drag
panning, selection framing and Home. Callers supply the document aspect ratio and normalized
selection bounds; domain edits remain with the app's authoring operations. `ViewportHandle`
keeps handles round and constant in screen space independently of document aspect and zoom.

Zoom requests accumulate into one bounded animation, with the pointer's world point fixed
through every intermediate camera. Native geometry, overlay layout and hit testing all consume
that displayed camera; there is no separate CSS transition. Pan or domain pointer input freezes
the current camera. Home, framing, resize, identity replacement and disposal cancel pending work.
Reduced-motion preference applies the same target immediately. Document owners include their
backend project generation in `identity`, so reloading the same UUID also cancels obsolete work.
`WorkbenchSession.projectGeneration` exposes that lifetime independently of full-snapshot refreshes;
ordinary snapshot receipts must preserve the camera of the same document.

Editors receive unconsumed primary clicks through `onbackgroundclick` and specialized key
input through `onkeydown`. Pan clicks and child buttons do not become background selection.
`ViewportHandle.selected` exposes an optional pressed state and shared selection styling;
the app owns which document object or local handle is selected.

Layout spacing and handles use relative units. Measured content dimensions and camera offsets
use CSS pixels to match browser pointer coordinates; these are computed geometry, not fixed
layout sizes. Set `native` only for a native underlay aperture: its DOM background must stay
transparent while the native renderer supplies the black active area.

The isolated real-input browser regression runs from the repository root:

`node packages/golden-ui/components/viewport/tests/browser/run.mjs`
