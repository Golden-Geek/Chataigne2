# Matrix playback controls

MapGyver owns playback policy under `apps/mapgyver/src/matrix`. Golden owns the real
Parameter nodes, Inspector widgets, script property writes, transport intents and bounded
app-operation dispatch. Controls do not poll the engine or store authored playback booleans.

The current M13/6 slice provides these intrinsic Trigger Parameters:

| Owner | Declaration | Behavior |
| --- | --- | --- |
| Column | `parameters/play`, `parameters/cut` | Configured transition or immediate launch of that Column. |
| Cell | `parameters/play`, `parameters/cut` | Independent launch on its Layer, preserving the Column anchor and other Layers. |
| Matrix media | `parameters/stop_all` | Immediate Cut to empty across the base Column and every independent Layer request. |

The Matrix grid's top-left Stop All uses Golden's ordinary Trigger editor on that same
Parameter. The previous Installed dropdown is removed. Runtime submission markers remain
observational; pressing a control does not claim the renderer has submitted a frame.

`controls::Pulses` captures an owner UUID and action into Golden's existing bounded operation
service. The service resolves current ownership and rejects missing, disabled or mismatched
owners. Weak lifetime tokens cancel queued work on disable/destruction. Readiness guards keep
project reconstruction from replaying pulses. Control callbacks do not invalidate authored
clip bindings or require a prior Matrix inspection. Backend playback still stages requests
behind atomic edit-completion receipts; failed edits cannot publish a new request.

Stop All clears current playback demand, including independent Cell launches and preparation
cursors. Previously captured render/application plans remain owned until their consumers
release them. This does not bypass GPU completion or process retirement barriers.

M13/6 remains open. Layer, Column, Layer Group, Column Group and Cell scoped stops still need
implementation and verification, including outgoing clips. Existing grid Play/context actions
and high-level launch wrappers still call playback intents directly; routing every caller
through the real controls remains required. Native final-use/process retirement and complete
scope/caller acceptance are not established by this initial slice.
