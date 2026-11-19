# Screen-handle links

Golden Engine owns connected bidirectional Parameter bindings and their propagation.
MapGyver owns which Surface corners may be linked. The editor will choose proximity
candidates; it must not maintain document links or write control-reference children.

- `ScreenEdit::LinkScreenCorners { source, target }` merges both complete groups and
  aligns them to the target. `UnlinkScreenCorners { member }` removes the resolved
  group's bindings while retaining every corner's current position.
- A `ScreenCorner` identifies a real Surface and corner index `0..3`. Its Vec2
  Parameter is the only authored position. Links use normalized Screen coordinates:
  `(0.5, 0.5)` means the center of each output, regardless of resolution, display
  arrangement or DPI. There is no physical-desktop-coordinate conversion.
- New links/merges require different Surfaces in the same Screen and at most 256 corners.
  Same-Surface and other-Screen corners are rejected by the backend for every caller. Conflicting
  Expression/Animation/Proxy controls, projected or missing bindings, non-corner
  members and incompatible constraints reject before edits are queued. Existing
  authored controls are never silently replaced to create a Screen link.
- Each merged group is a star: one Manual anchor and ordinary Binding/reference
  Parameters pointing to it. No link DTO or UI state is persisted. Inspector,
  script, transport and editor edits all use the same Parameter runtime.
- Golden chooses the component's latest writer, with the existing lowest-node-ID
  tie break. It visits each vertex/edge a bounded number of times and queues every
  reachable member in that evaluation. Propagated echoes do not become competing
  authors on subsequent ticks. Conversions retain the existing binding projection
  rules; MapGyver creates direct Vec2 bindings only.
- Missing UUIDs remain authored references. Restoring the original identity reconnects
  the group; recreating a label with a new UUID does not. Unlink can clear a missing
  singleton. No polling or synthetic missing node is added.
- Copies retain Golden's normal reference-remapping behavior: external bindings stay
  external; references inside a copied subtree remap to copied identities. A copied
  Surface can inherit its existing same-Screen link; new merges reject repeated Surfaces.
- Link/merge/unlink participate in the caller's existing app-operation transaction.
  Geometry propagation, control children, Undo/Redo and project reload use ordinary
  Golden history and persistence.

The Screen editor fetches 64-corner pages automatically across its existing128-Surface
geometry scope. No paging arrows, peer previews, source leases or full-document pointer
scans are introduced. Proximity uses a0.65rem displayed radius and the current zoomed
canvas bounds. The supplied magnet SVG is colored when active and grey when off.
Turning snapping off suppresses new links; authored links continue operating.
Candidate and related-Surface inspections are background reads. Candidate refresh does
not disable otherwise available geometry input; its metadata is optional snapping context,
and the backend still validates every final link against current document state.

The final drag/link request is flushed before the existing Golden edit group closes.
Focused tests and five native drag/link/Undo cycles reverse both in one Undo. Earlier
automation failures are retained in the receipt; the corrected fixture requires authoring
readiness immediately before pointer-down as well as accepted native hit readiness.
Screen Warping projects the latest dragged vertices onto every visible member of the
selected Surface's admitted corner groups. The Screen editor read returns those groups
alongside geometry. The snapping toggle controls the separate candidate read. The SVG
outlines, native outlines and hit geometry use one projection. After release, the last
accepted geometry covers the handoff to the next authoritative editor read. Golden
Parameters still own the authored geometry and binding propagation.
Held-pointer peer contours and independent red/blue output pixels pass. The guarded
foreground capture also matches all eight image edges to DOM geometry (32 color-boundary
probes within the contour-stroke/pixel-rounding allowance) and all four corner/unlink native
hits. M15/8 is Accepted; see the [receipt](../progress/evidence/mapgyver-m15-surface-links-2026-11-18.json).
The native fixture waits for each exact EndEdit receipt; the five-cycle backend history
test passes. This does not requalify retained high-refresh, Intel or presentation-motion gates.
