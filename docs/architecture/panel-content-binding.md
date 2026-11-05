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

Composition, Media Preview and Inspector publish the same capability and have no body lock.
Composition stores the last successful target even while unlocked. Media Preview retains
its existing explicit lock/channel/grid/page behavior. The Matrix spatial adapter must use
this same capability and relevant-target rule when integrated under M12/6–7; this slice does
not claim that future integration or complete M12/6 acceptance.

The native `--content-binding`, `--composition-panels` and `--preview-bindings` flows verify
real tab input, independent instances, target deletion/history, layout reload, reconnect and
project fencing through isolated production hosts. Fixtures await the exact persisted panel
identities before reloading, rather than a stale layout that happens to mention one target.
The owning progress receipt records which checks have actually passed.
