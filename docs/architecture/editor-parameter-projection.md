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
instance and its independently selected off-page instance. The generated
protocol carries those bindings and the engine event boundary. The app's
`composition/parameter-projection.ts` maps typed values onto its existing read
projection; transform semantics and validation remain in Rust.

A disconnected/replaced/loading/resynchronizing session cannot project live
values. A committed graph older than a backend snapshot cannot overwrite that
snapshot. A local canvas gesture retains its existing transient presentation
and acknowledgement path while active. Its applied receipt raises the event
fence before the gesture's presentation is cleared, so a delayed graph batch
cannot rewind an accepted final transform. Editor observation still refreshes
backend-dependent metadata and authoritative snapshots; it is no longer the
only way Inspector geometry reaches Composition controls.

New editors can use the same Golden reader with backend-owned typed bindings.
Matrix and Screen bindings have not yet been added. This presentation mechanism
does not certify native rendering cadence or remove render-plan preparation
latency; those remain separate acceptance requirements.
