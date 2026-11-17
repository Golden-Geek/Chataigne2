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
- New links/merges require distinct Screens and at most 256 corners. Conflicting
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
  Surface can therefore inherit a link already present on that Screen. New merges
  still reject repeated Screens, and unlink remains available for inherited groups.
- Link/merge/unlink participate in the caller's existing app-operation transaction.
  Geometry propagation, control children, Undo/Redo and project reload use ordinary
  Golden history and persistence.

The backend commands and focused tests are the first delivery slice. Magnet snapping,
the link icon, viewport thresholds and native editor/output geometry verification
remain M15/8 work; this backend slice does not accept the full gate. Turning snapping
off will suppress new links while existing authored bindings continue to operate.
