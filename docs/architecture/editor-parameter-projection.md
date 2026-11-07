# Live editor Parameter presentation

Golden UI owns `store/editor-parameter-projection.ts`: a read-only, typed view of
the session's existing Parameter drafts and committed graph. It performs exact
UUID lookup and never enumerates a document, mutates authored state, or chooses
product defaults. Inspector drafts are visible before a request completes.
Cancelled/rejected drafts disappear through the existing session edit owner.
Ordinary full-value and component Parameter writes publish through
`store/session/parameter-write-preview.ts`, wired at the shared `ui-intents.ts`
boundary. Inspector and other existing controls retain their original dispatch,
history and validation path. A component preview composes presentation axes;
the authored intent still changes only its requested backend component. Preview
capacity exhaustion falls back to ordinary dispatch and never drops a write.
Its Svelte companion owns the reactive applied-receipt fence. Editors pass its
observer through the ordinary authored operation and use its bounded reader;
they do not implement transport ordering or acknowledgement fences themselves.

The backend owns bindings and snapshot event fences. MapGyver's bounded
Composition editor query returns the exact Parameter UUIDs for each visible
instance and its independently selected off-page instance. Matrix queries do
the same for visible/independently selected Layers and Cells, with one Layer
binding for the Cell canvas. Screen editor queries return the first 128 Surface
bindings plus an independent selected Surface; Surface Source queries return
only their selected binding. Crop origin/size, four corners and opacity use the
same shared reader. The generated
protocol carries those bindings and the engine event boundary. The app's
`composition/parameter-projection.ts` maps typed values onto its existing read
projection; transform semantics and validation remain in Rust.

A disconnected/replaced/loading/resynchronizing session cannot project live
values. A committed graph older than a backend snapshot cannot overwrite that
snapshot. A local canvas gesture retains its existing transient presentation
and acknowledgement path while active. Its applied receipt raises the event
fence before the gesture's presentation is cleared, so a delayed graph batch
cannot rewind an accepted final transform. Receipt identity uses a weak session
reference so a retained panel cannot pin a replaced session's graph. Editor observation still refreshes
backend-dependent metadata and authoritative snapshots; it is no longer the
only way Inspector geometry reaches Composition controls.

Optional typed reads and Parameter enabled-state projection preserve the Cell
Size Parameter's natural-source mode without inventing an authored size. App
projections keep membership, natural source extents and other metadata from the
authoritative DTO. Direct geometry operations write only their requested
position/size fields, preserving unrelated settings and concurrent size changes
during translation.

Surface geometry operations write only their affected crop/corner Parameters;
Fit/Fill writes only the quad. Other authored f64 values never round-trip through
the f32 render projection merely because geometry moved. Mapping validity remains
backend-owned, including temporarily invalid editable warps.

Media Preview owns no authored geometry copy: it passes stable channel identity
to NativePreview and reads renderer extents for presentation. Its bounded reader
already observes live edit groups and retains same-project native leases across
refresh/reconnect. No extra geometry adapter belongs in that panel.

New editors can use the same Golden reader with backend-owned typed bindings.
This presentation mechanism
does not certify native rendering cadence or remove render-plan preparation
latency; those remain separate acceptance requirements.
