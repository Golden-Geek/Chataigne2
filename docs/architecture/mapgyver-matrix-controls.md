# Matrix playback controls

MapGyver owns playback policy under `apps/mapgyver/src/matrix`. Golden owns the real
Parameter nodes, Inspector widgets, script property writes, transport intents and bounded
app-operation dispatch. Controls do not poll the engine or store authored playback booleans.

App Control configuration uses the generic Golden Inspector through MapGyver's
`ApplicationInspector.svelte` for Columns, Cells and Column Groups, including direct
inspection of their real App Control children. It renders only authored Parameters and
configured conditional folders. No Runtime section, app-snapshot controller or runtime
reader belongs to an Inspector instance. One workbench-scoped application runtime reader
still supplies the Matrix's compact runtime markers; process supervision, demand and
callbacks remain in the backend and do not depend on mounting an Inspector. See the
[M13/10 acceptance](../progress/evidence/mapgyver-m13-app-control-inspector-2026-11-07.json).

Transition Time is the ordinary typed time Parameter (`parameters/duration_ms`). It is
mandatory on Layers and optional on Columns and Cells: its own enabled metadata selects
override versus inheritance. Disabling it preserves the dormant duration and curve. Playback
reads that metadata from the document and invalidates its cached policy on metadata changes;
there is no separate authored override boolean. Configured launches resolve Cell → Column →
Layer, with a 250 ms base duration. Smooth is the default curve and maps to the existing
smoothstep interpolation; Linear remains selectable on overrides. Cut still uses zero time.
Column v3 and Cell v4 reject the former schemas, and the generated transition DTO exposes
`Smooth` without an old-name alias. Generic Golden widgets own metadata, entry and history.

The current M13/6 slice provides these intrinsic Trigger Parameters:

| Owner                 | Declaration                         | Behavior                                                                                         |
| --------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------------ |
| Column                | `parameters/play`, `parameters/cut` | Configured transition or immediate launch of that Column.                                        |
| Cell                  | `parameters/play`, `parameters/cut` | Independent launch on its Layer, preserving the Column anchor and other Layers.                  |
| Matrix media          | `parameters/stop_all`               | Immediate Cut to empty across the base Column and every independent Layer request.               |
| Layer / Layer Group   | `parameters/stop`                   | Cut every playing clip on that Layer / its current descendant Layers.                            |
| Column / Column Group | `parameters/stop`                   | Cut matching Column clips across every Layer, including independent launches and outgoing clips. |
| Cell                  | `parameters/stop`                   | Cut only that Layer/Column address, preserving other outgoing contributions on the Layer.        |

The Matrix grid's top-left Stop All uses Golden's ordinary Trigger editor on that same
Parameter. Compact grid Play and Stop buttons send Parameter intents to their exact real
nodes, using Golden's declaration-only child index. The previous Installed dropdown is
removed. Runtime submission markers remain observational; pressing a control does not
claim the renderer has submitted a frame.

`controls::Pulses` captures an owner UUID and action into Golden's existing bounded operation
service. The service resolves current ownership and rejects missing, disabled or mismatched
owners. Each callback mints a one-use token held by its owner; the internal delivery operation
must consume that token. Transport callers cannot invoke the internal operation directly or
replay a delivery. Weak lifetime tokens cancel queued work on disable/destruction. Readiness guards keep
project reconstruction from replaying pulses. Control callbacks do not invalidate authored
clip bindings or require a prior Matrix inspection. Backend playback still stages requests
behind atomic edit-completion receipts; failed edits cannot publish a new request.

Scope membership comes from the actual owning hierarchy when the backend services the
trigger. A group walk visits its selected axis subtree and stops at Layers/Columns; it never
descends into Cell media or another Matrix. Cell/Layer stops retain the Column anchor.
Column-axis stops clear that anchor only when its selected Column is in the stopped scope.
Affected Cell requests become empty instead of falling back to the Column. Unrelated
contributions retain their original fade weights, curve and deadline.

Deferred Cut filters are committed with the playback edit receipt. A private launch order
separates historical clips from a later Play before the next capture. Suppressed targets
prevent a cancelled, uncaptured launch from retargeting unrelated mixes. Filters run before
sampling a newer transition and after materializing pending requests, then drain after the
timeline installs them. Admission accounts for committed cuts, so Stop All recovers a full
interrupted mix immediately. Existing 4096 Layer-request/32 clips-per-Layer bounds remain;
coalesced deferred scope/target entries are bounded at their product, not an event queue.
Inactive configured Layers do not materialize runtime requests when a group stops.

Scoped stops withdraw their playing Cell/application references. A retained Column anchor
can still legitimately demand an application through its own anchor/pre/post input. Stop All
clears those inputs as well as independent playback. Previously captured render/application
plans remain owned until their consumers release them; these controls do not bypass GPU
completion or process retirement barriers.

High-level `LaunchColumn`, `LaunchCell` and `ClearPlayback` resolve and fire those same real
Parameters. This also covers grid double-click/context actions, script and command callers.
Their `MatrixControlInvoked` result identifies the owner and Parameter UUIDs. It acknowledges
the invocation; deferred playback admission and frame submission are separate observations.
Callers inspect playback for applied state; deferred rejection appears as a warning on the
owner. Golden's existing atomic edit fences and bounded post-commit service remain intact.

First Play at a sparse empty address creates one ordinary typed Cell, with no media, in one
authored undo step. Its Parameter owns the pulse. Later Play/Cut pulses are ephemeral and do
not alter project history. Undo/Redo reconstructs the Cell without replaying its original
control. Existing Cells resolve through the bounded address index without rebuilding the
document on every pulse.

M13/6 is [Accepted](../progress/evidence/mapgyver-m13-control-acceptance-2026-11-07.json).
All six scopes pass real transport/script and generic Inspector controls, rapid/history/load
checks, held-completion aggregate process retention and actual both-GPU output/preview pixels
with balanced native retirement. Column Group's app-owned Inspector filter includes its real
Parameters folder alongside App Control; filtering only App Control hid Stop from the Inspector.
Separate interaction timing, source format, WOW64 and production qualification gates remain open.

Project-candidate hierarchy classification keeps each owner's declared App Control root
`Regular`, with non-curated permissions matching its embedded constructor. The typed owner's
handle determines this policy. Promoting those settings to `ItemRoot` during load incorrectly
made Column reparenting validate the embedded control as a new user item in the destination
Group. Correcting the app's classification restores nested grouping after reload without
changing Golden admission or insertion order. The [ownership regression and bundled native
fixture](../progress/evidence/mapgyver-m13-intrinsic-controls-2026-11-07.json) now pass; earlier
native failures remain preserved in the preceding scoped-stop evidence.
