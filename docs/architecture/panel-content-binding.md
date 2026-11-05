# Panel content binding

Golden owns tab presentation and capability lifetime. Applications own the meaning and
resolution of a panel's target. A binding is presentation state, separate from authored
document Parameters.

`PanelApi.setContentBinding({ locked, canLock, toggle })` publishes one panel instance's
live capability. The workbench renders its keyboard-accessible lock immediately before
Close, reflects `aria-pressed`, and removes the capability when the content is disposed.
Fixed panels publish no capability and show no lock. The registry belongs to one workbench;
it is neither global nor persisted. Tab renderers subscribe by exact panel ID and dispose
their subscriptions. A toggle invokes the current panel owner, without inspecting app data.

Each content owner persists only its typed target identity, document identity and lock state
through the existing panel-parameter persistence boundary. Restored definitions come from
the current backend/projection; the panel never persists a copy of authored node data.
Inspector persists node UUID and document-root UUID, and retains a missing locked identity
for Undo rather than following another node. The graph's `IdentifiedNodeMap` resolves the
current numeric ID after Load. Its UUID index shares the existing copy-on-write trie lifetime,
including staged/cancelled projections; complete-identity collision buckets prevent hash
collisions from aliasing nodes. Ordinary value updates keep the identity index unchanged.
Lookup does not enumerate the graph or add per-panel document scans.

MapGyver's Composition query resolves relevant descendants in Rust. Its `retained` channel
is a fallback only when the current selection has no Composition owner and the saved
document matches. `locked` takes precedence over selection. An unlocked panel can follow a
new document's relevant selection, but it cannot reuse that document's unrelated selection
to revive an old target. Missing targets remain unavailable and can recover through Undo or
same-document Load. Bounded pages and independent selected-object reads remain unchanged.

Composition, Media Preview, Inspector, Screen Warping, Surface Source and the Matrix grid publish the same
capability and have no body lock.
Composition stores the last successful target even while unlocked. Media Preview retains
its existing explicit lock/channel/grid/page behavior. The Matrix spatial adapter must use
this same capability and relevant-target rule when integrated under M12/6–7; current Screen
integration does not claim that future Matrix integration or complete M12/6 acceptance.

Screen/Surface editors own separate readers and geometry gestures; they no longer share a
window-level selection/controller. `InspectEditor` resolves relevant ancestors and locks in
Rust, with the same persisted-root fencing and unrelated-selection retention. Screen pages
contain at most 128 Surface definitions plus one independent selected Surface. Surface Source
reads its exact Surface without traversing a Screen's child list. Native Screen rendering
still includes the complete authored scene. `DropSourceAt` resolves the topmost actual Surface
at drop time through the admitted projective mapping, including beyond the geometry page;
the editor's viewport hit is only a visual hint. Explicit handle drops retain their explicit
target. Screen rows remain public Golden outliner projections with bounded row readers.
The obsolete shared editor context and its manager/media catalog reads are removed.

Matrix grid panels own separate controllers, viewport/scroll presentation and selected addresses.
`InspectMatrixEditor` resolves the nearest Matrix ancestor and its Layer/Group/Column/Cell
selection in Rust, including Parameters and owned Cell media. A nested Matrix establishes its
own target. The query shares the existing bounded viewport index rather than reading a
catalog or walking the UI graph. Relevant selections follow while unrelated selections retain
the last target; document-root fencing, missing-target recovery and lock precedence match the
other editors. Grid presentation never launches playback. One application-runtime subscription
remains shared by the workbench, independently of the number of grids. The removed global
Matrix controller no longer couples one panel's selection or viewport to another panel.

The native `--content-binding`, `--composition-panels`, `--matrix-bindings` and `--preview-bindings` flows verify
real tab input, independent instances, target deletion/history, layout reload, reconnect and
project fencing through isolated production hosts. Fixtures await the exact persisted panel
identities before reloading, rather than a stale layout that happens to mention one target.
The owning progress receipt records which checks have actually passed.
