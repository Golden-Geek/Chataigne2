# Media Preview ownership

MapGyver owns selected-media resolution and panel presentation choices. Golden owns docking,
transport, native preview geometry and completion-based view retirement. The Inspector keeps
authored Parameters and Channels; it no longer embeds media images.

`media::preview_selection` resolves the nearest actual Media/Reference ancestor from a selected
node UUID. The immutable `InspectMediaPreview` operation returns a bounded channel page plus
one independently resolved selected channel, including when that channel is outside the page.
Explicit channel choice, selected descendant channel and first available source channel are
resolved in that order by the backend. Reference identities use the existing public media
contracts. Reads neither change history nor checkpoint the graph.

Each `MediaPreviewPanel` stores only presentation choices through Golden's public panel-state
API: locked media/project UUID pair, channel UUID, single/grid mode and page. Locks are scoped
to the persisted project-root UUID and reject a different project, including a New project.
Follow mode sends the shared workbench selection UUID without walking a UI copy of the graph.
Late replies are fenced by session, generation and history. Valid channel identities stay
mounted during parameter edit sessions; reconnect/project replacement immediately hides them.

Grid pages contain at most four tiles. Paging exposes every channel without creating an
unbounded number of native views. The existing host's eight-view and source/resource admission
limits remain unchanged; excess visible demand uses the existing unavailable state. Multiple
panels are independently dockable, removable and restored by Golden's saved layout.

`ChannelPreview` fits native geometry to the available panel while preserving source aspect.
The shared `NativePreview` retires registration when explicitly hidden or outside its viewport,
and re-registers on return. Its existing retirement covers remain alive until the host confirms
the replacement frame. Closing one panel does not own or stop another consumer's source lease.

Verification state and remaining native/output acceptance belong to the
[status ledger](../progress/mapgyver-status.md), not this boundary description.
