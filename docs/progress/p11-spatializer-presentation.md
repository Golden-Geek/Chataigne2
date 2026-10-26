# P11 Spatializer presentation owners

Spatializer remains app-owned. Golden provides the graph document, canvas, camera and transport;
the app supplies its custom world and endpoint behavior. This follows `1f4762ff` and starts the
remaining Spatializer consumer work inside the existing graph checkpoint. **P11 remains 4/6
(67% rounded), whole-plan coverage 59%; P03 has zero packages remaining.**

The panel shrinks from **2,561 to 924 lines**. Separate owners read the public graph (187 lines),
calculate display geometry and debug overlays (607), retain existing pointer scheduling/previews
(438), and render the SVG world with its own scoped styles (522). Types/constants remain small;
one graph-to-spatial coordinate conversion serves both creation and pointer paths. No app-owned
Spatializer behavior is moved into Golden. Source inventory is **2,245 / 25 oversized**, reducing
the oversized count by one. No touched source exceeds 1,000 lines.

## Native preservation fixture

The production Svelte fixture mounts the actual SpatializerEditorPanel against the default
executable through authenticated HTTP/WebSocket. Isolated app-data paths and an owned headless
browser keep the user's desktop untouched. The reusable test host records executable and fixture
asset hashes, failures and results, and cleans up its child processes. It does not control input.

Six groups verify:

- An empty Golden graph document still renders the app's three endpoint controls and two Voronoi
  cells, with correct world styling and a real canvas viewport.
- Selection publishes the actual inspector, computed value rows and debug overlays.
- Authoritative parameter publication updates SVG geometry, and the production Home command
  reveals the focused world endpoint.
- All four influence modes replace their radius/cell presentation correctly; target-centric
  matrix replacement retains the expected inspector pair identities.
- 3D source data projects XZ correctly while the authored middle coordinate remains seven.
- Full snapshot refresh and unmount/remount retain the expected source, inspector and debug view.

The fixture reuses the module's two backend-created endpoints and creates one further target.
Generic authored edits arrange the test data. **Those fixture edits do not qualify the panel's
endpoint mutation API, pointer gestures or keyboard editing.** No synthetic mouse/keyboard input
or success-path diagnostic backend snapshots are used. This is a three-endpoint correctness scene,
not scale, full-shell or physical/native-webview usability evidence.

Four early browser failures are preserved: the initial setup added three endpoints to the backend's
two defaults; a viewport assertion incorrectly measured the deliberately small overflow-visible SVG
basis; text assertions included swatch whitespace; and a configuration lookup omitted its full DSL
declaration prefix. These were fixture corrections. Product timing/density gates were not relaxed.

## Checks and next work

All **425 UI cases** pass (27 audio / 58 graph / 193 Golden / 147 app). App and expanded graph-fixture
Svelte checks report zero errors/warnings. Both Cargo formatters pass. Rust runtime and protocol
are unchanged; their preceding milestone evidence is retained rather than represented as a new run.

All 97 original top-level arrow-function initializers retain their normalized AST token bodies across the
split. This is a move/review check, not proof of new mutation ownership. The final default locked
build passes in **4m01s**, rebuilding the frontend. Its executable SHA-256 is
`88250516d87a1e29dcab743e1a51551fdaaec05838cf0ac76c15684d25a8faa7`.
That exact executable passes all [six native groups](evidence/p11-spatializer-presentation-native-2026-10-26.jsonl).
The [four preserved fixture failures](evidence/p11-spatializer-presentation-native-failures-2026-10-26.jsonl)
and [source/build/check record](evidence/p11-spatializer-presentation-source-2026-10-26.json)
identify the actual inputs, machine and assets.

```powershell
npm test
npm run check --workspace chataigne-ui
npm run check:browser:graphs --workspace chataigne-ui
cargo build --locked
node apps/chataigne/ui/tests/browser/spatializer-native.mjs
```

The next implementation belongs at the app-owned Spatializer authoring boundary: backend intents
must own endpoint policy and preserve the non-displayed 3D coordinate. UI requests must capture
session, module/endpoint identities and snapshot generation before bounded admission. Migrate
creation, pointer ownership and keyboard operations, and qualify read-only/rejected/stale requests
through the actual adapter. The current private parameter writer and global edit-session queue are
not accepted as the final architecture. Actual-app scale and composed workflows remain in the two
open P11 checkpoints. Native physical usability retains P17 and full-product throughput P16.

The subsequent [authoring milestone](p11-spatializer-authoring.md) implements and qualifies that
backend, bounded gesture and keyboard migration. Actual-app scale and composed workflows remain.
