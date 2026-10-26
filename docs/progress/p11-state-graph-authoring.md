# P11 State graph authoring and native adapter qualification

The app-owned State graph operation owns mutation policy; Golden provides transactions, session
admission and the canvas. This delivers the State Machine adapter portion of P11's graph
checkpoint after `cd8b4dad`. **P11 remains 4/6 checkpoints (67% rounded); whole-plan coverage
remains 59%.** P03 is complete with zero remaining closure packages.

## Ownership and behavior

`systems/state_machine/runtime/src/authoring.rs` defines Rust requests, committed identities and
socket constants. Its supported protocol exporter generates all nine UI contract files.
`integration/state/authoring/` validates manager and State NodeId/UUID pairs, direct membership,
permissions, finite coordinates and bounded requests. Multi-state movement validates every target
before any queued edit; connection creation inserts one complete detached transition subtree.
The existing app-operation transaction provides atomic rejection and one undo group.

The panel delegates to `stateGraphEdits.ts`, which captures its session, snapshot generation,
manager and State identities before waiting. Shared bounded session admission replaces its
private geometry promise chain. Confirmed edits participate in save/history/project barriers;
queued edits cannot migrate to a replacement owner. Results and errors reach the canvas rather
than being discarded. The session snapshot generation is now reactive, so unchanged identities
in a replacement full snapshot still retire pending endpoints.

The panel is 759 lines after removing layout-parameter and transition-construction sequences.
The backend operation is 203 lines and the UI controller 165. State creation's existing placement
path is outside this change; this does not claim all State authoring has been migrated.

## Qualification

Six backend cases cover atomic multi-move/history, stale/duplicate/non-State rejection, sizing
and metadata, complete connections, invalid sockets and a read-only later target rejecting the
entire move. Eight UI controller cases cover captured coordinates/identities, backend/malformed
reply failure and recovery, session/manager/snapshot replacement, barriers, request mapping and
disposal. The State Machine runtime's 16 existing unit cases also pass.

The owned headless native workflow mounts the actual StateMachinePanel against the default
Chataigne executable over authenticated HTTP/WebSocket, using isolated project/app-data paths.
Its two-state fixture passes four correctness groups:

- Multi-state movement, rendered coordinates and one-group undo/redo.
- Collapse and directed connection with exact persisted transition/target UUIDs.
- Save/reload, unchanged-identity snapshot cancellation and panel remount cancellation.
- Read-only rejection without a committed move prefix or history change, rendered rollback
  and visible failure feedback.

The fixture invokes the same public component operations as keyboard handlers and native graph
controls and inspects focus/layout/state. It never synthesizes keyboard or mouse input or controls
the desktop. Physical keyboard/native-webview usability remains P17. The production fixture type
check reports zero errors/warnings. All 410 UI unit cases pass (27 audio / 58 graph / 191 Golden /
134 app); app Svelte check, generated contract comparison and strict app all-target Clippy pass.

Preserved early harness failures exposed Windows Node entrypoint handling and a wrong HTTP base
URL. Node cannot consume the canonical Windows verbatim path; the launcher now uses the normal
workspace path. The detailed readiness failure reported HTTP 404 because the fixture omitted
`/api/ui`. Its corrected run passes; the failure is retained with the final evidence.

Evidence: [native workflow](evidence/p11-state-graph-native-2026-10-26.jsonl),
[HTTP-base failure](evidence/p11-state-graph-native-failure-2026-10-26.jsonl), and
[source/build/check record](evidence/p11-state-graph-source-2026-10-26.json).

```powershell
$env:CARGO_INCREMENTAL='0'
cargo test --locked -p Chataigne2 state_graph_authoring -- --skip native_browser_adapter
cargo test --locked -p chataigne_state_machine --lib
npm test
npm run check --workspace chataigne-ui
npm run check:browser:graphs --workspace chataigne-ui
cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings
cargo build --locked
# Run the already-built app test executable directly after the default build, so cargo test
# does not replace the default executable before exact-artifact qualification:
& target/debug/deps/Chataigne2-<test-hash>.exe state_graph_authoring_native_browser_adapter --ignored --nocapture
```

The two-state native fixture is correctness evidence, not the required actual-app scale matrix.
Alchemist still needs captured queue ownership and returned connection failures, with a cohesive
split of its oversized panel. Spatializer's empty GraphCanvas/custom world integration needs its
own read-only qualification. Actual-app scale and composed workflows remain open under P11;
shared 100k-node timing does not substitute for those checks. No checkpoint was added or closed.
