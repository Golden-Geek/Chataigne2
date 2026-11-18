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

Matrix provides these intrinsic Trigger Parameters:

| Owner                 | Declaration                         | Behavior                                                                                         |
| --------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------------ |
| Column                | `parameters/play`, `parameters/cut` | Configured transition or immediate launch of that Column.                                        |
| Cell                  | `parameters/play`, `parameters/cut` | Independent launch on its Layer, preserving the Column anchor and other Layers.                  |
| Matrix media          | `parameters/stop_all`               | Immediate Cut to empty across the base Column and every independent Layer request.               |
| Matrix media          | Previous/Next Column and Previous/Next Column Group | Configured launch in document order, scoped to Columns or top-level Column Groups. |
| Layer                 | `parameters/stop`                   | Fade the complete current mix using resolved Cell → Column → Layer duration/curve; zero cuts.     |
| Layer                 | `parameters/previous_cell`, `parameters/next_cell` | Step between playable Cells on that Layer with the configured transition. |
| Layer Group           | `parameters/stop`                   | Cut every playing clip on its current descendant Layers.                                         |
| Column / Column Group | `parameters/stop`                   | Cut matching Column clips across every Layer, including independent launches and outgoing clips. |
| Column Group          | `parameters/play`, `parameters/previous_child`, `parameters/next_child` | Play its first enabled Column or step among its direct Columns and nested Groups. |
| Cell                  | `parameters/stop`                   | Cut only that Layer/Column address, preserving other outgoing contributions on the Layer.        |

The Matrix grid's top-left toolbar exposes Previous/Next Column, Previous/Next Column Group
and Stop All. The generic Inspector exposes the same Triggers. Compact grid Play and Stop
buttons send Parameter intents to their exact nodes using Golden's declaration-only child
index. Runtime submission markers report frames independently of these triggers.

Column navigation follows the document order through nested Column Groups. Group navigation
steps among top-level Column Groups, skipping Groups without an enabled Column. A Group
launches its first enabled descendant Column with the configured transition. The Matrix's
last requested Column anchors both controls. An ungrouped Column anchors Group navigation at
its own position in the axis. After Stop All, Next starts from the beginning and Previous
starts from the end. Navigation at a boundary holds the current playback.

Each Layer row header also exposes Previous/Next Cell. These controls start from the Layer’s
requested Cell, or the Matrix Column if the Layer inherits playback. An explicitly stopped Layer
starts at its first or last playable Cell. Navigation walks the existing sparse Cells in Column
order. It skips unconfigured and disabled Cells and disabled Columns. A Cell with an enabled,
configured App Control can play without media. Stepping uses the normal Cell transition and
preserves the Matrix Column anchor and the other Layers. Named scripts use
`navigateMatrixCell(requestId, matrixId, layerId, direction)` with `"Previous"` or `"Next"`.

Column Group headers and the Inspector expose Play and Previous/Next Child. Play launches the
first enabled Column in the Group, recursing through nested Groups in document order. Previous
and Next step through direct children. When the current Column belongs to a nested direct child,
the entire child acts as the anchor. Landing on a nested Group launches its first enabled Column.
Empty or disabled children are skipped. From outside the Group, Next starts at its first eligible
child and Previous starts at the last. At either boundary the current launch continues. The typed
edits `PlayColumnGroup` and `NavigateGroupChild` and named scripts `playMatrixColumnGroup` and
`navigateMatrixGroupChild` fire the same Parameters.

Cells expose Auto Next in the Inspector. **Specific Time** counts wall time from the committed
Cell launch, including when the Matrix launches a whole Column. **End of Media** follows the
active Video's crop-relative transport and speed. Paused, zero-speed and looping transport
does not supply a finite deadline. **Include Transition Time** subtracts the next Cell's
effective transition from the remaining time, with a floor of zero. The outgoing Video keeps
its ordinary transition lease and transport policy until the fade finishes.

Auto Next uses the same eligible successor and per-Layer launch as manual Next Cell. Runtime
requests carry committed launch identities and use Matrix's shared deadline and actor service.
Authoring changes, Video clock revisions and committed replacements cause reevaluation. Stop
or removal cancels the former launch, and an advance leaves the Column anchor, peer Layer
requests and project history in place.

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

Direct Layer Stop is resolved in `matrix/stops/layer.rs`, using the latest requested
playback destination on that Layer, including independent Cell launches. If the target
has already been cleared, the most recent retained outgoing launch supplies its Column;
the Layer fallback applies when its authored Column/Cell no longer exists. Current
optional Cell/Column metadata determines precedence. One resolved transition fades all
contributions together from their sampled weights; Inspector selection is not policy.
Repeated Stop reuses the committed empty target and does not restart its deadline.

`matrix/playback/layer_stop.rs` owns receipt-gated stop identities and logical outgoing
application holds. A newer Play or immediate scoped Cut revokes only its affected holds.
The existing Matrix deadline notifier expires headless holds and wakes application
reconciliation; there is no main-thread polling. Rendered fades use shared timeline time,
and renderer plan/final-use holds retain their independent GPU completion fences. Stops
remain runtime state: they do not edit authored Parameters or create Undo entries.

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

High-level `LaunchColumn`, `LaunchCell`, `NavigateColumn`, `NavigateColumnGroup`, `NavigateCell` and
`ClearPlayback` resolve and fire those same real Parameters. This also covers grid double-click,
context actions, script and command callers. Named script methods `navigateMatrixColumn` and
`navigateMatrixColumnGroup` take a Matrix ID and `"Previous"` or `"Next"`. `unloadMatrix` fires
Stop All.
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
