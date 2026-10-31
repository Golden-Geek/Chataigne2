# MapGyver implementation plan

MapGyver is a new Windows x64 video mapping app alongside Chataigne. This plan implements the
[MapGyver architecture](../architecture/mapgyver.md), using the same Golden Core and UI.
The user requested architecture and planning on 2026-10-27, **not implementation in that turn**.
No milestone below is implemented by these documents. Current execution state belongs only in
[mapgyver-status.md](../progress/mapgyver-status.md).

## Entry and reconciliation

To start this product explicitly, request:

```text
Implement docs/plan/MapGyver_Implementation_Plan.md.
Read AGENTS.md and docs/progress/mapgyver-status.md, reconcile the checkout and shared
Golden work, then continue the earliest dependency-ready unfinished MapGyver milestone.
Preserve unrelated edits and completed Chataigne work. Follow each milestone's acceptance
criteria, public ownership boundaries and repository validation rules.
```

The unqualified phrase "implement the plan" retains its AGENTS.md meaning: the Chataigne plan.
This is a separate, explicitly selected product plan, not a restart, replacement or extension
of its P-task ledger. Follow the standing
[implementation plan delivery rules](../../AGENTS.md#implementation-plan-delivery): commit and
push each meaningful phase or coherent milestone without further confirmation. Use the
selected `mapgyver` branch and `origin/mapgyver`, establishing the upstream if absent.
This authorization does not include releases or deployments.

Start every resumed session with checkout/diff review and both current product ledgers. On
2026-10-27 the checkout already contained unfinished engine/Spatializer P11 work and modified
architecture/status evidence. Do not absorb, revert or reformat that work into a MapGyver batch.
Shared changes need owner-specific validation and evidence in the relevant product handoffs.

## Product acceptance

- A thin `apps/mapgyver` app starts through Golden's ready-to-run desktop/headless host; it shares
  the workbench, inspectors, parameter system, history, scripting, transport and persistence.
- Media Manager supports Solid Color, Picture, Shared Texture, Composition and Matrix media.
  Channels are independent concurrent textures, each with automatic/fixed size. Spout maps one
  sender per channel. Stable media/channel routing survives resize, reorder and reconnect.
- Screen Manager supports physical fullscreen displays with DPI correction, physical desktop
  rectangles, and virtual Spout outputs. Ordered Surfaces sample media/channel sources with
  source crops, valid four-corner perspective warp and generated calibration patterns.
- Screen, Surface Source, Composition and Matrix editors are app-owned Svelte 5 panels using
  reusable native preview presentation. Output cadence survives hidden/disconnected editors.
- Matrix provides layers/groups, column launches, transitions, blend modes and layer/group/final
  channels. Column app associations use neighboring-column pre/post demand and retain owned
  processes through completed unloading, including outgoing transitions and shared app demand.
- UI, scripts and transports invoke the same backend operations. Each module includes command
  nodes, methods, callbacks and app-owned script snippets in its delivery, not a later catch-up.
- Project roundtrip, undo/redo, missing-resource states, endpoint recovery and bounded runtime
  ownership work before a milestone is declared verified. No native handles/frame pixels appear
  in persisted documents or the control protocol.

Initial movie playback, audio, effect hosting, mesh warping, edge blending, remote video streaming
and cross-platform/cross-adapter guarantees are outside this feature set. Do not add an unused
framework for them. Proposed defaults not directly specified by the user are identified in the
architecture and remain backend policies that can change during implementation review.

## Milestones and dependencies

| ID | Outcome | Dependencies |
| --- | --- | --- |
| M00 | Reconciled baseline and fixed qualification workloads | Explicit implementation request |
| M01 | Functional native WebView underlay and Spout/D3D11 route decision | M00 |
| M02 | Clean multi-app workspace, host and UI seams; runnable MapGyver shell | M01 functional route decision |
| M03 | Golden Video execution and MapGyver media/channel contracts | M02 |
| M04 | Solid Color, Picture and concurrent Spout media | M03 functional foundations: gates 1–3 and 5–8 |
| M05 | Screens, Surfaces, output targets and two mapping editors | M04 working media/channel contracts |
| M06 | Composition media and 2D editor | M04/M05 working media, surface and editor contracts |
| M07 | Matrix rendering, exported channels and grid editor | M06 working composition/channel contracts |
| M08 | Column-controlled app lifecycle and neighbor demand | M07 working column/transition contracts |
| M09 | Complete-app qualification and Windows artifact | All feature and qualification acceptance in M03–M08 |

### Execution correction — 2026-10-30

The user requested concrete anti-drift rules and product advancement after prolonged M03
investigation. Functional prerequisites now govern **implementation order**, separately from
milestone acceptance. The seven verified M03 foundation gates permit M04 product work now.
M03 gates 4 and 9 remain open, owned by M03, with their complete original scope, budgets and
failed evidence intact. They are not moved wholesale to M09 and do not count as passed.
Likewise, missing exhaustive physical/display qualification must not prevent implementing
Composition or Matrix against working, verified contracts. A reproduced correctness or resource
ownership defect on the selected feature path must still be fixed before building on that path.

Apply [the anti-drift rules](../../AGENTS.md#autonomous-progress-and-anti-drift): 45-minute feature
batches, 20-minute/two-experiment investigation limits, and at least four product-delivery
batches per diagnostic-only batch. The immediate next deliverable is M04 Media Manager and
Solid Color functionality through the existing app-owned media/channel model. Define M04's
fixed acceptance breakdown before starting its implementation. Checkpoint working product
slices instead of extending the renderer measurement chain. Functional readiness never closes
an unverified milestone or authorizes release; M09 still requires every retained gate.

### Completion priority — 2026-10-31

The user explicitly requires **100% completion of the plan**, with existing open milestones
returned to and closed. Functional dependency ordering permitted implementation to proceed;
it did not complete acceptance. Execution now prioritizes one dependency-ready phase/gate at
a time, starting with M04 source acceptance, while other incomplete phases remain in the
explicit return queue. Do not add incidental features to satisfy the earlier delivery/diagnostic
ratio or expand work in several phases at once. Consecutive qualification batches are allowed
for closure; the 45-minute batch and 20-minute/two-experiment limits remain operative.

Blocked gates retain their exact evidence, missing prerequisite and next concrete decision.
All original requirements and budgets remain required. For a minor optimization that consumes
roughly a full day, present a specific proposed deferral/lower gate, evidence and consequences
for the user's decision before changing acceptance. This is not blanket authorization to lower
gates, and correctness/ownership/data integrity/essential behavior are not small optimizations.

### M00 — Reconcile and define evidence

Identify existing public Golden contracts and completed product work, available GPU/monitor
hardware, pinned host versions, toolchain and Spout SDK candidates. Preserve Chataigne's default
root launch behavior. Record shared P12 boundary repairs and still-open P14/P15 time/asset work.
Record the intended Windows build floor, actual Windows build, WebView2 Runtime, GPU adapter
and driver versions as part of the compatibility evidence.
Define reference scenes and memory/frame/interaction budgets before the native or scale runs.
Planning estimates, source inspection and a passing synthetic render are distinct evidence.

**Acceptance:** a current checkout/ledger handoff, exact available/unavailable native routes,
documented workloads and budgets, and no implementation inferred from historical evidence.

### M01 — Decide the functional native rendering route

Build narrow native probes using locked Tauri/Wry and D3D11. Exercise transparent WebView regions,
DOM overlays, two concurrent Spout inputs, native output and Spout send. Inspect existing Rust
Spout bindings and upstream SDK ownership/build/license contracts. Use the scoped
[native feasibility gate](../architecture/mapgyver-native-rendering.md#native-feasibility-gate).

Do not copy the desktop bootstrap into MapGyver as a production solution. Keep the probe scoped,
then remove or retain it as an owned qualification harness. If the ordinary underlay fails,
resolve the explicit DirectComposition alternative before marking the milestone complete.

The user approved a scope correction on 2026-10-28: M01 decides whether the route works well
enough to build the thin M02 shell; it does not complete product performance/display qualification.
Its four fixed acceptance items are:

1. Observed ordinary Tauri/Wry native-underlay layering, clipping and DOM input/overlay behavior
   on the recorded available configurations, with their DPI/latency limits explicit.
2. Caller-owned D3D11 devices and two differently sized Spout sources compose into native and
   Spout outputs without recurring CPU-frame transport, with the binding/SDK ownership audited.
3. Scoped source resize/restart, independent output during editor interruption/reconnect, and
   owned resource/process cleanup smoke have native evidence; sustained recovery budgets remain separate.
4. Record the host/backend/binding choice, its evidence and limitations, and the owners of every
   continuing qualification requirement.

The [recorded decision](../progress/mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)
selects ordinary Tauri/Wry, D3D11 and pinned `spout2-sys =0.1.1`. This changes the M01 acceptance
denominator from eight broad qualification items to four functional decision items. The original
full gate has not passed: renderer budgets/resources/recovery belong to M03, physical output,
preview/DPI/input qualification to M05, and integrated sustained/full-product Q05 to M09 and
the feature owners. The [qualification contract](../progress/mapgyver-qualification-contract.md)
retains all workloads, budgets, failures and unavailable checks.

NVIDIA runs follow the user-approved [overlay-off qualification prerequisite](../progress/mapgyver-qualification-contract.md#nvidia-overlay-prerequisite);
retain the overlay-on failure and unchanged budgets.

### M02 — Establish the second app without product leaks

M02 depends on the recorded M01 functional route decision, not completion of the continuing
performance/display matrix. It delivers the shell and public seams; it grants no native quality pass.

Create the MapGyver shell/UI manifests, identity, project file spec/data directory, resources,
capabilities and empty project scaffold. Introduce functional Media/Screen managers with their
owning M04/M05 deliveries, avoiding placeholder product models. Use Golden's lifecycle and
bundled/dev-server APIs. Extract reusable UI bundling into the supported Golden codegen/build surface before
duplicating Chataigne's large build script; Chataigne formula/native-SDK steps stay app-owned.

Repair exercised multi-app seams once at their public owner:

- App-published branding/version/file-folder labels replace hardcoded Golden UI Chataigne strings.
- App identity namespaces browser layout/project/selection storage, separate from origin alone.
- Generic service discovery replaces hardcoded `chataigne` service/endpoint policy; update
  generated protocol and consumers together, without a compatibility alias.
- Cargo/npm/xtask/watch/package commands select an app explicitly; retain Chataigne as the
  existing root default and allow both products to run on distinct configured ports.
- The Golden desktop host exposes an optional, typed native-presentation lifecycle extension.
  Keep host construction, native window creation and COM-thread coordination in the host.

**Acceptance:** both bundled/dev apps start independently and concurrently; names, recent files,
preferences, layouts, project formats and discovery do not cross-contaminate. Engine-only,
headless, protocol-only and Chataigne full-host consumers retain valid public boundaries.
No private path imports, copied host, or app-specific branch appears in Golden packages.

Track this unchanged scope as seven fixed acceptance items, defined before M02 implementation:

1. Public UI-bundling API adopted by the existing Chataigne app.
2. MapGyver host/UI scaffold with app identity, project specifications and resources.
3. App branding and browser-state isolation.
4. Generic discovery with generated protocol and endpoint isolation.
5. App-selected Cargo/npm/xtask/watch/package commands, preserving the root Chataigne default.
6. Optional typed native-presentation host lifecycle extension.
7. Both bundled/dev apps start independently and concurrently, with headless/engine/protocol
   boundary validation.

### M03 — Deliver render/resource and channel foundations

Introduce Golden Video's exercised D3D11 backend, opaque resources, render DAG, frame scheduling,
bounded handoffs, generations, completion/retirement and null/mock qualification path. Add
MapGyver media/channel models, references, size resolution and backend operations. Add reusable
preview registration/geometry bindings when the native consumer exists; keep the specialized
editors in the app. Rust DTOs generate all TypeScript contracts.

Reconcile shared P14 time and P15 asset requirements at their Golden owners. Production readiness
requires a monotonic transition/activation authority, asset IDs/locators separate from prepared
textures, worker-side preparation/cancellation, relocation/relink and stale-generation fencing.
Implement missing reusable pieces once and update shared evidence; MapGyver does not wait for
unrelated Chataigne roadmap tasks and does not create competing clock/cache/file protocols.

**Acceptance:** deterministic DAG/cycle/resize tests, atomic plan install/rejection, stable channel
IDs across history/reload, bounded GPU/worker admission and retirement, source/output state
observations, and clean device recreation in the strongest available native harness. No frame
transport through JSON, engine-loop GPU polling, or final cleanup on the actor/UI thread.
Measure the reusable renderer against the unchanged Q01/Q02 CPU/GPU, cadence, frame-age,
queue/admission, memory/handle and shutdown budgets. M03 owns bounded resource retirement and
device recovery; preserve unresolved harness failures and verify their production replacements.
These measurements are M03 acceptance work, not deferred wholesale to M09.

Track this complete scope as nine fixed acceptance items, defined before M03 source work:

1. App-neutral immutable render plans, explicit texture/color/alpha metadata and deterministic
   DAG/demand/resource-budget compilation, including cycle/resize/rejection coverage.
2. Worker-owned runtime and null/mock path: atomic frame-boundary install, project/revision/device
   fences, bounded desired/prepared/in-flight/retiring work and completion-based resource ownership.
3. Exercised D3D11/Spout backend with opaque resources, explicit caller-device/adapter ownership,
   generic compositing/conversion and shared native-host presentation consumption.
4. Source/output state observations and device-removal/recreation, independent healthy paths,
   cancellation and bounded recovery/retirement through the production runtime.
5. App-owned media/channel IDs, typed references, backend size/default operations and generated
   contracts, with stable identity through history/reload and resource recreation.
6. Shared production-time authority and activation/discontinuity contracts used by M03 scheduling;
   reconcile shared P14 evidence without claiming unrelated audio/DMX acceptance.
7. Shared persistent asset IDs/locators, worker preparation/cancellation, relocation/relink and
   stale-generation/resource admission; reconcile shared P15 evidence at its reusable owner.
8. Reusable preview registration/geometry and generated control contracts exercised by the native
   consumer, leaving specialized editors and full physical/DPI/input qualification with M05.
9. Production-renderer Q01/Q02 native CPU/GPU/cadence/frame-age/admission/memory/handle/shutdown
   measurements and applicable recovery runs against the unchanged qualification contract.

This replaces M03's provisional whole-phase item with an unchanged-scope breakdown; all nine
start open. A compiler/mock/source checkpoint alone does not qualify native rendering or retire
the recorded failures. Deliver each coherent implementation batch before starting the next batch.

### M04 — Deliver source media and Media Manager

Implement multi-channel Solid Color, Picture and Shared Texture. Picture preparation probes and
decodes assets on workers, handles alpha/color conversion and respects pixel/decode/cache budgets.
Spout discovery and receiver supervision remain at the native boundary; every configured channel
owns an independent sender subscription and can resolve a different extent.

Ship Media Manager inspection, channel/source/size operations, commands, script methods,
callbacks and snippets, including manager registration in the default project. Batch
creation/import with detached trees and authoritative IDs.

**Acceptance:** at least two differently sized channels per relevant media; shared consumers;
automatic/fixed extents; missing/relinked/replaced pictures; sender absence/restart/rename/resize;
one channel failing while another remains live; explicit adapter mismatch; project/history
roundtrip and UI/script/headless intent parity. Initial asset upload is not recurring readback.

### M05 — Deliver mapping output and two editors

Implement the Screen Manager, Screen/Surface authoring and runtime plans, display catalog/recovery, fullscreen/native
rectangle windows and virtual Spout senders. Add crop/homography/calibration primitives and the
Screen and Surface Source panels. Normal UI layout remains relative; desktop pixels/texture
extents use deliberately typed physical units.

**Acceptance:** source crop followed by perspective warp matches a reference pattern; invalid
quads/crops fail atomically; calibration works with no source and restores routing; overlapping
Surfaces preserve order/alpha; each target mode works; mixed DPI/negative origins/hotplug retain
intended display identity. Multiple outputs continue when editor panels close or minimize.
One drag is one undoable gesture. Commands/scripts/callbacks/snippets ship with these modules.
M05 owns the retained native presentation matrix: exact physical client/output and preview
geometry, alpha/color/reference patterns, 100/125/150/200% OS DPI, mixed-DPI/negative-origin
placement, overlays/pointer capture, and measured alignment/input-to-visible latency. Verify
physical output and editor independence on available routes; unavailable configurations stay open.

### M06 — Deliver Composition media and editor

Implement ordered instances with media/channel references, position, scale, rotation, pivot,
opacity and blend modes; authored canvas outputs resolve to reusable render passes. Add the 2D
editor and source-picker behavior through app-owned registrations.

**Acceptance:** nested compositions render once per demanded output; shared-source instances
retain independent transforms; cycle rejection spans all media types; source deletion/undo,
canvas resize, alpha and transform pivots are correct in both preview and output. Selection,
drag/rotate/scale, history and scripted operations use the same backend contracts.

### M07 — Deliver Matrix media and editor

Implement stable layer/group/column/cell identities, grouping, source assignment, atomic column
launches, empty-cell policy, interrupted transitions, blend modes and derived layer/group/final
channels. Use the shared time boundary. Virtualize the grid and expose live state distinctly
from selection; preparation/preload does not itself display a cell.

**Acceptance:** exported IDs survive reorder and reload; independent output consumers do not
duplicate the full render; group ordering/blending and interrupted transitions are deterministic;
outgoing media remains available until final use; burst launches cannot accumulate unbounded
plans. Creation/removal/reorder/history/copy and commands/scripts/callbacks/snippets are coherent.

### M08 — Deliver column-controlled app lifetimes

Add the public Golden IO owned-process primitive rather than importing Chataigne's private
App Control. Keep column associations, pre/post windows, readiness and demand aggregation in
MapGyver Matrix. Launch suspended/assign Job Object/resume; own the process tree through stop.

**Acceptance:** fake-time/fake-process suites plus an owned test executable cover:

- Column 3 `pre=1` starts at column 2; `post=1` retains through column 4; bounds, jumps and reorder
  match the specified non-wrapping neighbor rule.
- Shared app definitions, multiple launch scopes, interrupted loading and outgoing transitions
  hold demand until the final unload/render completion; no transient kill/restart on reconciliation.
- Sender-based readiness and timeout, crash/restart/backoff, cancellation during launch, forced
  shutdown deadline, process-tree cleanup and project replacement reject stale outcomes.
- Pre-existing external/singleton/elevated/unsupported targets produce truthful outcomes and
  never trigger process-name kills. Intentional unloading never schedules a restart.

All app-lifecycle editing/control and observation callbacks are available through the same
generated UI/script/headless contracts; authored undo is distinct from OS side effects.

### M09 — Qualify the actual app and Windows artifact

Exercise all five media types, four editors and three output modes in the bundled x64 build.
Record real GPU, display/Spout, DPI, memory/handle, frame-time and interaction evidence for fixed
scenes, including 1080p60, 4K and multiple outputs. Qualify repeated project/column replacement,
recovery and shutdown, not just a static render. Distinguish authored-node scale from active
channel/pixel throughput. Run Chataigne regression checks for every changed shared boundary.

**Acceptance:** exact binary/assets/dependency versions and repeatable reports; required Rust/UI/
codegen/boundary/license checks; native smoke and sustained runs against recorded budgets;
truthful unavailable routes; onboarding and app-specific run/package commands. A package can be
reviewable without being published. Release/signing/publication requires its own instruction.
Repeat the required sustained Q01/Q02 series and Q03 soak in the actual app, integrating the
M03 renderer and M05 presentation evidence. Full product Q05 covers the source/output, project,
plan and Matrix lifecycle counts as those features arrive; owning milestones verify their own
recovery first. Neither the M01 decision nor a passing probe replaces these requirements.

## Verification and handoff rules

Keep the [status dashboard](../progress/mapgyver-status.md#at-a-glance) at the top of the ledger
current at every implementation handoff and milestone. Show overall completed phases and
per-phase acceptance-item counts, bars and percentages, plus the current topic, next action
and blocker. Follow its documented counting rubric; preserve fixed acceptance scope and record
the reason for any item breakdown change. Detailed evidence follows the dashboard.

Keep meaningful tests under adjacent `tests/` directories, including native fixtures under the
owning crate's integration tests. Use deterministic null/fake backends for lifecycle, DAG and
time semantics; native evidence is still required for texture sharing, composition and DPI.
Follow repository rules for desktop automation. The user authorized it on 2026-10-27;
native input/visual checks may use the computer-use skill against the probe's own windows.

For each changed boundary run its owning tests and generated-output drift checks, Svelte checks
and relevant packaged/native probes. Finish Rust formatting at both the root and Golden Core
manifests. Update the MapGyver ledger with what is implemented, exact verification, unresolved
checks and the next dependency-ready task. Do not label design work or untested code "verified."
