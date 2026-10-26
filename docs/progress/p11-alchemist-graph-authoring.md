# P11 Alchemist graph authoring and adapter qualification

Alchemist owns Formula mutation policy and its editor adapters. Golden supplies transactions,
bounded session admission, transport and the shared canvas. This continues P11 after `d6696752`;
it does not close the graph or composed-workflow checkpoint. **P11 stays 4/6 (67% rounded),
whole-plan scope coverage 59%.** P03 has zero remaining closure packages.

## Delivered boundary

The generated `chataigne.alchemist/editGraph` contract owns movement, sizing, naming, collapse,
enable and deletion. Requests carry Formula and target NodeId/UUID pairs. The backend checks all
targets, direct membership, permissions and finite values before queuing changes. Deletion gathers
incident edges in one direct-sibling pass; no per-node full snapshot rebuild is added. Existing
app-operation transactions provide atomic rejection and one undo group.

The panel composes focused preview, clipboard, item creation, camera, graph request and mutation
admission owners. The separate property sidebar uses relative sizing and measured viewport insets.
The panel shrinks from 2,091 to 993 lines; every new owner and test stays below 1,000. The source
inventory is 2,231 / 26 oversized, reducing the oversized count by one without adding an exception.

Graph requests capture their complete identities/coordinates before shared session admission.
Pending work cannot transfer to another Formula, session or snapshot generation. Read-only changes
and unmount cancel queued dispatch. Confirmed work participates in save/history/project barriers;
the private promise queue and edit-session polling are removed. Connection callbacks return their
backend promise through both app components to the shared canvas. Rejection and malformed replies
remain failures; a later successful operation can recover normally.

Preview demand, activity retention, lane selection, clipboard fences and per-Formula cameras keep
their previous behavior. Editable-copy creation still works when a built-in is presented through a
processor without a directly resolved Formula. Item/property creation uses the existing public
creation boundary; this does not claim all creation policy has been migrated.

## Replacement race and qualification

The first native run passed movement, undo/redo, collapse, connection and save/reload but timed out
waiting for the read-only replacement in the client. A diagnostic rerun independently observed the
replacement's backend layout and passed. Investigation found a deterministic transport defect:
the subscription coordinator discarded resync notices received during an in-flight snapshot.
The regression expected a second request after 64 coalesced notices; the old code issued only one.

The coordinator now retains one pending invalidation and requests a fresh snapshot before
resubscribing. Disconnect/disposal retire that pending request. Two unit regressions qualify
coalescing across a reset event clock and retirement. The diagnostic backend read was removed from
the native success path before repeated qualification. The first failure is preserved; repeated
native success does not substitute for the deterministic regression.

That fix did not resolve the native stall. The preserved wire trace then identified the owning
server defect: successive project loads both had event boundary `{tick: 2, micro: 0, seq: 0}`.
An idle subscription's cursor exactly matched the replacement clock, so neither the ahead-of-clock
check nor replay revealed the replacement. Subscription state now retains the authoritative project
generation and requests resync on generation change independently of event time. The host regression
uses a real ProductionRuntime replacement with an equal cached boundary and verifies one resync,
then paused delivery until subscription recovery. No success-path polling or diagnostic snapshot
is used to hide the replacement failure.

Five new backend graph cases and the Formula operation preservation suite pass: **24 passes /
two existing ignored native cases**. Nine app controller cases cover captured inputs, returned
connection failures, malformed results/recovery, owner/read-only replacement, barriers, bounded
admission, disposal and generated request mapping. All **421 UI cases** pass (27 audio / 58 graph /
193 Golden / 143 app). App and graph fixture Svelte checks report zero errors/warnings; all five
changed generated contracts match the Rust exporter. Strict app/all-target Clippy passes.

The owned browser mounts the actual AlchemistEditorPanel against the default executable over
authenticated HTTP/WebSocket. Its two-ANode fixture checks authoritative multi-move and one-group
undo/redo, collapse and exact connection UUIDs, save/reload, unchanged-identity full-snapshot and
unmount cancellation, and read-only rejection with no move prefix/history entry, rendered rollback
and visible feedback. It invokes public component operations, without synthesizing input or taking
control of the desktop. Physical input/native-webview usability remains P17.

Evidence: [native runs](evidence/p11-alchemist-graph-native-2026-10-26.jsonl),
[preserved replacement failure](evidence/p11-alchemist-graph-native-failure-2026-10-26.jsonl), and
[source/build/check record](evidence/p11-alchemist-graph-source-2026-10-26.json).

```powershell
$env:CARGO_INCREMENTAL='0'
cargo test --locked -p Chataigne2 systems_alchemist_formula::operations -- --skip native --nocapture
npm test
npm run check --workspace chataigne-ui
npm run check:browser:graphs --workspace chataigne-ui
cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings
cargo build --locked
# Run the already-built test executable directly after the default build.
& target/debug/deps/Chataigne2-<test-hash>.exe formula_graph_native_browser_adapter --ignored --nocapture
```

This small native fixture is correctness evidence. Replacement during the complete unsubscribe/
snapshot/re-subscribe interval still needs explicit qualification; the wire subscribe cursor carries
event time rather than a project-generation token. Spatializer's actual empty-document/custom-world
integration, actual-app scale qualification and complete composed workflows remain open under P11.
Shared 100k-node canvas results do not replace those checks. Full-product throughput remains P16.

The final default locked build passes in **2m24s**. Its binary SHA-256 is
`de7d8ab90f19780f6bb4f4c50e90f6ee91348342b26871eda2bb8d870477bd81`.
That exact executable passes **three Alchemist repetitions / 12 correctness groups** and the
[State Machine regression](evidence/p11-alchemist-graph-state-regression-2026-10-26.jsonl) / four
groups. The standalone transport suite passes **75 cases / one existing manual benchmark ignored**,
including the equal-clock replacement regression. Both Cargo formatters, explicit app graph-module
formatting, UI formatting and whitespace checks pass. Earlier clock/resync failures remain preserved.
