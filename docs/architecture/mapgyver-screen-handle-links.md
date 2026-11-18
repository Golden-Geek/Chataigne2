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

The final drag/link request is flushed before the existing Golden edit group closes.
Focused tests and one native cohort reverse both in one Undo, but a repeated native run
restored an intermediate drag point; repeatable native history remains unaccepted.
Typed Parameter projection stays live during manipulation
for every linked Surface, while the active handle retains its immediate presentation.
Held-pointer peer contours and independent red/blue output pixels pass. M15/8 remains
open for the intermittent native Undo issue and native window visual confirmation.
