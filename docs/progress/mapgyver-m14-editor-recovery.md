# MapGyver editor recovery (M14/1)

- A Source crop drag could leave its first accepted sample behind after one Undo when the
  connection closed mid-drag. Golden now retires the gesture and queued controls at the
  connection boundary. An outstanding control cannot move onto the replacement socket.
- An external New could reach the runtime before the outgoing workbench received its project
  change notice. Each workbench control now carries the project generation it was authored
  against. The runtime checks that generation before applying the control or changing history.
- The native fixtures check the retired Screen and Surface identities, pointer capture and
  replacement history. This allows the new project's own automatic Screen and Surface to
  appear while the old controls are retired.

[Earlier failures and the parked candidate](evidence/mapgyver-request-lifetime-deferred-2026-11-17.json)

## Software checks on November 20

- 707 Golden Engine and 104 transport tests passed. The runtime regression keeps the socket
  alive, replaces the project with one that reuses the same root ID, then verifies that the
  old batch is canceled and the new generation can edit. The old batch leaves the new label
  and Undo history untouched.
- The generated TypeScript protocol, 412 Golden UI tests and the MapGyver UI check passed.
  Queue and WebSocket tests cover stale work, the generation on both single and batch
  controls, and closing sockets whose replacement opens before `onclose` arrives.
- Strict Engine and transport Clippy and the ordinary bundled development build passed.

## Owned Windows app checks

- Intel and NVIDIA passed the full Surface Source suite with normal app exit, zero page errors
  and zero domain rejections. One Undo restored the exact pre-drag scene after a real socket
  reconnect. Captured Escape, the unavailable-source peer, offscreen Inspector editing,
  an external New during capture and exact Load restoration also passed.
- The fixture saves the actual two-panel layout in the project, then uses File → Reopen Last.
  The original project-data-only save omitted the locks. New is free to show its own initial
  Screen and Surface after the old capture has retired.
- Screen Warping passed overlap and before-release selection, bounded off-page Inspector
  editing, Escape, reconnect, held-pointer New and locked-panel reopen on both GPUs. The
  replacement project had zero Undo entries and the old Screen was absent.
- Matrix Layer and Cell passed independent locks, held-input retirement, project replacement
  and exact restoration on both GPUs. Both installed previews were observed together in one
  WebView sample. Physical outputs were suppressed in all of these fixtures.
- The NVIDIA moving-hit test found the intended native corner in 15 Screen and 16 Source
  samples. The separate DOM measurements differed by as much as 34.1 and 41.9 CSS pixels
  respectively. These sequential measurements establish native hit identity during zoom.
  OS-level pointer and same-frame image/control/hit qualification remain open. The Computer
  Use skill requires `node_repl`, which is unavailable in the current session.

Receipts in `target/`: `mapgyver-m141-generation-core-185327.log`,
`mapgyver-m141-generation-build-190458.log`, `mapgyver-m141-clippy-190555.log`,
`mapgyver-m141-restored-source-intel-191225/result.json` and
`mapgyver-m141-restored-source-nvidia-191311/result.json`.
Screen receipts: `mapgyver-m141-stable-screen-0-191623/result.json` and
`mapgyver-m141-stable-screen-1-191643/result.json`.
Matrix receipts: `mapgyver-m141-stable-matrix-0-191718/result.json` and
`mapgyver-m141-stable-matrix-1-191702/result.json`.
Moving hit receipts: `mapgyver-m141-native-hit-screen-191733/native-hit.json` and
`mapgyver-m141-native-hit-source-191743/native-hit.json`.
