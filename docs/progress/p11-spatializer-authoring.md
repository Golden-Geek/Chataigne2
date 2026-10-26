# P11 Spatializer authoring

This milestone follows [presentation qualification](p11-spatializer-presentation.md). It replaces
the panel's private parameter writer with app-owned backend operations, captured bounded UI
admission, keyboard editing and one-group pointer gestures. P11's fixed checkpoint count remains
six: actual-app scale and composed workflows still need qualification.

## Ownership and behavior

`chataigne_spatializer_protocol` owns the Rust request/result DTOs and public TypeScript generator.
It depends on the generic model, serialization and type export crates, without importing the app
engine or moving Spatializer policy into Golden. Ten checked-in generated files match its export
after the repository formatter. The app registers `chataigne.spatializer/edit` through the public
app-operation boundary.

The backend checks module and endpoint IDs/UUIDs, current XY/XZ space, direct ownership, manual
writable fields and finite values. Multi-endpoint moves validate every member before applying;
one read-only member cannot commit a writable prefix. XZ edits preserve the latest backend height,
including changes made by another caller during a gesture. Relative keyboard/radius operations
read current backend values. Creation supplies one complete detached target subtree using current
mode/dimension defaults and engine-owned naming, returning the exact committed identity.

The UI captures session, snapshot generation, module and coordinate space before shared bounded
admission. One-shot requests copy caller input. Gestures retain one in-flight request and one
latest unsent sample; they share one acknowledged history group. Snapshot replacement, unmount
and barriers retire pending work. Failures retire previews, surface status and release admission;
successful previews wait for the committed event boundary. Radius handles remain keyboard
reachable at zero. Arrow keys move endpoints or resize a focused radius; Shift moves farther.
Add target uses the current viewport center as a presentation hint.

Golden changes are public generic hooks: app-operation acknowledgement observation and an optional
canvas interaction-help string. Neither contains Spatializer domain policy. All touched sources
stay below 1,000 lines; the inventory is 2,252 sources / 25 oversized.

## Regressions found during qualification

A deterministic admission test first exposed a rejected BeginEdit that remained unnoticed until
a pointer sample arrived. Begin acknowledgement is now observed immediately, and shared terminal
cleanup ends the group once even when rejection overlaps confirmation.

The native workflow then exposed an open history group after a same-project snapshot refresh.
The UI had treated its projection epoch as the engine group's lifetime and skipped EndEdit.
A red regression observed only BeginEdit; cleanup now sends the unique matching EndEdit after an
acknowledged begin. The engine rejects a stale ID after replacement without closing another owner.
Native target creation subsequently passed undo/redo without merging into the old drag.

Save/reload exposed an independent persistence gap immediately after catalog creation, before
gesture or undo activity. Prepared-subtree commit notified observers about roots while some
already-decoded descendants never entered the immutable authored capture. The app actor regression
reproduced the missing-node save failure without a browser. Finalization now publishes its existing
collected node IDs to persistence, without a whole-engine scan or extra descendant observer events.
The owning Golden test compares live/captured codecs through creation and undo/redo and retains
an immutable prior capture.

An early native fixture selector also needed to target the coordinate-space span after adding the
toolbar button. Diagnostic runs narrowed save failure from the combined workflow to initial
module creation; their failures remain evidence, rather than being reclassified as successful runs.

## Verification

The Golden engine suite passes 635 cases with three existing ignored
manual measurements, including the prepared-descendant capture regression. All 36 Spatializer
backend cases pass, including production actor creation/save, atomic authoring, and preservation
of the existing bounded runtime, math, script and callback contracts. The current UI suite
passes 441 cases (27 audio / 58 graph / 194
Golden / 162 app), including 15 Spatializer admission/lifecycle cases. App and graph-fixture
Svelte checks report zero errors/warnings. Generated DTO freshness passes for all ten files.
Strict app/all-target Clippy, package-owned Prettier checks, both Cargo formatters and whitespace
checks pass. The normal locked build passes in **2m44s**, rebuilding the frontend with
`GC_SKIP_UI_BUILD` unset. Binary SHA-256:
`a9706de975f79c55ad5cf151be84beccd7979cbf59e4aae29ed1f2567a907dd1`.
Backend test and Clippy compilation used the explicit skip flag; they are not represented as
bundled-host builds.

That exact executable passes all **13 native Spatializer groups**: the original six presentation
groups plus stale-coordinate-space rejection, keyboard/gesture/history with a concurrent height
change, zero-radius keyboard editing, active-gesture refresh/unmount, backend creation with unique
identities/names and undo/redo, read-only replacement rejection/rollback, and saved-project recovery.
Save checkpoints also succeed before creation, after setup, after mode/layout/dimension changes,
and after gesture/history and radius editing. The same binary passes all four State Machine and
five Alchemist regression groups, including subscription-gap replacement recovery.

Evidence: [Spatializer native workflow](evidence/p11-spatializer-authoring-native-2026-10-26.jsonl),
[State regression](evidence/p11-spatializer-authoring-state-regression-2026-10-26.jsonl),
[Alchemist regression](evidence/p11-spatializer-authoring-alchemist-regression-2026-10-26.jsonl),
[seven preserved native failures](evidence/p11-spatializer-authoring-native-failures-2026-10-26.jsonl),
and [source/build/check record](evidence/p11-spatializer-authoring-source-2026-10-26.json).

```powershell
$env:CARGO_INCREMENTAL='0'
cargo test --locked -p golden_engine --lib
cargo test --locked -p Chataigne2 --bin Chataigne2 spatializer -- --nocapture
npm test
npm run check --workspace chataigne-ui
npm run check:browser:graphs --workspace chataigne-ui
cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings
cargo build --locked
node apps/chataigne/ui/tests/browser/spatializer-native.mjs
# Run existing native State/Alchemist regressions from the already-built app test harness.
```

The browser fixture mounts the production panel against the default executable through real
authenticated HTTP/WebSocket. Public component operations are shared with actual pointer/key
handlers; the fixture calls those operations and focuses/observes its own DOM without synthetic
input or desktop control. A held real resync callback lets an old-view gesture reach the actual
replacement backend, exercising rejection and preview rollback before normal snapshot recovery.
No backend response is forged. This small correctness scene does not qualify actual-app scale,
composed workflows, native-webview usability or physical input. Those retain P11/P16/P17 owners.
