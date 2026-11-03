# M04/5 source UI acceptance

MapGyver owns source/variant policy, creation permissions and contextual asset operations.
Golden owns the manager tree, shared selection, typed Inspector widgets, child collections,
history and persistence. **Accepted 2026-11-03** against the ordinary bundled app on `mapgyver`.

The retained `tests/browser/sources.mjs` interaction matrix now uses the revised typed model.
All authoring uses actual DOM controls; backend reads only inspect committed state. The old
custom source forms and manager channel rows are replaced by real Channels children and generic
Parameter widgets. Resolution/Sampling apply to the media, while each variant retains its own
color, Picture asset or exact Spout sender. Removed legacy Spout format/encoding/alpha controls
are covered by M04/4's native discovery contract rather than recreated in the Inspector.

| Requirement                          | Verified interaction                                                                                                                                                                                                                                   |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Source CRUD and independent variants | Create Solid Color, Picture and Shared Texture through the manager catalog; rename media and fresh first channels; add variants, edit RGBA including alpha, reorder/delete and restore exact catalog with Undo. Peer source settings remain unchanged. |
| Typed shared/variant settings        | Shared Resolution and Sampling affect both variants; enable/edit/disable, integer pixels, Undo/Redo and procedural/Matrix defaults pass. Sender-only Shared Texture fields edit independently.                                                         |
| Picture operations                   | Contextual import-and-assign, generic asset Project path relink, generic reference Clear/reselection, media removal preserving the asset and peer, and atomic top-level import with one-step Undo pass.                                                |
| Hierarchy and capabilities           | Manager creation/context Add/Paste omit channels and automation; declared Parameter folders have no Add action. Channels use the generic collection, stable identities, copy/history and a 32-entry rendering bound with exact off-page reads.         |
| Rejection and lifetime               | Last-channel deletion rejects atomically; its following Undo restores the deleted peer. Context menus do not toggle enabled state. Browser reload preserves the exact catalog; no ordinary manager forms or unexpected page errors/rejections occur.   |

Three product defects found by this check were fixed at their owning boundaries:

- Fresh app-created source variants receive rename/remove permissions immediately, rather than
  depending on project reload classification. Composition/Matrix fixed outputs remain protected.
- `InspectChannel` is an immutable background query. A deleted owner/channel returns a nullable
  snapshot, so queued Inspector reads retire quietly; wrong existing node types/owners still
  reject. Rust owns the generated nullable TypeScript contract. Reads/rejections preserve history
  and project data; Undo restores the original channel identity and snapshot.
- Golden's enable button ignores secondary mouse presses. Previously right-clicking it to open
  a context menu toggled the node and inserted an unexpected history entry. The actual source
  matrix opens channel menus from that button and verifies unchanged state plus exact Undo.

Verification completed before acceptance:

- `cargo test --locked -p MapGyver --lib`: **341 passed, 2 hardware tests ignored**, including
  generated-contract equality and fresh-permission/deleted-read regressions.
- Strict MapGyver library/test Clippy: passed. Both required Cargo formatters and diff check pass.
- `npm run test --workspace golden_ui`: **233 passed**, 44 files.
- `node --test apps/mapgyver/ui/src/lib/media/tests/*.test.ts`: **14 passed**.
- MapGyver and Chataigne Svelte checks: **zero errors and warnings**.
- Ordinary bundled review binary rebuilt successfully using
  `cargo rustc --locked -p MapGyver --bin MapGyver -- -C extra-filename=-codex-review`.
- `MAPGYVER_TEST_BINARY=target/debug/deps/MapGyver-codex-review.exe` with
  `node apps/mapgyver/tests/headless/media.mjs --source-ui`: passed in **10.10 seconds**, exactly
  one intentional guard rejection, zero page errors, normal host exit. The screenshot was visually
  inspected: shared media controls and typed per-variant children occupy the shared Inspector.
- The same rebuilt app with `--channels-ui --resolution-ui --catalog-ui`: all five bundled
  interaction checks plus headless source history/Save/New/Load passed in **11.90 seconds**.

Local reproducible receipts are `target/mapgyver-source-ui-ported.log`,
`target/mapgyver-source-ui-final-baseline.log`, `target/mapgyver-source-ui-final-tests.log`,
`target/mapgyver-source-ui-final-clippy.log`, `target/mapgyver-source-ui-golden-tests.log`,
`target/mapgyver-source-ui-final-unit.log` and `target/mapgyver-media-source-inspector.png`.
The checked-in fixtures retain the assertions; transient logs/screenshots are not source assets.

This accepts M04/5 only. M04/6 retains complete revised caller parity, missing/relinked/replaced
Picture behavior, duplication/Undo/Redo/project roundtrip and functional native resource
recreation. No GPU rendering or physical timing is inferred from the headless browser checks.
