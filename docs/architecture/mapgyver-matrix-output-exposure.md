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
`source_preview` is the effective leaf ChannelReference, so cells sharing a Reference target
identify one preview source. Independent copies keep their own identities. These fields are
runtime projections, never authored settings or persisted preview handles. Source loss clears
both fields; rename, Undo and project reload resolve current metadata through the bounded
viewport query. The authored Cell source remains unchanged.

M11/8 remains **Active**. Backend exposure and Cell labels/preview identities are implemented.
Visible native Cell snapshots, actual app Inspector/grid exposure acceptance, complete sparse
source-loss/preview retirement and the revised full native cohort remain required. The
window's 64-view budget is unchanged. Native thumbnails must reuse bounded presentation rather
than create one independent live source/decoder for every visible Cell.
