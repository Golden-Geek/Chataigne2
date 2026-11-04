# Matrix output exposure

MapGyver owns export policy in `matrix/outputs.rs` and public media admission in
`media/visibility.rs`. Golden owns typed Parameters, history and the reusable GPU runtime.
Layer/Group channels remain real stable internal identities; hiding an output never deletes
or recreates its channel node.

Each Layer and Group has a real **Expose Channel** Boolean Parameter, default **false**.
It projects through Rust-owned `CompositeSettings.expose_channel` and the generated protocol.
The existing Blend Parameter is presented as **Blend Mode**; no second blend model is added.
Configuration intents edit the same Parameters as the generic Inspector and script callers.

Public channel pages, choice queries, manager snapshots and virtual Reference channels honor
exposure. Final remains available. Direct channel inspection reports hidden outputs as
unavailable. Authored pinned identities remain intact, including while unavailable: validation,
copying and cycle checks must read authored dependencies rather than discard hidden edges.
Runtime `MediaSource::capture` applies availability separately and guards visibility changes.

Capture retains the complete internal Group/Layer graph needed by Final. Hidden-only public
demand captures a missing record and does not activate the producer graph. Renderer output
routes use a transparent missing target for hidden roots; Screen/preview routing excludes
internal textures, including when Final simultaneously needs that same texture. The additional
missing pass is charged to the existing plan limits. Internal passes still composite normally.
Retired producer metadata preserves its captured exposure policy: deletion cannot make an
unexposed internal channel public while an outgoing clip keeps it alive.

Matrix viewport DTOs resolve Reference chains at the app boundary. `source_label` is the
effective media label followed by `[channel]`; the grid displays that backend-owned value.
`source_preview` is the effective rendered ChannelReference. Enabled intrinsic-resolution
Reference aliases share their target; fixed-resolution or disabled References retain their
own rendered identity, preserving Fit Mode and enablement. Independent copies keep their own
identities. These fields are
runtime projections, never authored settings or persisted preview handles. Source loss clears
both fields; rename, Undo and project reload resolve current metadata through the bounded
viewport query. The authored Cell source remains unchanged.

`CellPreviews.svelte` groups visible Cells by that effective identity, using one native preview
registration per source and Golden's bounded repeated-image placement. Each tile reserves a
label strip and follows the grid's existing compressed scroll projection. CSS-relative image
rectangles are measured presentation coordinates, not authored fixed-pixel layout settings;
row/column/label spacing remains in rem. Negative partial rectangles stay intact until the
host clips them, preserving the corresponding source crop. Native thumbnails do not launch
Cells or change their authored source.

Golden preview version 4 carries passive tile rectangles and fixed repetition capacity. Its
completion sample and visibility guard avoid per-Cell source, decoder, upload or dependency
duplication. Geometry-only updates reuse bindings; backing and image passes share the tile
set instead of covering the full grid. Duplicate views of one channel count once for source
admission. Host metadata separately admits at most 2,048
tile rectangles and 4,096 reserved slots per layout. UI fallback is confined to each tile.

The desktop capture policy now admits at most 128 logical channel definitions, including
procedural channels and Reference dependencies. This metadata work does not admit native
inputs: physical texture/receiver admission remains eight, with 24 passes, 128 dependencies
and the existing memory/retirement limits. The former capture limit of eight conflated those
separate units. A nine-color software plan retains all nine correct clears with no external
textures; a ninth physical texture is still rejected.

**Registration limit correction:** Golden's shared default is 64 registrations, but MapGyver's
desktop host explicitly negotiates **eight**. Earlier Cell-tile notes incorrectly described
the shared default as the app's limit. This limit has not been raised. The current one-source-
per-registration grid therefore cannot install its ninth source: the native nine-source test
fails with one unregistered tile. The next implementation must carry source groups inside
one logical grid registration, preserving the app's eight-registration cap. The passing
two-source grid and the software nine-color plan do not qualify that missing behavior.

The actual native grid passes guarded foreground captures for two shared sources/four Cells,
live color changes, source loss and Undo, scroll withdrawal/recovery and project New. Source
grouping, clipping and same-generation updates pass software checks; both GPUs pass the native
repeated-image primitive. This is an admitted-scene checkpoint, not full Matrix qualification.
Mixed/large distinct-source snapshot admission, retained GPU completion and the full revised
native cohort remain required; a viewport beyond the active source budget is not accepted by
this evidence. Generic Inspector exposure/discovery still needs actual-app acceptance.

M11/8 remains **Active**. Backend exposure and native Cell tiles/labels are implemented and
verified within the admitted scene. Mixed/large distinct-source snapshots, actual Inspector
exposure acceptance, complete sparse/source-loss/preview retirement and the revised full
native cohort remain required. The desktop's eight-registration budget is unchanged. Thumbnail admission
must continue sharing sources/decoders rather than duplicating them per Cell.
