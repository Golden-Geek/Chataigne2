# 2D document viewport

`Viewport2D` owns presentation-only camera state, pointer-anchored zoom, Space/middle-drag
panning, selection framing and Home. Callers supply the document aspect ratio and normalized
selection bounds; domain edits remain with the app's authoring operations. `ViewportHandle`
keeps handles round and constant in screen space independently of document aspect and zoom.

Layout spacing and handles use relative units. Measured content dimensions and camera offsets
use CSS pixels to match browser pointer coordinates; these are computed geometry, not fixed
layout sizes. Set `native` only for a native underlay aperture: its DOM background must stay
transparent while the native renderer supplies the black active area.

The isolated real-input browser regression runs from the repository root:

`node packages/golden-ui/components/viewport/tests/browser/run.mjs`
