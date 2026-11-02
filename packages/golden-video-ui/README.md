# Golden Video UI

This package owns reusable native-preview registration and CSS geometry measurement. Apps own
source routing and editor gestures; Golden Video owns native layout admission and rendering, and
the shared desktop host supplies authentic WebView/DPI metrics. No pixels, native texture handles
or app graph mutations cross this presentation transport.

`NativePreview` registers a rectangular region with a window-scoped `PreviewSession`. It measures
the complete source rectangle and its ancestor/viewport clipping in CSS coordinates, including
scroll containers. The host converts those coordinates once. Source keys are opaque app-owned
routing references, while view IDs are transient frontend presentation identities.

Create one session per editor window with `createPreviewSession(createDesktopPreviewTransport())`,
then call `connect()` from that window's owner. Pass it to each `NativePreview` with a `source` key.
The component accepts a `visible` flag and an optional children snippet for DOM/SVG overlays.
Parent layout sets its size using normal relative CSS units. Browser sessions report preview
unavailable while app editing remains usable; they do not pretend to stream remote video.

The session coalesces geometry at animation-frame boundaries, keeps one IPC request in flight and
one newest desired layout, and discards replies from replaced/disposed sessions. A one-second
heartbeat renews the native layout lease. Endpoint failure withdraws availability and reconnects
with bounded backoff. Source changes retain the last installed frame while the replacement is
prepared. Pan/zoom updates live native placement instead of replacing GPU resources.
`presentedViews` tracks established presentation separately from `installed`,
which still requires confirmation of the matching latest revision through Layout/Status replies.
Geometry changes cover retired pixels outside the current aperture, leaving live placement
uncovered while it moves. Golden's native `Viewport2D` supplies a stationary canvas clipping/backing
rectangle: newly exposed canvas pixels are black without turning the rest of the workbench black.
Hidden/removed views and session failure cover stale
preview holes. Adding or removing an Inspector preview does not cover an unchanged editor preview.
Pending views show a plain black backing; “Preview unavailable” is reserved for capability failure.
Pending/unavailable views and retirement covers use opaque black, matching the output's empty
area. Installed preview surfaces remain transparent so the native underlay stays visible.
An acknowledgement proves a native generation/frame observation, not scanout or physical alignment.
Unchanged installed heartbeat layouts do not flash the covers. Unregister/dispose explicitly
withdraw presentation; native lease expiry remains the fallback after frontend interruption.

Session-owned retirement masks survive a removed component. Each view unions its old clipped CSS
areas until the latest native installation is confirmed; a fixed 64-cover ceiling conservatively
merges overflow during rapid unmount/remount churn. Each cover uses at most four rectangular masks
around its current aperture, bounding the DOM at 256 masks. These pointer-transparent, opaque masks sit
below DOM controls. Their fixed CSS-pixel geometry is a deliberate measurement exception, not a
product layout unit. Disposal first publishes an empty layout and waits for its installed clear
before Disconnect. Failed/expired confirmation leaves masks in place and reports an error rather
than claiming the stale underlay retired; the host owns final window retirement. Awaiting dispose
does not block the UI thread, and its state must still be checked for confirmed clearing.

Geometry support is axis-aligned rectangular layout/clipping. Rotated/skewed or partially opaque
containers are hidden rather than represented as an incorrect rectangle. Full physical display, mixed-DPI, zoom and input
qualification remain with the app's presentation milestone. Editing semantics never live here.

Rust's canonical DTOs generate `generated/`; do not edit those declarations. Run `npm run test`,
`npm run check` and `npm run lint` in this workspace after generation. `npm run build:smoke` builds
the owned Svelte/native fixture under `tests/native/`, honoring `GC_UI_OUT_DIR` for the shared Rust
UI-bundle helper. The fixture reports bounded metadata through `golden-preview-smoke` events and
`window.__previewSmoke`; it never reads back production frames.

`npm run test:browser` exercises that built Svelte fixture in an owned headless browser with an
injected metadata-only native adapter. It verifies real ResizeObserver/animation-frame ordering,
installation covers, clipping/unregister lifecycle and a truthful browser-only fallback. On Windows
it uses installed Edge; `GOLDEN_PREVIEW_BROWSER_PATH` can select another installed executable.
The adapter is not native-renderer or pixel evidence. The fixture bootstrap also reports import,
mount and unhandled runtime failures so a native smoke cannot silently wait for missing phases.
