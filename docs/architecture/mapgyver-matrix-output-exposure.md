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

The generic Channels Inspector consumes the same bounded public channel page rather than
raw document children. It renders the admitted identities through Golden's normal child
Inspector; it does not infer exposure from Parameters. While metadata refreshes, it retains
only page membership within the same session/project/media/page so live Parameter editors
keep their DOM and input capture. Definitions remain live Golden nodes. Project replacement,
disconnect and page changes fence that membership. Actual native WebView tests verify default
Final-only discovery, Layer/Group toggles, Blend Mode, Reference discovery and Undo/Redo with
stable identities. Ordinary channel color editing, CRUD/history and 37-channel paging also pass.

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

`CellPreviews.svelte` groups visible Cells by that effective identity, using one logical native
preview registration for the grid and Golden's bounded per-source repeated-image placement. Each tile reserves a
label strip and follows the grid's existing compressed scroll projection. CSS-relative image
rectangles are measured presentation coordinates, not authored fixed-pixel layout settings;
row/column/label spacing remains in rem. Negative partial rectangles stay intact until the
host clips them, preserving the corresponding source crop. Native thumbnails do not launch
Cells or change their authored source.

Golden preview version 5 carries typed source groups, passive tile rectangles and fixed
repetition capacity inside one registration. The parent source identifies the owner without
creating image demand. Completion waits for the parent geometry marker and every child source
placement. Geometry-only updates reuse bindings; withdrawn groups clear retained placements.
The generic UI normalizes the supplied lifetime value through a Svelte derived boundary:
refreshing a parent viewport DTO with the same project identity does not recreate its lease.
Actual sparse-grid loss/Undo and guarded native Fit/source-loss checks verify this lifetime.
The parent paints no full-grid opaque backing. This avoids per-Cell source, decoder, upload
or dependency duplication. Host metadata admits at most 2,048 groups and tile rectangles and
4,096 reserved slots per layout. UI fallback is confined to each tile.

The desktop capture policy now admits at most 128 logical channel definitions, including
procedural channels and Reference dependencies. This metadata work does not admit native
inputs: physical texture/receiver admission remains eight, with 24 passes, 128 dependencies
and the existing memory/retirement limits. The former capture limit of eight conflated those
separate units. A nine-color software plan retains all nine correct clears with no external
textures; a ninth physical texture is still rejected.

**Registration limit correction:** Golden's shared default is 64 registrations, but MapGyver's
desktop host explicitly negotiates **eight**. Earlier Cell-tile notes incorrectly described
the shared default as the app's limit. This limit has not been raised. The former one-source-
per-registration grid failed to install its ninth source. The grouped implementation now
admits all nine sources in one installed registration with no fallback in the actual app.
The exact nine-source/twelve-Cell pixel cohort now passes on explicitly selected Intel and
NVIDIA adapters, alongside independent complete Matrix Final output. Both pass live edits,
source loss/Undo, fixed-resolution Reference Fit black areas, scroll and New. Eighteen guarded
native screenshots retain the original PID/foreground and pixel-tolerance checks. Source
labels have two complete lines and a full tooltip, with the same reserved strip in native
tile geometry. The probe follows the image center when that strip changes.

Actual native WebView checks on both adapters keep a 1,000×1,000 document sparse at six Cells,
mount only 64–100 addresses and share two source bindings in one lease. Source-loss/Undo
retains that lease; scroll and tab changes withdraw/recover it. New removes the preview and
receives a newer empty backend observation set. The host retains its clearing presentation
window; UI lease withdrawal does not imply runtime destruction.

The complete five-test native Matrix suite separately passes 240 complete images, original
large-grid/burst/fade checks and all 16 balanced GPU owner retirements on both adapters.
Public export observations opt in explicitly while default-hidden exposure retains its own
native test. Original pixel, admission, resource and watchdog limits remain unchanged.

**M11/8 Accepted.** The [acceptance receipt](../progress/evidence/mapgyver-m11-cell-tiles-2026-11-04.json)
maps each requirement to its proof. Global physical/timing/process and final product
qualification retain their separate open gates; this feature acceptance does not close them.
