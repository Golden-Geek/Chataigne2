# Matrix thumbnail presentation

MapGyver's `ui/src/lib/matrix/cell-previews.ts` owns visible Cell apertures and canonical
source grouping. Golden Video owns fitting, clipping and drawing those shared textures.
Thumbnail presentation does not edit media Fit Mode, Cell placement or Matrix output geometry.

Each Matrix group requests `RegionFit::Contain` through the Rust-owned native preview
protocol, version 6. TypeScript is generated for both the public preview package and the native
qualification fixture. Ordinary generic callers can explicitly request `Stretch`; no persisted
setting or UI-owned source-size cache is introduced.

The host validates and clips each tile, retaining its normalized destination and source crop.
The render worker reconstructs the full aperture from that pair, fits the **current texture's**
actual aspect into it, then intersects the result with the clipped destination. Fitting after
cropping would change scale while scrolling; fitting from delayed UI status would briefly use
stale dimensions after a source resize. The worker path avoids both problems. An aperture
showing only an empty margin contributes no image draw. Existing preview backing supplies
the deliberate black margins and preserves alpha composition.

The fit choice is immutable on a bounded `RegionPlacement`. Changing it replaces routing and
withdraws old tile geometry. Scrolling/resizing update existing placement metadata without
allocating another texture, source or decoder. A changed source descriptor naturally refits
on the next rendered plan. The existing 2,048-tile capacity, dependency, producer and byte
admission bounds remain authoritative; work scales with visible demanded tiles.

Software tests cover pixel aspects, partial clipping, empty margins, live dimensions, routing
withdrawal, maximum metadata capacity and generated parity. Both-GPU native tests verify
transparent margins, half-alpha source pixels, 256 shared repetitions and independent outputs.
Actual MapGyver window checks additionally cover grouped grids, live resize, scroll, virtualized
reuse, viewport resizing and unchanged SDK output pixels. See the
[M12/9 evidence](../progress/evidence/mapgyver-m12-thumbnail-fit-2026-11-06.json).
