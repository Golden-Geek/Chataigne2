# P11 dashboard layout intent boundary

Dashboard move/reorder and constrained resize now use explicit backend intents. The UI no longer
enables internal sizing parameters before resizing or builds a move from successive parameter and
metadata edits. This repairs the authoring boundary required before ordered gesture qualification;
it does not establish complete ordered-operation lifetimes or close the dashboard checkpoint.

## Owners and behavior

Rust `UiEditIntent` owns `MoveDashboardWidget` and `ResizeDashboardWidget`. Protocol **0.11.0**
and generated TypeScript bindings change together. Golden UI imports the new variants from the
generated union rather than declaring another wire shape. Existing HTTP/WS/headless intent
dispatch reaches the same engine implementation; transport tracing names both requests.

`ui_sync/dashboard_layout.rs` checks attached widget identity, dashboard destinations, supported
axes, finite/nonnegative sizes and read-only parameters. Move indices address the authoritative
user-item sequence, exclude the source, preserve preceding structural children and clamp at the
end. Self/descendant moves still use engine hierarchy validation. Placement follows the destination
layout: free uses anchor/position/both dimensions, horizontal uses width, vertical uses height,
and grid/tab/accordion use their own layout. The backend owns applicable sizing enablement.

Moves first settle the destination's layout dependencies inside the atomic operation, then resolve
newly materialized fields and apply placement. A rejected placement rolls that move and its lifecycle
effects back together. Resize prepares all fields before submitting any edits. Both use the existing
atomic operation/history boundary. A standalone request creates one history entry; a caller's existing
group remains owned by that caller. Layout field lookup scans declared local children rather than
building a process-tree snapshot for each parameter. Move ordering scans only destination siblings.
These ownership changes do not themselves establish large-project action latency.

`dashboard-actions.ts` converts viewport placement and sends one move intent. It no longer waits
for individual fields or patches sizing metadata. Reorder callers send the requested item index
for backend resolution. Confirmed free-layout reparenting supplies its captured session explicitly.
The constrained pointer adapter reuses the frame/latest-sample committer and session admission,
captures host/transition/source paths, and opens one owned history group. Cancellation drops the
unsent tail; confirmation hands remaining work to the session, independently of view disposal.
It performs no eager enable edit. Component-level delayed-receipt/interruption qualification
and pending size presentation still remain.

Source review also found that the gesture fence required layout fields to be direct widget
children. It now captures their structural folder paths through the owning widget and rejects
paths crossing another widget. Folder replacement/reparenting retires the operation. Two mounted
move/resize cases use declared layout folders, alongside unit tests for folder identity and invalid
ownership. The earlier flat fixtures alone did not qualify that backend-shaped hierarchy.

## Validation

All **seven new engine cases** pass, covering free/horizontal/vertical sources against all six
destination layouts (18 combinations), explicit sizing, caller-owned grouping, index clamping,
invalid destinations/cycles/anchors, invalid sizes and read-only rejection. Successful operations
undo/redo as one authored change; rejected requests preserve authored state, history and events.
Two-axis rejection also proves that a valid first dimension cannot leak before a rejected second.

The initial [two failed probes](evidence/p11-dashboard-layout-intent-probes-2026-10-25.json)
assumed unsupported dimensions still had nodes. Their expectations now match actual dependency
materialization. Source review then moved placement resolution after destination lifecycle
settlement and added the complete source/destination matrix. A broader in-progress build was
terminated by its verified process tree for this source correction; it supplies no pass claim.

The full engine suite passes **634 unit cases / three existing ignored**, **eight public integration
cases** and **seven doctests**. Protocol passes **three public cases**; transport **74 unit / one
existing ignored plus one public headless-host case**; codegen **11 unit plus three public cases**.
Strict all-target Clippy passes for engine, protocol, transport and codegen. Regeneration of the
protocol and version is byte-for-byte stable.

Root UI passes **349 cases**: audio 27 / graph 52 / Golden UI 155 / app 115. Five added UI unit
cases cover one-intent move semantics, authoritative indices, rejection/source retirement and
structural folder fences. Targeted TypeScript diagnostics for both added/extended unit sources
and their dependencies report zero errors. Browser and app Svelte checks report zero errors and
warnings. Source/report Prettier, whitespace and both Cargo formatters pass.

The final [browser evidence](evidence/p11-dashboard-layout-intents-2026-10-25.json) passes
**215 correctness groups and five sparse tiers** through 100k authored nodes, with **117 frozen
source fingerprints** and no page errors. The two added mounted cases verify move/resize through
declared layout folders. All prior 213 groups pass. These component-operation fixtures do not
exercise the constrained adapter's pointer handlers or establish complete ordered lifetimes.
No native input is synthesized and no dashboard action-latency distribution is claimed.

Inventory is **2,152 source files / 27 oversized**. DashboardCanvas is 969 lines, dashboard actions
679, the ordered controller 416, the backend layout owner 220 and its tests 326. All touched
runtime/test sources remain under 1,000 lines without a new exception.

The locked default app build passes in **2m21s**, rebuilding the frontend bundle. Executable
SHA-256: `fc77fb0ae2965831300eca003e6a3747e0129924124c293fa34e478d11cdc141`.

Reproduce with `cargo test-fast --locked -p golden_engine`,
`cargo test-fast --locked -p golden_protocol -p golden_transport_server -p golden_codegen_support`,
`npm test`, `npm run check:browser --workspace golden_ui`, `npm run check --workspace chataigne-ui`
and `npm run test:browser:dashboard --workspace golden_ui`.

## Remaining acceptance

Finish explicit ordered start/update/finish/cancel operations and qualify the resize adapter's
coalescing, history and session handoff. Qualify cancellation versus release, mode/
layout/identity changes, in-flight receipts, offscreen disposal, container and CSS-unit geometry,
and composed project/save/undo lifetimes. Then run the fixed action/timing matrix before closing
dashboard viewport awareness. Tree keyboard, graph keyboard and composed workflows remain.

**P11 remains 2/6 checkpoints (33% rounded); whole-plan scope coverage remains 57% (57.41% exact).**
