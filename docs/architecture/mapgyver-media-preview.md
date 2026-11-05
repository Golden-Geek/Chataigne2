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
Late replies are fenced by session, generation, history and the latest selection request.
The last valid page in the same document stays displayed while selection/paging resolves;
superseded replies cannot replace it. Reconnect/project replacement immediately hides it.

Grid pages contain at most four tiles. Paging exposes every channel without creating an
unbounded number of native views. The existing host's eight-view and source/resource admission
limits remain unchanged; excess visible demand uses the existing unavailable state. Multiple
panels are independently dockable, removable and restored by Golden's saved layout.
Grid registrations belong to stable viewport slots rather than channel UUID keys. Changing a
slot's source reuses its native view and Golden's existing last-frame retention until replacement
is installed; document fences still retire stale views. This is presentation lifetime, not a
second media identity or authored setting.

`ChannelPreview` fits native geometry to the available panel while preserving source aspect.
The shared `NativePreview` retires registration when explicitly hidden or outside its viewport,
and re-registers on return. Its existing retirement covers remain alive until the host confirms
the replacement frame. Closing one panel does not own or stop another consumer's source lease.

Matrix Cell previews use one measured overlay beside the scrolling grid, inside a shared
clipped viewport container. The overlay does not participate in the grid's scroll extent.
Its tile positions project the measured scroll offset; it must not feed those measurements
back into normal-flow sizing. Otherwise fractional dock bounds can alternate scrollbar
presence and keep unrelated previews in the shared native session awaiting installation.

Golden's shared retirement layer composes historical coverage as one SVG luminance mask,
subtracting all retained live apertures together. A view's old cover must not crop a neighbor's
new aperture. Overlapping cutouts remain a union; hidden/removed/fenced views supply no cutout.
The existing retained-presentation predicate and confirmed-installation release are unchanged.
At most 64 cover rectangles and 64 live cutouts are reused, avoiding fragment growth during
layout churn. This coverage contract does not establish synchronized browser/native presentation.

Visible native layouts are immutable measured cohorts. The session dispatches each cohort from
a task after an animation callback, allowing the browser's rendering task to finish first;
see the [HTML rendering order](https://html.spec.whatwg.org/multipage/webappapis.html#update-the-rendering).
This is a rendering opportunity, not a compositor-present receipt. Stale source/session work
is cancelled before dispatch; disposal and empty-layout withdrawal bypass the rendering wait.
A suppressed frame has a bounded timeout and uses the existing unavailable/reconnect path.
Historical retirement paint uses Golden's background token, remaining opaque while matching
the host's surrounding backdrop instead of introducing black rectangles outside live apertures.

Verification state and remaining native/output acceptance belong to the
[status ledger](../progress/mapgyver-status.md), not this boundary description.

The [functional requirement audit](../progress/evidence/mapgyver-media-preview-native-2026-11-03.json)
links the backend, bundled and native workflows, including eight simultaneous views, actual
transport reconnect and independent Screen/Spout delivery through final preview closure.
Physical display/DPI and whole-app timing/resource qualification keep their own gates.
