# MapGyver implementation status

## At a glance

**Updated: 2026-10-30 · Overall: `██████░░░░░░░░░░░░░░` 30% · 3/10 phases complete**

**Current topic:** M03 remains **7/9** accepted gates. The
[reader-ownership fix](mapgyver-m03-qualification-runner.md#shared-reader-ownership-across-plan-generations--2026-10-30)
removes recursive keyed-mutex acquisition when a new plan copies a publication still owned by
an earlier frame. The controlled native repro fails with `0x887A0001` before the fix and now
verifies both images plus latest-fence retirement. New controls retain both sources in their
first generation and remove the ten Q02 interruption lineage failures. Q02 still misses
capture/cadence/age budgets; instrumented GPU p95 is 15.8522 ms. No qualification gate closes.

**Next:** attribute the remaining Q02 renderer/consumer stalls from retained CPU, GPU-context
and exact software-display timelines, then establish supported GPU-clock/queue calibration
before changing GPU work. Preserve slot-owned leases, explicit source policy and nonblocking
renderer admission; do not repeat output-order variants or recollect identical traces. Complete GPU
critical-path and native buffer/GPU-queue measurements (gate 9), then address remaining
cadence, capture coverage and growth qualification. Native
device removal and bounded stuck retirement retain gate 4.
**Blocker:** new Q02 sample calls are main/Spout/editor 300/300/295. Minimized-editor main/Spout
calls are 55/59 in one second, with four Spout images uncaptured; disconnect captures 51/60,
with covered source A/B p95 age 62.11/66.17 ms. There are no current lineage-invalid captures
or source restarts. The remaining interruption failures, prescribed cadence coverage and
window deadline failures remain open. The short
renderer-context timing does not qualify displayed cadence, three-output age or the full GPU
critical path. Full GPU
critical-path and native buffer/GPU-queue coverage remain unavailable. Genuine device removal
and stuck-driver retirement bounds remain unverified.
Historical memory-growth, geometry and display/input failures retain their M03/M05/M09 owners.

| Phase | State | Progress | Done / total items |
| --- | --- | --- | --- |
| [M00 Baseline/workloads](mapgyver-m00-baseline.md) | Complete locally | `████████████████████` 100% | 1/1 |
| [M01 Functional route decision](#m01-acceptance-items) | Complete locally | `████████████████████` 100% | 4/4 |
| [M02 Multi-app seams/shell](#m02-acceptance-items) | Complete locally | `████████████████████` 100% | 7/7 |
| [M03 Rendering/channels](#m03-acceptance-items) | In progress | `████████████████░░░░` 78% | 7/9 |
| [M04 Source media](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M05 Screens/Surfaces/editors](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M06 Composition/editor](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M07 Matrix/editor](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M08 App lifetimes](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M09 App/artifact qualification](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |

Overall counts fully accepted milestones: `3 / 10 = 30%`; partial phases do not advance it.
Phase percentages count closed acceptance items, rounded to the nearest whole percent
(`M01: 4 / 4 = 100%`, `M02: 7 / 7 = 100%`, `M03: 7 / 9 = 78%`). Bars use 20 cells, rounded from the exact fraction. Each phase uses
its whole plan acceptance as one item unless a fixed breakdown is recorded here before
implementation continues. On 2026-10-28 the user explicitly approved changing M01 from eight
full-qualification items to four functional-route items, allowing M02 to start. The original
eight-item matrix remains in the [native evidence](mapgyver-m01-native-feasibility.md#remaining-gate)
with continuing owners in M03/M05/M09; its failures are not converted to passes. M02's seven
fixed items below were defined before source implementation and cover its complete acceptance.
M03's provisional whole-phase item was split into nine fixed gates on 2026-10-28 before source
work; the combined scope and native budgets are unchanged, and all nine started open.
These counts measure accepted scope, not effort or product readiness. Partial evidence earns
no item closure; future breakdowns must preserve the complete acceptance scope and explain
denominator changes. See [milestones](#milestones) and [next action](#next-action) for detail.

### Delivery checkpoint — 2026-10-28

The repository-wide [delivery rule](../../AGENTS.md#implementation-plan-delivery), committed as
`21d5bae`, now requires commits and pushes for every meaningful implementation milestone in
every plan. The current `mapgyver` checkpoint preserves the existing M00 baseline, architecture,
M01 harness and historical evidence. M01 remains at **1/8** acceptance items; this source
checkpoint does not qualify the remaining native gates or advance the dashboard.

Checkpoint validation:

- `cargo test --locked -p mapgyver_native_probe --lib`: 38 passed.
- `cargo test --locked -p mapgyver_native_probe --features desktop`: 52 passed; six hardware
  tests ignored. No new native timing, scanout or hardware qualification is claimed.
- `python -m unittest discover -s tools/mapgyver/tests`: 36 passed. Two color-accuracy tests
  now mock platform inventory so their native-process mock cannot intercept Windows platform
  detection; the first run exposed that fixture error and the corrected suite passes.
- Probe UI `check`, `check:generated` and `build`: passed using `npm.cmd`; Svelte reports zero
  errors/warnings and Rust/TypeScript DTOs match. All 100 MapGyver evidence JSON files parse.
- Required `cargo fmt --all` at the root and Golden Core manifests completed.

The follow-up coherent checkpoint adds NVIDIA fixed-card accuracy, the first working official
PresentMon capture and strict CSV analysis, plus lossless large-integer evidence transport.
Its independent review recomputes all 1,192 trace rows exactly and verifies the four NVIDIA
reports against their original stdout. All 44 Python tests pass again; required root/Core
formatters and whitespace checks pass. Native sources remain unchanged from their existing
52-test checkpoint, so no new native build or hardware result is inferred. This checkpoint
also leaves M01 at 1/8 and overall progress at 1/10.

Publication through the live-counter/warmed checkpoint is verified: the remote query returns
`0e4a6deba1bbcffddb555e65750090bc05a42a44` for `refs/heads/mapgyver` at
`git@github.com:Golden-Geek/Chataigne2.git`. This external publication clears the earlier
automatic-review rejection. That checkpoint was pushed and upstream/remote equality verified
before this batch. The output-only policy control, sampled admission and complete retained
three-run evidence follow that baseline and must be delivered before the next batch.

Milestone `8138e397` (`Test MapGyver output presentation with sampled backpressure`) and delivery
note `b07c9753` are now published. The user explicitly approved these commits, including the
reviewed code/tests/docs, three diagnostic packets (10,542,779 bytes total), plan and analysis,
to `git@github.com:Golden-Geek/Chataigne2.git`, branch `mapgyver`. The push succeeded and an
independent remote query returned `b07c9753f923d8492945d9e8e7aa8a654f6426ea`, matching local HEAD
with zero ahead/behind commits and a clean checkout. This resolves the earlier automatic-review
rejection and publication blocker; the M01 acceptance blockers above remain unchanged.

Milestone `4fd531d2` bounds renderer imports by weak allocation identity and splits handoff
timings. It is published on `origin/mapgyver`; an independent remote query returned
`4fd531d23de4f11c2e539cbd933051bc9f588602`, matching local HEAD with zero ahead/behind commits
and a clean checkout before this delivery note. [Its native evidence](mapgyver-m01-native-feasibility.md#bounded-renderer-imports-and-split-handoff-timing--2026-10-28)
and [strict analysis](evidence/mapgyver-m01-import-cache-analysis-2026-10-28.json) retain two new
Intel captures and 4K cross-process accuracy. Sampling performs no resource opens; each cache
peaks at three aliases and drains to zero. The sustained run still accepts 6,548/7,200 output
attempts with 44 CSV drops, so it closes no additional gate. The longer explicit collector tail
allows both live queries to succeed without changing the older unavailable observation.

### M01 acceptance items

| Item | State | Accepted scope / remaining work |
| --- | --- | --- |
| Ordinary underlay/DOM/native-input functional feasibility | Done | Native observations at 100%/125% establish the scoped route; full DPI/input/latency qualification stays with M05. |
| D3D11/Spout two-source GPU composition and caller-device ownership | Done | Independent processes, finite pixel-accuracy checks and GPU-only frame transport establish functional feasibility. Renderer budgets stay with M03. |
| Scoped editor/output independence, resize/restart and ownership cleanup | Done | Recorded interruption/reconnect/rebuild smoke establishes the functional mechanism. Sustained retirement/recovery budgets stay with M03 and owning features/M09. |
| Host/backend/binding decision with explicit later-gate ownership | Done | Ordinary Tauri/Wry underlay, D3D11 and pinned `spout2-sys` 0.1.1 are selected for the thin shell; full qualification is explicitly not passed. |

Evidence and limitations: [functional route decision](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)
and [fixed qualification contract](mapgyver-qualification-contract.md). This is the user-approved
functional gate, not a retrospective pass of the original eight-item matrix.

### M02 acceptance items

| Item | State | Accepted scope / remaining work |
| --- | --- | --- |
| Public UI-bundling boundary | Done | [Public helper](../../crates/golden_core/support/codegen/src/ui_bundle/mod.rs) adopted by Chataigne; 19 support tests, strict Clippy and a real app build check with UI bundling enabled pass. SDK/formula/node/Tauri steps remain app-owned. |
| MapGyver host/UI scaffold | Done | [Thin shell](mapgyver-m02-shell.md#implementation-boundaries) uses Golden's public host/workbench, app identity, project format/data directory, resources and capabilities. Fresh projects have the standard Dashboard; no placeholder Media/Screen managers. |
| App branding and browser-state isolation | Done | [Backend metadata and browser namespaces](mapgyver-m02-shell.md#implementation-boundaries) separate layout, selection, recent path and credentials; app projects carry a validated identity. Stream Deck's editor belongs to Chataigne. |
| Generic discovery and endpoint isolation | Done | [Rust-owned discovery DTOs](mapgyver-m02-shell.md#implementation-boundaries), generated protocol 0.13.0 and distinct backend/frontend ports; app identity is checked before UI admission and on fresh credentials. |
| Explicit app selection in tools | Done | [Cargo/npm/xtask/watch/package commands](mapgyver-m02-shell.md#reproduction) select MapGyver explicitly; root defaults remain Chataigne. |
| Typed optional native-presentation host lifecycle | Done | [Public extension and native smoke](mapgyver-m02-shell.md#optional-native-presentation-lifecycle) keep window construction/UI preparation at Golden's host, task resources on its worker, HWND destruction after retirement, and joining after the event loop. Default/headless paths do not prepare presentation. |
| Combined launch and boundary verification | Done | [Concurrent bundled/dev/headless runs](mapgyver-m02-shell.md#live-windows-verification--2026-10-28), own save/reopen and foreign-file rejection pass; engine/protocol packages and both full desktop apps compile and test through public boundaries. Native quality remains unqualified. |

### M03 acceptance items

| Item | State | Accepted scope / remaining work |
| --- | --- | --- |
| Immutable render/resource plan compiler | Done | [Compiler evidence](mapgyver-m03-render-foundations.md#immutable-render-plan-compiler--2026-10-28): explicit texture/color/alpha, deterministic whole-graph validation/demand compilation, bounded raw work/checked logical bytes, 29 tests including a 10k chain. No runtime/native admission pass is inferred. |
| Bounded worker/null runtime and atomic install | Done | [Runtime evidence](mapgyver-m03-render-foundations.md): Golden IO workers/fences, one preparation/latest desired plan, bounded reliable outcomes and resource admission, frame-boundary install, at most two in-flight uses/two retiring generations, completion-based worker-local destruction. Null resources simulate allocation/use; native proof stays open. |
| D3D11/Spout backend and host integration | Done | [Native input/output evidence](mapgyver-m03-render-foundations.md#bounded-spout-inputs--2026-10-29): opaque GPU resources, explicit LUID/caller-device ownership, four-mode conversion/composition, independent-process Spout inputs and window/Spout output consumers pass. Shared-host regression passes. Full recovery/steady-state budgets retain gates 4/9; physical presentation remains M05. |
| Runtime observations and device recovery | Open | [Startup retry](mapgyver-m03-render-foundations.md#admitted-startup-retries--2026-10-29) and [endpoint recovery](mapgyver-m03-render-foundations.md#independent-native-endpoint-recovery--2026-10-29) verify baseline admission, isolated output retry/state/fenced replacement, source withdrawal and simulated failed-reset/cancellation/project fencing. Genuine native removal/recreation and stuck-drain recovery/retirement bounds remain required. |
| App-owned media/channel backend contracts | Done | [Authoring and live bridge](mapgyver-m03-render-foundations.md#app-channel-publication-bridge--2026-10-29): typed persistent references, backend defaults/operations, generated contracts, demanded-leaf capture and guarded plan publication. History/reload, source changes/resize/reconnect and controlled native runtime/device recreation preserve channel identity. Genuine device-removal recovery stays gate 4; dedicated manager controls/commands/templates stay M04. |
| Shared production-time authority | Done | [Golden Time](../../crates/golden_time/README.md) supplies checked coordinates/rational mappings and authority/discontinuity epochs consumed by the threaded video scheduler; fake-time activation/shared-output and stale-epoch tests pass. Shared P14 plan records this evidence; audio/DMX acceptance remains open. |
| Shared persistent asset/preparation boundary | Done | [App Picture/asset integration](mapgyver-m03-render-foundations.md#app-assets-and-picture-resources--2026-10-29): app-owned stable references/locators, sparse/full codec/history/QuickJS, missing/replaced/relinked/relocated sources, bounded worker decode and retained CPU/GPU resources. Native upload/share/recreation and stale installation rejection pass. Chataigne audio/collect/scheduling P15 acceptance and full native qualification remain open. |
| Native preview registration/geometry contracts | Done | [Preview contract/native consumer](mapgyver-m03-render-foundations.md#native-preview-registration-and-geometry--2026-10-29): Rust-generated DTOs, host-owned metrics, coalesced leased layout, Svelte clipping/visibility/retirement, finite crop/clear pixel oracle and actual shared-host lifecycle pass. Specialized editors/full physical/DPI/input stay M05–M07. |
| Native renderer qualification | Open | Unchanged Q01/Q02 CPU/GPU/cadence/frame-age/admission/memory/handle/shutdown budgets and applicable recovery against the production replacement; historical failures preserved. |

The [nine-item definition](../plan/MapGyver_Implementation_Plan.md#m03--deliver-renderresource-and-channel-foundations)
covers the original complete M03 acceptance. This is a denominator breakdown from 1 to 9, not a
scope reduction or a native pass. Time and asset primitives missing at their shared Golden owners
must be implemented and exercised once; audio-specific clocks/caches are not app project truth.

## Detailed status

Updated: 2026-10-29. Entry point: [MapGyver implementation plan](../plan/MapGyver_Implementation_Plan.md).

**State: M00, revised M01 and M02 complete locally; M03 implementation in progress.**
The user explicitly requested implementation over successive turns on 2026-10-27, then requested
a MapGyver branch. `mapgyver` was created from `137c9203` with the existing dirty checkout intact,
then externally fast-forwarded from `main` to the Chataigne P11 checkpoint `40acd67d` during M00.
The checkout was reconciled at external checkpoint `73548569` (`Checkpoint MapGyver baseline
and partial native feasibility`). NVIDIA/PresentMon checkpoint `8e4615b5` and native-role
checkpoint `203970bf` and live-counter/warmed checkpoint `0e4a6deb` are published; this batch
starts from the latter at `origin/mapgyver`.
M00 delivers the baseline tooling and fixed qualification contract. M01 owns an isolated native
Spout/D3D11 harness with locked Tauri/Svelte underlay/output windows, an independent consumer,
source-worker GPU bridges, bounded admission/retirement and lossless timing/resource evidence.
It has not delivered a production MapGyver shell or Golden protocol change.

The current accuracy diagnostic adds a CPU oracle for the existing procedural-card shader.
Intel fixed 1080p/4K cases compare all 10,368,000 composite pixels through independent processes
and the real source-worker bridge, with no exclusions and maximum component error 1/255 against
a fixed 2/255 tolerance. Producers upload only one 32-byte uniform each, with no CPU pixel
uploads. Matching authored-image controls share the same native checkpoint and captured wrapper. This establishes
fixed-card shader/transfer accuracy. NVIDIA now passes the same four cases on identical native
inputs. Those captures retain their exact earlier binary. The new diagnostic release reports
ephemeral presenter identities only in CLI generation reports; product DTOs/UI/persistence remain
unchanged. Short Intel Q01/Q02 traces match every raw row to editor/output roles using exact
PID, queried DXGI aliases and Present-QPC bounds. Q02 retains 85/20 drops out of 480 Presents
per role despite native accepted-call counts of 480/480. The later 30-second-warmup control
keeps Present flags/geometry unchanged and records no missing source publications during its
sample, but still observes one output drop/480 Presents and five editor readiness gaps.
Its independent live ETW query succeeds after the recorded Present intervals with all three
loss counters zero; final shutdown/analysis loss remains unverified. Report-only window
observations identify primary 240-Hz/125% and secondary 60-Hz/100% nominal configurations.
Role mapping is verified; physical displays, animated/lifecycle, previews and full performance remain open.
The earlier lifecycle build
verifies Intel/NVIDIA output through >60-second busy and blank-document windows, then fresh
preview reconnect. Those captures and startup-invalid markers retain their exact earlier inputs.
Before the lifecycle-fixture change,
Intel/NVIDIA Q01/Q02 each collected three full release samples on identical inputs: 86,395 exact
dependency joins within scoped call/CPU/age targets. Their fingerprints stay historical. Full
asynchronous GPU critical path is still open. See [native evidence](mapgyver-m01-native-feasibility.md).

Earlier failures remain authoritative for their exact fingerprints: stalled cadence, a
207.798-ms combined QueryEnd interval, source-stop ordering and 47.926-MiB positive private
growth against 32 MiB. Stop ordering and timestamp-free event-query admission were repaired;
the old QueryEnd delay and memory-retention cause remain unproven. A later reconstruction
meets relative growth in one cohort with a higher absolute peak, so it does not close memory.

NVIDIA qualification requires verified overlay Off before startup. The approved on/off/on
control isolates handle growth to enabled conditions; no speculative driver teardown is added.
The earlier lifecycle turn restored On at 08:17:51 UTC. Current card-accuracy observations find
an already-Off setting at 09:19:55 and confirm Off at 09:22:32; it stays unchanged. Both adapters'
card captures share identical native source/assets/executable. The wrapper's transport-only
change protects large integer evidence. No overlay, display, driver or security setting changes
are made during this turn.

Primary 125% and secondary 100% native input sequences are recorded, including hidden/minimized
intervals over 60 seconds. [ETW preflight now passes with the administrator token](mapgyver-m01-native-feasibility.md#etw-access-after-administrator-relaunch--2026-10-28);
the agent sandbox still returns access denied. Displayed cadence, physical latency/alignment,
remaining display configurations and complete retirement stay open.
Chataigne's separate [ledger](implementation-status.md) and default root launch remain intact.

## Confirmed product decisions

- Product name: **MapGyver**, planned at `apps/mapgyver`, alongside Chataigne.
- Shared Golden Core and UI; Windows x64 first.
- Each Shared Texture channel receives its own Spout sender; channels run concurrently.
- Both preloading and postloading are distances in neighboring Matrix columns.
- Required media, screens/surfaces and four editors are recorded in the
  [architecture](../architecture/mapgyver.md); research is separated from native proof in
  [native rendering findings](../architecture/mapgyver-native-rendering.md).

## Milestones

| Milestone | State | Evidence / next acceptance |
| --- | --- | --- |
| M00 Reconciliation/workloads | VERIFIED_LOCAL | [Checkout/public-boundary/hardware handoff](mapgyver-m00-baseline.md), [captured inventory](evidence/mapgyver-baseline-2026-10-27.json), and fixed [qualification workloads/budgets](mapgyver-qualification-contract.md) |
| M01 Functional route decision | COMPLETE_LOCAL | [Approved decision](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28): ordinary Tauri/Wry + D3D11 + pinned Spout sys binding permit the thin shell. Full qualification remains with M03/M05/M09. |
| M02 Multi-app seams/shell | COMPLETE_LOCAL | [Shell/isolation and typed host lifecycle](mapgyver-m02-shell.md) close all seven fixed items. The real Windows smoke verifies STA/worker/window teardown; no GPU or physical-output qualification is inferred. |
| M03 Rendering/channels | IN_PROGRESS | [Compiler, runtime, native execution, channels, shared time/assets and preview contracts](mapgyver-m03-render-foundations.md) close 7/9 fixed gates. Production recovery and native qualification remain open. |
| M04 Source media | NOT_STARTED | Solid Color, Picture, concurrent Spout scope defined |
| M05 Screens/Surfaces/editors | NOT_STARTED | Output/crop/pin/calibration contracts defined |
| M06 Composition/editor | NOT_STARTED | Instance/canvas/dependency contracts defined |
| M07 Matrix/editor | NOT_STARTED | Layers/groups/exports/transition contracts defined |
| M08 App lifetimes | NOT_STARTED | Neighbor demand and owned-process lifecycle specified |
| M09 App/artifact qualification | NOT_STARTED | Requires implemented product and fixed native workloads |

## Next action

The user approved the scope correction on 2026-10-28 and explicitly instructed continuation
to M02. Its shell/isolation and typed host lifecycle deliveries now close all seven fixed
items. M03's fixed compiler, worker/null-runtime, shared-time and app channel-contract gates are verified.
The D3D11 compositor, Spout source workers, window/Spout outputs and shared-host consumer are
exercised. The app-owned bridge publishes guarded channel plans and preserves references through
history/reload and controlled native resource recreation. Shared persistent asset IDs/locators,
worker preparation/cancellation and consumer-lifetime admission now have a reusable owner.
App asset records now exercise project/history, Picture preparation and native resources (gate 7).
Reusable native preview registration/geometry now passes its real shared-host consumer (gate 8).
Independent output retry/state, completion-fenced replacement, source withdrawal and simulated
failed-reset/cancellation/project fencing have a recovery checkpoint. Initial allocation now
retries within its original admitted baseline and first device generation, serving controls and
retaining only the latest desired plan. The production Q01/Q02 runner and its short native controls
now complete. Their traces retain all rows and exactly anchored software slots.
Native output admission now precedes conversion, preserving readiness and retirement semantics.
Public source copied/freshness status is now truthful, versioned unchanged polls skip GPU batches,
and static sources survive a short demand pulse or suppressed publication. Unknown external
senders remain unversioned. Preparation/frame identities now measure native preparation
and first use. Compatible native texture/view sets are reused without retaining unused storage
or asset leases; conservative per-generation admission is unchanged. Both short controls reuse
all seven textures after initial allocation, with later preparation below 0.011 ms. Q02 still
misses cadence/age targets. Schema 4 joins copied-image labels across producer, source use,
output and independent capture, checking GPU pixels against the exact claimed renderer inputs.
Direct native regression and GPU input/conversion taps isolate the observed mismatch to the
outgoing SDK handoff. GPU keyed outputs with zero-timeout send ownership now retain exact pixels
in 518 available regression comparisons and the new short control captures. The Q02
window-Present control misses 18 sample images and cadence/age targets. Counter agreement alone cannot
qualify identity.
CPU publication/first-receipt/installation records now join exact resource generations to first
use and per-output API age. The bounded recorder exists before worker startup; unread
superseded plans have no invented receipt. Retained plan age grows between edits and is not
edit latency. Optional PID-filtered PresentMon now joins each actual window API call to its
software display event and exact retained sources/plans. Q01 has complete three-output age
observations in one short control; Q02 retains cadence/age/capture failures. Pending software
display events do not establish native buffer occupancy. Anchored delivery-deadline analysis
now retains every opportunity, including a pre-window API call whose deadline belongs to the
sample. Q01 has 180 on-time observations per output; Q02 window deadlines fail, while late
Spout polling stays uncertain. Source-local hardware sRGB decoding now reduces isolated sampler
cost by 33%; its short Q02 control still misses six of 295 accepted Spout images and does not
improve renderer-context GPU p95. Schema 7 now partitions every sample renderer span on its
own GPU counter. Outputs dominate 271/300 Q02 frames in the prior control, motivating the
completed direct-window change at that output boundary.
Direct window rendering now removes two conversion textures and copies. Its Q02 control has
full main acceptance and exact accepted-Spout coverage, but editor/Spout/producer cadence and
the GPU budget remain insufficient. The output interval's median stays near 9.5 ms even as
its p95 and the enclosing renderer p95 fall; do not treat that as a complete critical-path fix.
The direct input receive path now removes private capture staging and one GPU copy per input.
The next control retains complete main/Spout acceptance and exact captures, but producer/editor
cadence remains insufficient. Its higher logical reservation peak is preserved, despite the
smaller per-source allocation; native buffer/queue and generation-overlap evidence remain distinct.
The bounded DMA decoder experiment now preserves all 16,272 owned packet records, with all
timing fields partial. Its zero reported ETW loss does not establish queue coverage. Do not
repeat the built-in WPR GPU profile without a concrete decoder/provider change.
The opaque shader branch now skips unused transfer math for linear targets. Its isolated
4K draw control improves about 15%, and branch-only Q02 renderer-context p95 falls to 11.4896 ms.
Q01 tails regress and source-A admission worsens; accepted-Spout capture coverage remains
complete. The rejected hardware-Spout/SDK path is fully removed. Publication/dependency
ordering was the next experiment: both staged conversion and cold-prepared shared-first
schedules now regress Q02 and are removed. Their complete evidence is retained, including
the shared-first main/editor shortfalls. The sender/receiver ownership investigation at the
source publication boundary now gives isolated fixture
producers an explicit 2 ms per-lock wait, while renderer sends remain at zero. The versioned
schema 8 controls preserve both independent 60 Hz source contracts and all existing budgets;
their samples have complete source/output acceptance and exact captures. Q02 editor minimized/
disconnected phases still fail cadence/capture/age, including 14/four missing accepted Spout
images. The transition investigation now reproduces recursive renderer ownership with a
static source. Slot-owned reader leases and latest-use retirement fix it; truthful fault
reporting replaces the old fabricated removal code. Both sources remain in generation 1 in
the new controls, with no unversioned source-use frames during the measured phases. Exact
window/software-display rows now cover all accepted sample calls. Q02 still misses four/nine
Spout images during minimize/disconnect, with capture-age and sample editor cadence failures.
Its consumer GPU-context spans reach 65.509 ms despite short SDK API calls; these spans do not
calibrate queue delay or GPU readiness. Use the retained traces to attribute those remaining
stalls and establish supported clock/queue observations before further GPU-work changes.
Do not infer full independence from sample counts or repeat the same instrumentation.
Do not start another output-order variant or change fixture pacing to hide Busy outcomes.
A gap between different benchmarks is not measured queue time. No graphics settings were changed.
Complete GPU critical-path and queue/consumption measurement
before the prescribed qualification series (gate 9).
Genuine native device-removal recovery and bounded stuck-drain retirement retain gate 4.
Both gates remain required before M03 completion; all existing budgets/failures stay operative.
Deliver each coherent batch
on `mapgyver` before starting the next. Preserve
Chataigne's root default and unfinished P11 work. No new M01 captures, duplicate consumer-only
cohorts, overlay investigation or identical WPR profiles are needed. Future clock/queue
instrumentation requires a concrete supported measurement change. The user confirms overlay Off.

**Approved scope correction — continuing acceptance ownership:**

| Milestone | Responsibility |
| --- | --- |
| M01 | Record functional feasibility and a host/backend/binding choice from the existing native evidence: ordinary Tauri/Wry underlay, D3D11, pinned Spout binding, two GPU sources, composition, editor independence and scoped ownership cleanup. Explicitly retain limitations and failed qualification results. |
| M02 | Proceed with the thin app shell and public multi-app/host seams after that functional decision. This does not approve production editors or claim qualified rendering. |
| M03 | Own reusable renderer scheduling, measured GPU/CPU budgets, resource bounds and applicable recovery/retirement fixes before accepting those mechanisms. |
| M05 | Own correct physical output placement, preview alignment, quantitative input/layout latency and the full display/DPI/input matrix before accepting those features. |
| M09 and owning features | Run the complete applicable sustained/recovery qualification against the actual app; preserve the fixed Q01/Q02 budgets and historical failures. Full product/Matrix Q05 remains here and with the features that introduce its behavior. |

The approved M01→M02 dependency is the functional route decision. The four-item M01 gate is
closed in its recorded scope; the original eight-item full-qualification matrix was **not
passed**. Its measured failures and unavailable checks retain their continuing owners. Future
native work belongs to the feature being delivered, with a bounded corrective batch and a
pass/fail checkpoint; a failed checkpoint produces a decision rather than a diagnostic loop.

Internal root-cause attribution, mandatory dual-vendor success, exhaustive SDK-internal copy
accounting, physical residency proof and optical scanout research are not additional gates to
creating a thin shell. Unknown observations remain unknown. The fixed required measurements
and budgets are not waived by this distinction. The plan, architecture, qualification contract
and evidence decision are revised together with this ledger.

Preserve Chataigne's unfinished P11 work and its separate ledger. No production source,
protocol, display configuration or overlay setting was changed by this review.

### Bounded owned-output fixture correction — 2026-10-28

The harness no longer chooses `available_monitors()[1]`. It selects only attached displays
belonging to the requested adapter, prefers a unique exact-size match and otherwise requires
one fitting display. Missing/ambiguous fixtures fail before window/GPU startup. A selected
4K adapter cannot silently use a different adapter's output or span smaller monitors.

Fixed-card output is borderless, shadow-free and nonresizable. Two ordered synchronous
owned-HWND setup calls first relocate into the target DPI context, then set the final physical
extent. After show, exact client origin/extent, monitor identity and containment must match
before the renderer, fixtures or consumer start. There are no placement retries, polling,
display-setting changes, new timing probes or protocol changes.

`cargo test --locked -p mapgyver_native_probe --features desktop` passes **104 tests**
(57 library, 47 binary), including nine new deterministic display-selection regressions;
seven hardware tests remain ignored. Strict desktop/all-target Clippy passes. Native
placement/DPI behavior is **not yet verified**; no capture or native GPU test was run.
At that source checkpoint the scope proposal was still pending and M01 was **1/8** under its
original definition. The later approval above changes ownership, not those test results.
This source fix does not establish a cadence, geometry or resource pass.

### Historical diagnostic backlog — superseded as an execution queue

The following retains the previous checkpoint's rationale and then-current next steps. It is
historical evidence, not the active execution queue; use the approved M02 action above.

Continue **M01**, the [native feasibility gate](../architecture/mapgyver-native-rendering.md#native-feasibility-gate),
on branch `mapgyver`, using the owned [native harness](../../tools/mapgyver/native-probe/README.md).
M02 depends on this gate; do not build production editors/shell around an unqualified underlay.

- The user-confirmed 4K display is connected as `DISPLAY5` on NVIDIA adapter 1, LUID
  `00000000:000170d3`, at `[4480,0,8320,2160]`, with 150% DPI observed in the rejected smoke.
  The Intel paused smoke remains rejected: native output is 3834×2143 and seven warmup CSV
  Present QPC transitions reverse within one swapchain. Fix owned-output placement and
  per-monitor-DPI sizing before another capture; preserve raw order and do not discard the
  failed smoke. Explicitly select and predeclare the new display/adapter route, and observe
  NVIDIA overlay Off before NVIDIA startup. One named adapter is the M01 requirement;
  mandatory dual-vendor success is not an additional gate.
- Pre-lifecycle Intel/NVIDIA Q01/Q02 release cohorts cover both fixed workloads with identical
  native inputs within that cohort. Preserve the earlier cadence/QueryEnd and memory failures; the split stages
  make a recurrence attributable to a smaller wall interval but establish no internal cause.
  Do not repeat unchanged performance cohorts in place of missing evidence.
- The predeclared output-only latest/vblank/latest control uses one new frozen release and
  matching window snapshots. B loses 31 native output submissions despite zero CSV drops;
  A2 latest drifts to 5,372/7,146 accepted attempts against 7,200 nominal slots. Do not adopt
  vblank or treat this as a completed qualifying series. The subsequent weak-identity import
  cache preserves reader ownership, passes full-image/native ownership checks and performs
  zero sampled opens in 30+8/30+120-second Intel controls. Long sampled poll stalls do not recur,
  but the sustained sample still loses 652 native output submissions (221 GPU coalesced plus
  431 readiness gaps) and has 44 CSV output drops. Investigate GPU/context scheduling and
  presenter readiness with a controlled experiment; do not assign historical stalls a cause
  from this uncontrolled comparison. Explicit ten-second collector tails permit both live
  queries to succeed; preserve A2's earlier status-4201 failure and unverified final loss.
- The opt-in stage diagnostic retains all 2,277/8,963 whole-frame joins for its short/sustained
  staged runs. Its largest sampled interval contains 143.0630 ms between post-composition and
  post-Spout markers, with zero observed Presents in that batch's CPU bracket. Isolate Spout
  send/consumer sharing and GPU scheduling before moving responsibility or claiming an SDK
  cause. Output still loses 75/7,200 native submissions and has 29 CSV drops; the return-to-off
  short control also drops 76/480 output Presents despite full native acceptance. Preserve
  separate GPU-pressure and presentation behavior, and do not infer equivalence from short
  off/on/off distributions. One sampled import per source remains visible in the long run.
- Both adapters' JS-busy/blank-document continuity now has native evidence and fresh reconnect
  ownership. Intel finite authored-image accuracy now compares every pixel through independent
  processes and the real source-worker bridge at 1080p/4K. The fixed procedural-card shader also
  matches its CPU reference, including frame digits and marker words, on both adapters. Continue
  lifecycle and preview/physical-output accuracy; fixed-image checks do not close those gates.
  Investigate supported cross-context GPU timing/correlation before treating context marginals,
  serial sums or CPU envelopes as the full GPU critical path. Source selection, budgets and
  native routes must stay truthful.
- Investigate private retention with controlled ownership/thread/report-data comparisons,
  preserving the warmed generation-10 baseline and every live peak. The later relative-growth
  pass has a higher absolute peak and cannot establish a memory repair. Full Q05 remains open.
- ETW provider/event capture, native role correlation and an independent live counter query now
  work outside the sandbox under the administrator account. Preserve the short-control Q02
  drops (85/20 per 480 Presents) and warmed-control output drop (1/480) plus five editor readiness
  gaps. Investigate steady GPU/compositor deadlines without treating 60 accepted calls/sec as
  displayed cadence. Live loss counters are zero at the successful query; final shutdown and
  PresentMon analysis loss remain unverified. The
  sandbox account still returns access denied (5). Use the working host execution path.
  The development account is already a Performance Log Users member; non-elevated access after
  a fresh Windows sign-in remains unverified. See [ETW access evidence](evidence/mapgyver-m01-etw-access-2026-10-28.json).
  Empty-session success, one-frame DXGI queue limits, accepted Present counts and consumer
  frame age do not qualify displayed cadence, scanout or physical input/geometry latency.
  Keep the renderer free of pixel readback.
- The available 100% secondary input sequence is now observed alongside the 125% primary.
  [Secondary evidence](evidence/mapgyver-m01-secondary-native-ui-2026-10-28.json) adds 32 native
  inputs plus activation restore, 65.458 seconds hidden and 79.263 seconds minimized.
  NVIDIA hidden/minimized input cases, OS 150/200%, negative origins, physical-pixel alignment,
  full-card qualification and physical 4K scanout remain open. Do not change monitor settings
  automatically. Use the computer-use skill for authorized native UI work.
- Long interactive captures should use the runner's explicit diagnostic underlay mode. The
  original 608-second native report is terminal-truncated; its surviving fields and native
  gesture observations are retained, with missing rows explicitly unavailable. The separate
  short runner control validates compaction, not reconstruction of the omitted report.
- NVIDIA qualification requires [overlay Off before process startup](mapgyver-qualification-contract.md#nvidia-overlay-prerequisite),
  with native verification and preservation/restoration of the current original setting. Latest
  card measurements started and ended with an already-Off setting; no toggle was performed.
  The on/off/on controls reproduce handle growth only when enabled, with the same loaded DLL;
  preserve those failures and do not add a speculative driver-teardown workaround.
- Both GPUs historically complete 100 controlled device rebuilds; the NVIDIA overlay-off
  [live-scene release capture](evidence/mapgyver-m01-scene-retirement-2026-10-28.json) adds a
  ten-second final live interval within scoped handle/private-growth targets. Complete Q05,
  genuine device removal/reset, physical retirement and complete-process/WebView shutdown
  remain open. Do not treat logical post-fence zero ownership as physical residency proof.

Renderer/source/consumer descriptor ownership, process DXGI observations, explicit harness
copy counts, bounded frame-age logs and per-output admission/cadence counters are available.
SDK-internal copies, physical residency and displayed statistics retain their narrower limits.
Keep desktop-capture sessions inactive during timing runs; prior slowdown/failure evidence
remains retained without an established cause.

Preserve Chataigne's unfinished engine/Spatializer P11 checkpoint (four of six fixed checkpoints)
and its [separate ledger](implementation-status.md). Shared P12 boundary repairs belong to M02;
P14/P15 time and asset contracts retain their shared Golden owners under M03. Root launch
behavior is unchanged. No production host/backend/binding decision is final yet.

## M03 shared reader ownership — 2026-10-30

A controlled static-source regression reproduces `DXGI_ERROR_INVALID_CALL` when a second
prepared target recursively acquires the same renderer device's already-held keyed mutex.
Previously, this caused source fallback/recovery; the old worker also converted every
ownership fault into `0x887A0005`, so that historical report cannot prove physical device loss.
Each input now has one fixed reader lease per bridge slot, reused for the same immutable
allocation and retained through its latest renderer completion serial. Retirement stays
completion-based, with no new texture, copy stage, queue or per-frame reader allocation.
Source recovery publishes the actual fault code before its request flag; actual source-device
removal status remains separate. Renderer send wait stays zero, source wait stays 2 ms per lock.

The [paired controls and exact display evidence](mapgyver-m03-qualification-runner.md#shared-reader-ownership-across-plan-generations--2026-10-30)
keep both sources at generation/pool 1, with no reported fault and no absent source-use
publication in the measured phases. The ten Q02 minimized-phase lineage-invalid captures are
gone. Both samples keep 300/300 exact accepted-Spout captures; Q02 editor accepts 295 calls.
All accepted sample window calls have exact software-display rows, but anchored deadline
failures remain. Q02 minimize captures 55/59 Spout images; disconnect captures 51/60 and
covered source ages reach 62.1145/66.1716 ms. Its instrumented renderer p95 is 15.8522 ms,
compared with 11.7707 ms before; added PresentMon means no isolated timing comparison is claimed.

The packet/archive retain two new native captures, per-phase image/software-display audits,
269 execution sources, the failing native repro and passing validation. Independent audit
checks all 1,199 sample GPU partitions and 1,647 exact accepted-Spout images across the sample/
interruption cohorts, preserving missing images and unknown pre-fix lineage. Validation passes
154 ordinary Rust, 17 Golden native, two app-native and 275 Python tests, strict clippy,
release build and both formatters. The native regression verifies two exact target images,
latest-fence retention and logical-fault recovery without a fabricated removal code.
**M03 stays 7/9; overall stays 30%.** Full GPU/queue, prescribed cadence/interruption/growth,
display deadlines/age and genuine removal/stuck retirement remain open.

## M03 explicit source send ownership wait — 2026-10-30

The detailed send API now takes a finite timeout per ownership lock. Renderer outputs always
pass zero, preserving independent healthy output progress; isolated fixture sources request
2 ms on their own workers. This separates renderer admission from an independent source's
60 Hz publication duty without adding retries, catch-up bursts, queues or textures. Busy still
copies no shared pixels and advances no version. The request does not bound opaque SDK/driver
wall time or close stuck-driver retirement. The existing SDK timeout overload implements it.

Rust owns the policy and generated capture metadata. Artifact schema 8 records the source wait
in initial/final snapshots; analysis rejects missing, excessive or changed policy and retains
the original cadence denominator. Old schema 7 evidence stays unchanged with its original
zero-wait source implementation. The vendor's three binding files and nine patch hashes are
updated; upstream SDK implementation files are unchanged.

The [paired short controls](mapgyver-m03-qualification-runner.md#isolated-source-send-ownership-wait--2026-10-30)
accept 300/300 publications from each source and 300 calls on every output at both sizes.
Each retains 300/300 exact accepted-Spout captures and the original logical memory peak.
Q02 renderer GPU p95/p99 is 11.7707/15.2588 ms, compared with the published 11.4896/13.5165 ms
baseline; improved source admission is not a GPU-tail improvement claim. Q02 minimized-editor
main/Spout counts fall to 56/59, with only 45/59 accepted Spout images captured. Disconnect
captures 56/60 and covered source-B p95 age is 57.0987 ms. Complete-run source admission-busy
counters remain five/seven; no sample or interruption failure is hidden.

Native held-ownership tests exercise both 0 and 2 ms: Busy leaves the image/version unchanged,
and release resumes exact pixels at the next counter. Validation passes 154 ordinary Rust,
17 Golden native, two app-native and 275 Python tests, strict Golden clippy, release build,
generated contracts, both formatters and vendor hash checks. The packet/raw archive preserve
268 execution sources, both controls and validation logs; independent audits reconstruct all
1,200 GPU partitions and exact accepted-Spout image joins including the unchanged baseline.
**M03 stays 7/9; overall stays 30%.** Editor-interruption, exact displayed deadlines/age, full
GPU/queue and applicable growth qualification, genuine removal and stuck retirement remain open.

## M03 rejected output-order experiments — 2026-10-30

Two bounded output-order experiments did not justify a renderer change. Staging every output
conversion before publication regresses short Q02 renderer-context GPU p95 from 11.4896 to
14.3252 ms. A simpler schedule prepared once per generation publishes shared outputs before
window conversion/Present, but regresses Q02 p95 further to 17.4177 ms and accepts only
270/300 main and 251/300 editor calls. Q01 p95 is 4.7699/3.7407/4.8264 ms for baseline/staged/shared.
These sequential short controls do not isolate cause; neither experiment earns retention.

Both implementations and their three schedule tests are removed. Golden Video native sources
match published `b68a5f06`; its shader branch, resource pools, source pacing, fences, copies,
SDK and plan-order output delivery remain intact. The retained app-owned offline audit observes
the rejected shared-first hypothesis on exact CPU frame tokens. It is descriptive, outside
the acceptance criteria, and correctly returns false for the restored plan order. CPU order
does not certify GPU readiness or queues.

The [packet](evidence/mapgyver-m03-shared-first-2026-10-30.json) and
[raw archive](evidence/mapgyver-m03-shared-first-controls-2026-10-30.zip) retain four new controls,
both rejected execution-source sets, validation logs and six independently reconstructed
cohorts including the unchanged published baseline. All 1,800 GPU partitions and 1,800 exact
accepted-Spout image/source-marker joins verify; no missing accepted image is discarded.
The shared-first property covers all 600 final experiment frames with zero unknowns, despite
its cadence/performance regression. Fixed acceptance remains **7/9; overall 30%**.

Experimental validation passes 156 ordinary Rust, 17 Golden native, two app-native and 274
Python tests, strict clippy and release build. After rollback, native source equality retains
the baseline's 153 ordinary/17 native/two app-native evidence; those hardware checks are not
claimed as newly rerun on the restored tree. Final Python, both formatters, whitespace and
archive checks and a rebuilt release executable validate the retained tools/docs checkpoint.
Source admission, complete GPU/
queue and cadence/growth coverage, genuine removal and stuck retirement remain open.

## M03 opaque output branch — 2026-10-30

The production change is one uniform shader branch that skips unnecessary sRGB transfer math
for opaque linear targets, including the existing hardware-encoded window path. SDK, wire
formats, resource pools, scheduling and generated contracts are unchanged. New native tests
check 96 independent finite color/alpha cases and alternate forced-flatten/branch draw controls.
The isolated 4K median is 0.802858 → 0.683555 ms, a 14.86% reduction, without a full-path claim.

The [paired native controls](mapgyver-m03-qualification-runner.md#opaque-output-shader-branch--2026-10-30)
retain fresh before-runs, a rejected combined hardware-Spout experiment and final branch-only
runs. Q02 renderer GPU p95/p99 changes 14.8372/16.6638 → 11.4896/13.5165 ms; Q01 p95 worsens
3.8685 → 4.7699 ms. All six runs retain 300/300 exact accepted-Spout captures and stable input
fingerprints. Final Q02 main/Spout accepts 300 calls each, editor 299; producers accept 290/296,
so cadence stays open. Fresh before/final logical peaks remain 144,691,264/468,172,864 bytes
for Q01/Q02. No memory reduction or physical-residency claim is made.

The unsupported-benefit SDK/storage change is removed completely; its 266 execution sources,
successful finite/native tests and regressing controls remain in the 5,824,018-byte evidence
archive. Independent reconstruction verifies all 1,800 renderer GPU partitions and 1,800 exact
accepted-Spout image joins. Final validation passes 153 ordinary Rust, 17 Golden native,
two app-native and 267 Python tests, strict Golden clippy, release build and both formatters.
Full GPU/queue, prescribed display/cadence/growth and genuine removal/stuck retirement remain
open. **M03 stays 7/9; overall stays 30%.**

## M03 DMA decoder boundary — 2026-10-30

The app-owned [offline exporter](../../tools/mapgyver/trace-packets/README.md) uses Microsoft's
locked TraceProcessor API. It filters the four manifest PIDs, preserves partial/missing values
as null, records decoder/input hashes and refuses to overwrite evidence. Its analyzer retains
every exported packet, reports missing fields and checks loss, clocks, identity and ordering.
No runtime, workload, shader, queue depth, DTO or graphics-setting change is made.

The [native experiment and limitations](mapgyver-m03-qualification-runner.md#dma-packet-decoder-boundary--2026-10-30)
retain the two unsuccessful capture controls and the completed GPU-profile capture. The latter
has zero reported final ETW loss but 16,272/16,272 owned packet timings partial, so queue and
critical-path acceptance remain unavailable. The 1.10 GiB system-wide ETL stays local; only
filtered observations and owned native artifacts are published. The profile is not added to
routine qualification. Root/Core formatting, a strict locked C# build and all 267 Python tests
pass, including eight new packet-coverage tests. **M03 stays 7/9; overall stays 30%.**

## M03 direct input bridge receive — 2026-10-29

The source worker now passes a free write-locked bridge slot directly to the pinned SDK's
receive-into API. It removes the private capture texture and capture-to-bridge copy, retaining
three slots, source/renderer completion authorities, static-image freshness, withdrawal and
resize/reconnect fences. `InputLimits::source_storage_bytes` shares one checked layout with
backend validation, source-pool admission and app fixtures: three bridge images plus one
sender alias. Source counts, pacing, marker identities and workload ceilings are unchanged.

The [packet](evidence/mapgyver-m03-direct-bridge-2026-10-29.json) and
[raw controls](evidence/mapgyver-m03-direct-bridge-controls-2026-10-29.zip) preserve new release
Q01/Q02 controls and the prior immutable baseline reference. Each uses five-second warmup/
sample and one-second interruption intervals, on the existing Intel route. Q02 main/Spout/
editor acceptance changes 300/299/271 → 300/300/296, with 300/300 exact Spout captures and no
invalid comparisons. Renderer GPU p95/p99 changes 16.9713/18.3391 → 15.5987/16.0877 ms; median
changes 13.2066 → 13.4602 ms. Q01 retains 300/300 outputs/captures and GPU p95 3.2120 ms.
Producer admission still fails; Q01 source A acceptance worsens 299 → 288/300.

Q02 baseline reservation falls 291,225,664 → 254,361,664 bytes (35.15625 MiB), while the recorded
total peak rises 505,036,864 → 575,078,464 bytes. The latter equals the new baseline plus three
106,905,600-byte generation charges; the prior peak equals its baseline plus two. This arithmetic
does not identify the unsampled transient roles or physical residency. Both peaks remain below
the unchanged 1 GiB bound, and no growth/retirement gate is closed.

Final checks: 153 ordinary Rust tests, 15 Golden native tests, two app-native channel/Picture
tests, 92 production Python tests, strict clippy, release runner and both Rust formatters pass.
Development checks caught and fixed an old admission multiplier and a missing app-fixture import.
No displayed age, critical-path, queue, genuine removal or full-series pass is inferred.
**M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 direct window backbuffer rendering — 2026-10-29

Golden Video now borrows a prepared render target from each admitted window endpoint and
draws directly into its SRGB backbuffer view. The shader emits opaque linear RGB over black;
the view encodes once. Window conversion textures and their copy disappear, while fixed
two-backbuffer ownership, readiness permits and completion-fenced endpoint retirement remain.
Spout's UNORM conversion/storage, color/alpha handling and keyed sender path are unchanged.
The latency handle is owned before fallible view creation, and foreign contexts are rejected
before drawing. Tests read all 16 backbuffer pixels across six presentations against scalar
transfer oracles, and verify actual preparation allocates only the render-pass texture.

The [packet](evidence/mapgyver-m03-direct-window-2026-10-29.json) and
[raw controls](evidence/mapgyver-m03-direct-window-controls-2026-10-29.zip) preserve new Q01/Q02
release controls and reference the immutable prior stage controls. Scene, adapter, queue,
polling and budgets stay fixed. Each uses a five-second warmup/sample and one-second
interruption intervals. Initial generation textures fall 7 → 5. Q02 logical reserved peak
falls 578,764,864 → 505,036,864 bytes (70.3125 MiB); this is not a physical-residency claim.

Q02 main/Spout/editor acceptance changes 276/294/165 → 300/299/271 of 300 opportunities.
Exact accepted Spout captures change 272/294 → 299/299, with no invalid comparisons. Whole
renderer-context GPU p95 changes 21.7279 → 16.9713 ms; output-interval p95 changes 15.2477 →
13.1157 ms while its median stays 9.5033 → 9.5441 ms. These sequential short controls do not
isolate causality or establish a full GPU critical path. The Q02 CPU-start-selected GPU
cohort has 299 complete spans because one accepted output belongs to a frame starting before
the sample boundary; all 300 anchored deadline opportunities remain. Updated Q01 retains
300/300 acceptance/capture and GPU p95 3.8944 ms. No displayed-frame pass is inferred.

Validation: 152 regular Rust tests, all 15 native tests, 92 production Python tests, strict
clippy, final release runner build and both Rust formatters pass on the final source.
Genuine removal/stuck retirement, GPU/queue, cadence/admission, growth and full series remain
open. **M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 renderer GPU stage partitions — 2026-10-29

Golden Video's optional diagnostics now prepare two intermediate timestamp queries per
renderer slot, inside its existing disjoint scope. Input-copy and render-pass boundaries
partition the exact completed batch; output conversion/delivery occupies the remaining
interval. There are still two slots and no additional frame allocation, event, flush,
admission dependency or wait. Source/fixture contexts allocate no intermediate queries.
Incomplete/out-of-order/duplicated marks cannot certify a partition, and slot reuse resets
marker state. Rust artifact schema 7 and its generated metadata advance together.

The [packet](evidence/mapgyver-m03-gpu-stages-2026-10-29.json) and
[raw control archive](evidence/mapgyver-m03-gpu-stages-controls-2026-10-29.zip) preserve new
release Q01/Q02 runs on the existing Intel route, each with five-second warmup/sample and
one-second interruption controls. All 600 sample spans have exact ordered stage joins.
Q02 input/render/output p95 are 7.2783/7.4998/15.2477 ms; these percentiles are not summed.
Outputs are the largest interval in 271/300 Q02 frames, median 9.5033 ms. The next performance
change should target output conversion/delivery. These elapsed spans include GPU scheduling,
dependency waits and command-feed gaps; they cannot attribute the interval solely to shaders.

Q01 accepts all 300 opportunities per output and independently captures 300/300 exact Spout
images. Q02 accepts 276/294/165 main/Spout/editor opportunities, with 272/294 exact captures
and no invalid comparisons. Missing captures and qualification failures remain open. No
PresentMon collector or display/driver/overlay change occurs; earlier display/deadline
evidence stays immutable. Whole renderer-context GPU p95 is 3.9429/21.7279 ms for Q01/Q02.

Validation: 152 ordinary Rust library/public tests, 13 native tests, a repeated two-test
timing check after the clippy style repair, 92 production Python tests, strict clippy, the
final release runner build and both Rust formatters. Independent raw reconstruction verifies
all 600 stage partitions and unchanged capture fingerprints. This measures an existing
failure; no resource, cadence, frame-age or recovery budget is relaxed. **M03 stays 7/9
(78%); overall stays 3/10 (30%)**.

## M03 hardware sRGB source decoding — 2026-10-29

Golden Video now prepares a decoded SRGB view for source-local straight/opaque sRGB storage,
while keeping raw views, shared input/output UNORM formats, logical byte charges and all
queue/admission limits unchanged. Encoded premultiplied sources still unpremultiply before
shader decoding. Hardware pixel oracles cover RGBA/BGRA, transparent colored texels, nearest/
linear filtering, straight/opaque/premultiplied alpha, and linear sources; the native input,
image recreation, region, recovery, marker-lineage and timing regressions pass.

The [evidence packet](evidence/mapgyver-m03-srgb-sampling-2026-10-29.json) and
[raw controls](evidence/mapgyver-m03-srgb-sampling-controls-2026-10-29.zip) retain one unchanged
Q02 baseline and updated Q01/Q02 controls on Intel LUID `00000000:00016c2a`. Each uses a
five-second warmup, five-second sample and one-second interruption controls. An isolated
same-device alternating sampler experiment reports 12 measured pairs after four warmup pairs,
four 4K draws per batch: median cost 2.0702 → 1.3827 ms per draw (33% reduction).

Q02 main acceptance changes 249/300 → 292/300, editor 156/300 → 174/300 and Spout 294/300 →
295/300; exact independent captures change 267/294 → 289/295 with no invalid comparisons.
These short sequential controls cannot attribute all end-to-end differences to this change:
renderer-context GPU p95 is 19.9860 → 20.3868 ms, so no overall GPU latency improvement is
claimed. Updated Q01 accepts and captures 300/300 on each applicable output/cohort.
PresentMon was not enabled; the previous window display/deadline evidence is preserved.

Validation: 148 regular library tests plus four public integration tests, 11 native regression
tests plus the isolated native cost control, strict Golden Video clippy, release runner build
and both Rust formatters. Native captures have stable before/after execution fingerprints;
the final source adds only the cost-test code after capture. No workload, budget or gate
denominator changes. **M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 anchored delivery deadline bounds — 2026-10-29

The [deadline checkpoint](mapgyver-m03-qualification-runner.md#anchored-delivery-deadline-bounds--2026-10-29),
[analysis packet](evidence/mapgyver-m03-delivery-deadlines-2026-10-29.json) and
[derived archive](evidence/mapgyver-m03-delivery-deadlines-analysis-2026-10-29.zip) reanalyze
the existing [schema-6 raw archive](evidence/mapgyver-m03-window-present-controls-2026-10-29.zip)
without changing or rerunning native code. Independently reconstructed nested-ceil scheduler
boundaries, exact API/display joins and canonical source-word/capture joins reproduce every
classification. Original raw files and analyses remain unchanged; the new analyzer version is
hashed separately. All non-analysis capture hashes and the native binary still match.

Each output retains **180 anchored opportunities**. Q01 main/Spout/editor each have **180 on
time, zero proven misses and zero unknowns**. Q02 main has 137 late software displays plus 43
nonaccepted frames; editor has 91 late displays plus 89 nonaccepted frames. Both therefore
miss all 180 opportunities. Q02 Spout has **three proven on time, one nonaccepted/proven miss,
176 unknowns**: 158 late completion upper bounds and eighteen missing exact captures. Its one
known miss already exceeds the unchanged 0.1% budget; unknowns are not relabeled as GPU misses.

Window observations use reported software display time. An exact completed Spout capture can
prove GPU readiness by its CPU observation time; a late poll alone cannot prove late readiness.
Late publication API start and known unsubmitted/nonaccepted opportunities supply separate
failure bounds. Missing slots/witnesses remain in the denominator, and even a late CPU API
return cannot erase an earlier valid delivery witness. Physical scanout, queue occupancy and
GPU critical path remain separate evidence. The short controls do not satisfy the prescribed
series. The five-member derived archive is 190,528 bytes, SHA-256
`930548d805d6ae35e92d255ccb079095af5a3b1cbac83220da44af081e8b9824`.

Validation passes **85 production Python tests**, including exact 1/1000 inclusivity, pre-window
API/deadline cohorts, late polling, CPU preemption, missing/foreign/duplicate/stale witnesses
and partial telemetry; both required Rust formatters run. Rust/native implementation is
unchanged, so the earlier 182 regular/twelve native test baseline is retained without claiming
a new native run. Gates 4/9 remain open. **M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 window Present/display correlation — 2026-10-29

The [window observation checkpoint](mapgyver-m03-qualification-runner.md#exact-window-presents-and-software-display-age--2026-10-29),
[schema-6 packet](evidence/mapgyver-m03-window-present-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-window-present-controls-2026-10-29.zip) retain exact endpoint
incarnations/three DXGI interface aliases and native Present API intervals. The optional pinned
PresentMon collector targets only the new renderer PID and an owned UUID session. The retained
probe supplies its public read-only session query, without running a legacy renderer. No
device/driver/display/overlay, SDK, scene, polling, queue-depth or budget setting changed.

All **1,194 Q01 and 875 Q02 CSV rows** join exactly to native calls, with no unmatched rows.
The sample main/editor cohorts are 180/180 and 180/180 (Q01), 137/137 and 91/91 (Q02), with no
missing/dropped/display/source/plan observations. Independent reconstruction reproduces every
raw join and source/publication/display-age distribution. Q01 main source-age p95 A/B is
32.185/28.376 ms; editor is 32.214/28.998 ms. Q02 main is 45.796/55.334 ms; editor is
52.630/63.386 ms, exceeding the unchanged 50 ms target. These are software display events,
not window pixel/physical scanout or optical observations. Pending-display backlog is neither
native buffer occupancy nor a GPU critical path, and its incomplete pre-window coverage stays open.

Q01 has 180/180/180 main/Spout/editor acceptances and 180 exact Spout captures. Q02 accepts
137/179/91, with 161 exact Spout images and 18 missing. Overall Q02 age remains unavailable
because its Spout cohort is incomplete; the unfavorable observed window distributions remain
explicit. All 262 execution hashes per capture and both external tool hashes match, telemetry
is lossless, live ETW counters cover the native calls, and no collector loss warning is reported.
Final collector-shutdown loss statistics remain unavailable. Retirement is actual/unforced,
with zero final resource bytes/in-flight work, in 0.095/0.124 s.

The initial trace finalized native work but failed decoding redirected UTF-16 output. Its raw
files and failure are preserved with unavailable input/live-query evidence. The fixed decoder
retains original bytes and explicit encoding, and the runner persists admission fingerprints/
native completion even if ancillary trace finalization fails. One fresh bounded pair verifies
the correction; no earlier run is silently promoted or overwritten.

Validation passes **182 regular Rust tests, twelve native GPU tests and 73 production Python
tests**, strict default/native all-target Clippy, generated fixture contracts, locked release
x64 build and both required formatters. Prescribed repetitions/durations, GPU critical path,
native buffer/GPU queue/deadline measurements, growth/handles and genuine removal/stuck
retirement remain open. **M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 CPU plan lifecycle and first-use age — 2026-10-29

The [runtime observation checkpoint](mapgyver-m03-qualification-runner.md#cpu-plan-publication-receipt-and-first-use--2026-10-29),
[schema-5 packet](evidence/mapgyver-m03-plan-age-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-plan-age-controls-2026-10-29.zip) record admitted publication,
first worker receipt, atomic installation and terminal supersession/rejection/cancellation.
Golden's bounded recorder is allocated before worker startup. Receipt is recorded once across
preparation retries; reconstruction records a new resource/device installation with the same
authored identity. CPU metadata alone crosses these boundaries. GPU queue, physical presentation
and critical-path measurements remain separate. Schemas 1–4 retain their original analyses.

Each bounded Q01/Q02 control has nine published plans, eight first receipts, six installed/used
generations and three superseded inputs, including one unread input. All 389/385 native frame
rows and every accepted sample output join their exact lifecycle/resource identity. Independent
raw reconstruction reproduces every reported handoff/first-use/per-output age. Median
publication-to-receipt is 0.0161/0.0157 ms; the startup observations reach 76.504/80.916 ms.
Publication-to-first-use maxima are 82.708/97.093 ms across six observations, including startup.
These finite distributions establish measurement coverage, not a new latency budget or a
steady-state edit-latency pass. Retained plan age grows naturally while the same plan is used.

Q01 accepts 180/180/180 main/Spout/editor calls and captures all 180 exactly. Q02 accepts
158/174/108; 153 Spout images match exactly and 21 remain uncaptured, with no observed pixel
disagreement. Its observed source-A age p95 is 56.572 ms. Minimized/disconnected Q02 phases
still have incomplete capture/source joins and cadence failures. No device/driver/display/
overlay, SDK, scene, polling, queue-depth or budget change is included. All 259 execution
fingerprints per capture match; telemetry is lossless; cleanup is actual/unforced with zero
final bytes/in-flight work, retiring in 0.094/0.114 s.

Validation passes **182 regular Rust tests, twelve native GPU tests, 60 production Python
tests**, strict default/native all-target Clippy, generated fixture contracts, release x64
build and both required formatters. Existing complete GPU/output qualification, growth/handle
series and genuine device-removal/stuck retirement remain open. **M03 stays 7/9 (78%); overall
stays 3/10 (30%)**. No denominator, budget or historical failure is changed.

## M03 GPU keyed Spout handoff — 2026-10-29

The [ownership checkpoint](mapgyver-m03-qualification-runner.md#gpu-keyed-spout-image-handoff--2026-10-29),
[schema-4 packet](evidence/mapgyver-m03-keyed-handoff-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-keyed-handoff-controls-2026-10-29.zip) retain the isolated
failure, SDK fix and one bounded release Q01/Q02 control pair. GPU taps show correct renderer
input/conversion pixels and an incorrect outgoing image. Owned Spout senders now request GPU
keyed shared textures; detailed sends use zero-timeout CPU metadata then GPU ownership. Busy
copies no shared image and advances no counter. Unchanged hints need only CPU metadata access.
No device, driver, monitor or overlay setting changed. Scenes and budgets are unchanged.

The native regression checks direct SDK, source bridge, full runtime with/without GPU taps and
held-GPU Busy/resume: **518 available-image comparisons, zero mismatches**. One tapped startup
capture has no available source and is explicitly excluded. Test taps/readback are absent from
production builds. Legacy named/unversioned input tests continue to pass. SDK baseline and all
nine patched file hashes are verified; no fallback to CPU-only output ownership is added.

Q01 has **180/180 exact sample captures**, plus 60/60 in each short editor interruption. Q02
has **148/176 exact sample captures**, no observed pixel mismatch and 28 uncaptured images.
Its minimized/disconnected phases have 46/58 and 36/61 exact captures; the latter also has 16
missing-source captures. Q02 main/Spout/editor API acceptance is 138/176/100 of 180; source-age
p95 A/B is 58.565/48.107 ms on the observed sample subset. Those failures retain gate 9.
Q01 source admission also rejects four attempts per source; output coherence is not a producer
cadence pass. All 255 execution fingerprints match per capture, telemetry is lossless, and
retirement is actual/unforced with zero final resource bytes/in-flight work (0.095/0.120 s).

Validation passes **179 regular Rust tests, twelve explicit native GPU tests, 49 Python tests**,
strict default/native all-target Clippy, SDK all-feature/all-target and legacy desktop probe
checks, release x64 build and both required formatters. Independent raw reconstruction agrees
for every sample/interruption cohort. Full plan/queue/presentation/critical-path measurements,
prescribed series, growth/handles and genuine removal/stuck retirement remain open. **M03 stays
7/9 (78%); overall stays 3/10 (30%)**. The denominator and historical failures are unchanged.

## M03 copied-image lineage and source-retention age — 2026-10-29

The [checkpoint](mapgyver-m03-qualification-runner.md#exact-spout-output-cohorts-and-source-retention-age--2026-10-29),
[schema-4 packet](evidence/mapgyver-m03-output-cohort-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-output-cohort-controls-2026-10-29.zip) retain diagnostic
SDK copy versions through source publication, renderer delivery and independent capture.
Producer GPU words independently check each claimed renderer source image. SDK share-handle
identity uses its documented 32-bit field, accepting only zero/sign-extended representations;
actual native access handles are unchanged. Unknown foreign sources claim no exact version.

One short Intel release Q01/Q02 pair exposes unresolved dynamic lineage. Q01 captures all 180
sample-accepted Spout versions, but 141 disagree with the exact renderer source-use labels;
39 match both cards. Q02 captures 144 of 180: 138 disagree, six match and 36 are uncaptured.
All observed sample words still match legitimate producer records, demonstrating why the old
producer-to-capture age checks cannot establish renderer-output lineage. Independent raw
recomputation agrees for the sample and both interruptions. In one Q01 example, renderer batch
127 labels producer frames 231/223 while its matched capture contains 229/222. Earlier renderer
predictions often match; input-copy versus output-copy origin remains undetermined.

Source-publication-to-output-API clocks join every accepted sample call on all three outputs.
Spout p95 A/B retention is 18.435/7.599 ms (Q01) and 22.881/13.928 ms (Q02). These CPU metadata
ages do not prove delivery of the labeled pixels or establish GPU queue time. Sample main/
Spout/editor acceptance is 180/180/180 and 147/180/135. Minimized exact matches/accepted Spout
images are 60/60 and 3/55; disconnected matches are 8/53 and 2/60. Both overall interruption
gates remain open, including their required 60-second durations. Q02 cadence/producer/age
failures remain. Age distributions of the 39/six exact sample matches are incomplete subsets.

All 248 execution fingerprints match before/after capture and current source; all roles have
lossless telemetry and valid sample GPU rows. Both runtimes stop with zero resource bytes and
in-flight work; all owned processes exit unforced. Actual retirement takes 0.096/0.109 s.
Validation passes 179 regular Rust tests, eleven explicit native GPU regressions and 49
production Python tests, strict default/native all-target Clippy, locked release x64 build and
both required Rust formatters. The SDK fork, scenes, budgets, queue depth, polling and display/
driver/overlay settings are unchanged. M03 remains **7/9**, overall **3/10 (30%)**. The next
implementation isolates the pixel/version copy boundary before additional qualification runs.

## M03 resource reuse and preparation attribution — 2026-10-29

The [checkpoint](mapgyver-m03-qualification-runner.md#native-resource-reuse-and-preparation-attribution--2026-10-29),
[schema-3 packet](evidence/mapgyver-m03-resource-reuse-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-resource-reuse-controls-2026-10-29.zip) retain device-local
weak texture/view reuse and exact resource-generation preparation/first-use joins. Mutable
storage keeps immediate-context ordering and fresh per-generation source hints; immutable
images pin exact payload leases, deduplicating logical inputs without confusing equal asset
IDs or recycled addresses. Expired weak entries retain no allocation or asset. Per-generation
admission remains conservative. Old schema-1/2 captures retain their original analyses.

Both short Intel release controls create seven textures, then reuse all seven in each of seven
later preparations. Later preparation takes at most 0.0109/0.0106 ms; initial allocation takes
6.587/40.588 ms. First-submit wait reaches 12.818/22.914 ms. Independent raw recomputation joins
all 388/385 renderer submissions and verifies all 247 execution fingerprints. This measures
native preparation/first use, with full publication/receipt/output-queue age still unavailable.

Q01 sample outputs accept 180/180/180; Q02 accepts 155/180/124 (main/Spout/editor). Q01 short
interruptions meet observed cadence/age checks; Q02 minimized main/Spout accepts 56/59 of 60
and source-age p95 reaches 193.446 ms despite no new allocation. Sample captured source-age
p95 A/B is 33.612/52.810 and 73.588/84.199 ms. Q02 producer shortfalls remain. These observations
establish reuse, not a causal timing improvement or prescribed qualification. All roles retain
lossless telemetry, all sample GPU rows/markers are valid, and cleanup is unforced with zero
final resource bytes/in-flight work; actual retirement takes 0.099/0.115 s.

Validation passes 177 regular Rust tests, eleven explicit native GPU regressions and 36
production Python tests, strict default/native all-target Clippy, locked release x64 build and
both required Rust formatters. New native checks cover outstanding-generation reuse, descriptor/
payload changes, pixels, deduplication and actual asset release. Source tests now wait for real
read leases and isolate uncontended preflight before retaining actual Busy coverage. No display,
driver, overlay, polling, queue, scene or budget changed. M03 remains **7/9**, overall **3/10
(30%)**; gates 4 and 9 retain their complete recovery/qualification scope.

## M03 source copy and freshness — 2026-10-29

The [source checkpoint](mapgyver-m03-qualification-runner.md#source-copy-and-freshness-boundary--2026-10-29),
[packet](evidence/mapgyver-m03-source-freshness-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-source-freshness-controls-2026-10-29.zip) retain exact copied,
unchanged, busy and handshake outcomes at the public pinned SDK boundary. Per-instance counters
make no registry writes. Controlled peers explicitly opt into versioning; ordinary external
sources remain unversioned, including when an old positive counter exists. CPU preflight uses
the actual sender lock before skipping GPU work. Busy preserves the last valid publication;
session replacement and lost publication re-arm copied history after actual completion.
The [vendor provenance](../../vendor/spout2-sys-0.1.1/GOLDEN_PATCH.md) retains all source hashes
and licenses; source/header edits now invalidate the static archive and capture fingerprints.

The native regression verifies unchanged static-source polls without GPU batches, frozen-counter
foreign copies, actual named-mutex Busy after a successful copy, demand 0→1/missing-mailbox
recovery and same-name resize/reconnect. Counted output names have a shared 239-byte admission
limit; unversioned inputs retain 255 bytes. Keyed synchronization is compiled and source-reviewed,
without new hardware coverage.

One short Intel release Q01/Q02 pair completes with all three outputs accepting 180/180 sample
submissions and no missing software slots. Source GPU batches are 180/180 and 169/173, matching
admitted producer frames, versus the preceding controls' 1,156/1,231 and 306/482. Consumer batches
are 180/157. Renderer-context GPU p95/p99 is 3.809/4.378 and 9.471/13.112 ms; captured source-age
p95 A/B is 22.988/9.803 and 66.250/67.934 ms. These are unpinned observations, not a causal timing
claim or a complete output-frame age cohort. Full captures preserve all distributions and failures.

Interruption acceptance remains insufficient: minimized main/Spout counts are 45/46 and 28/44
of 60 expected opportunities, with observed age reaching 279.459/308.800 ms at p95. Q02's
disconnected main/Spout counts are 50/60. Both independence flags remain false; the one-second
interruptions also remain shorter than the required 60 seconds. Q02 producer shortfalls and GPU
outliers remain. All four roles have complete, lossless traces; all 244 fingerprints match;
actual retirement is 0.100/0.117 s with clean, unforced owned cleanup and zero final resource bytes.

Validation passes 177 regular Rust tests, ten explicit native GPU regressions, 32 production
Python tests, strict default/native all-target Clippy, desktop probe all-target compilation,
all-feature SDK compilation, the locked release x64 build and both required Rust formatters.
The fixed scenes, budgets, polling, queue depth and display/driver/overlay settings are unchanged.
M03 remains **7/9**, overall **3/10 (30%)**; genuine device removal/stuck retirement (gate 4)
and complete renderer qualification (gate 9) remain required.

## M03 output admission before conversion — 2026-10-29

The [output-admission checkpoint](mapgyver-m03-qualification-runner.md#output-admission-before-conversion--2026-10-29)
moves zero-timeout window readiness and availability ahead of per-output conversion. An exclusive
admitted-output borrow spans conversion through delivery; busy/hidden/recovering endpoints skip
that work while healthy outputs continue. Late Present Busy retains its readiness permit and
completion tracking covers the preceding backbuffer copy. Buffering, maximum latency, source
polling and all budgets are unchanged. Schema-2 rejected rows have no conversion/API interval;
successful preflight is retained in inclusive frame wall time without separate attribution.

Independent review found no material issue. The owned kernel-event test exercises real signal
consumption, hidden/resume behavior and late-Busy retention; it does not substitute for native
GPU trace evidence. Source copied/freshness status remains the next checkpoint at the public
binding, without registry changes or freezing unknown foreign senders on an untrusted counter.
No SDK/source-publication change is included. M03 stays **7/9**, overall **3/10 (30%)**.

The [packet](evidence/mapgyver-m03-output-admission-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-output-admission-controls-2026-10-29.zip) retain one short
Intel release Q01/Q02 pair. Every busy/unavailable row skips conversion/API: 68/223 rows across
the respective whole captures. Minimized editor rows skip all 60/47 conversions. Independent
raw recomputation verifies ordered brackets for all 1,093/878 accepted rows. Both sample main
and Spout outputs accept 180/180 submissions; editor acceptance is 180/119. Telemetry loss is
zero in all four roles, all 180 sample GPU spans are valid, fingerprints are stable, and cleanup
is actual, unforced and error-free. No late API Busy occurred in these native controls.

Renderer-context GPU p95/p99 is 7.533/10.285 ms (Q01), 18.470/21.025 ms (Q02); captured source-age
p95 A/B is 35.679/34.025 ms and 216.710/146.563 ms respectively. Capture age still lacks the full
output cohort. Independence does not pass: minimized main/Spout acceptance is 57/60 and 27/47
of 60 expected opportunities; Q02 disconnected acceptance is 30/56 of 60. The one-second
interruptions are shorter than the prescribed 60 seconds. Main output progress is functional
evidence, not a cadence pass. These unpinned diagnostics establish avoided work without a causal
performance claim. No device-removal, retirement-bound or qualification gate closes.

Validation passes 172 regular Rust tests, nine explicit native GPU regressions, 31 production
analyzer/deadline tests, strict default/native all-target Clippy, the locked release x64 build
and both required Rust formatters. Native finite pixel and app source/history/recovery checks
remain green. No display, driver, overlay, queue-depth or source-polling setting changed.

## M03 measurement integrity and output conversion — 2026-10-29

The [measurement checkpoint](mapgyver-m03-qualification-runner.md#measurement-integrity-and-equal-extent-conversion--2026-10-29)
and [schema-2 evidence](evidence/mapgyver-m03-measurement-integrity-2026-10-29.json) retain two new
short release controls. Bounded concurrent recording removes the former metadata-mutex loss;
all four processes have exact written/drained/raw counts and no full/clock losses. Exact QPC
mapping, per-opportunity admission and final coverage fences establish all 180 software slots
and their output joins in each sample. Replaced clocks become explicitly unmapped. Disabled
diagnostics retain the existing clock and create no cadence UUIDs. API deadlines remain
separate from physical output cadence; unverified criteria never become passes.

Equal-extent output conversion now uses the existing single-load path. Native finite pixel
oracles preserve color/alpha and scaled filtering. Q02 observes 172/180 native-window and
180/180 Spout acceptances, compared with 75/180 and 139/180 earlier. Q01 records 177/180 and
180/180. Renderer GPU p95 is 16.906 / 19.640 ms, still above the diagnostic target; both complete
180 valid context-local spans and 360 source joins. Q02's sparse capture observations have
high age; repeated/source captures do not prove every output frame's age. No sustained or
causal performance pass is claimed. Both controls exit cleanly with actual fenced retirement.

Validation passes 204 regular Rust tests, nine explicit native GPU regressions, 198 Python
tests, strict default/native Clippy, Chataigne all-target compilation, release x64 build and
required root/Core formatting. Touched source files remain under 1,000 lines. The archive retains
the exact inputs, statuses and unavailable observations. M03 stays **7/9**, overall **30%**;
source freshness/output backpressure and the remaining measurement/recovery gates are next.

## M03 production qualification runner — 2026-10-29

The [runner boundaries and reproduction](mapgyver-m03-qualification-runner.md) and
[evidence packet](evidence/mapgyver-m03-qualification-runner-2026-10-29.json) retain five native
captures in a SHA-256-identified [raw archive](evidence/mapgyver-m03-qualification-controls-2026-10-29.zip).
Independent GPU producers/consumer exercise the production Golden host/runtime, two held previews,
bounded opt-in CPU/GPU/source metadata, real LOCAL DXGI admission and sparse process observations.
Rust-generated fixture contracts, GPU marker checks, owned-process watchdog and offline analysis
preserve missing/invalid/lost observations. Normative GPU critical-path and output-frame age remain
unavailable; API/consumer-capture observations are not relabeled as those measurements.

Corrected controls use one-second warmup, three-second sample and one-second minimized/disconnected
intervals, on Intel adapter 0. Q01 accepts 180/180 main window and Spout submissions. Q02 accepts
75/180 main-window and 139/180 Spout submissions; its renderer-context GPU p95 is 25.268 ms.
Both achieve the requested output client/backing and 640×360 / 480×270 preview registrations,
advance main outputs through the short editor interruptions, retain both HWNDs through actual
retirement and exit all owned processes without force. Final native retirement takes 0.101 / 0.139 s.
Renderer telemetry drops 286 / 17 contended rows respectively; complete coverage is not established.

The three failed controls remain retained. Fixes select the known loopback fixture URL instead of
the WebView's recorded `about:blank`, settle the physical client after the 125%→100% monitor move,
and observe actual HWND destruction after Tauri's asynchronous destroy request. No display/driver/
overlay setting changed. Screenshot approval timed out; no visual/physical qualification is inferred.

Validation: 164 regular Rust tests, nine explicit native GPU regressions, 185 Python tests,
22 UI unit tests, Svelte check/lint and browser fixture/recovery checks pass. Strict all-target
native Clippy, locked release x64 build, both required formatters and whitespace checks pass.
M03 stays **7/9**, overall **3/10 (30%)**. No prescribed three-sample series was run while measurement
coverage and observed performance remain insufficient. M05/M09 criteria retain their owners;
renderer handle/growth/drain and genuine removal/recovery remain required under M03.

## M03 admitted startup verification — 2026-10-29

- [Admitted startup retries](mapgyver-m03-render-foundations.md#admitted-startup-retries--2026-10-29)
  and [artifact packet](evidence/mapgyver-m03-startup-retry-2026-10-29.json) resolve initial allocation
  retry within gate 4. Admission precedes native work; pending core operations cannot query GPU
  completion or fabricate loss. Retry retains the original baseline and first device identity.
- Time/project controls and one latest desired plan remain live while startup waits. Cancellation
  wakes backoff; terminal startup cannot park before wake-notifier installation. Current native
  readiness and first-success history are explicit, separate observations.
- **147 regular tests** and **eight explicit native tests** pass. Real missing-LUID factory
  attempts hold a 320-byte reservation, then recover on attempt 4 into device generation 1,
  install the pending plan and submit accepted native output. Both shared-host smokes, strict
  Clippy, both-app compilation, generated drift and required formatters pass.
- **M03 stays 7/9 = 78%, overall 3/10 = 30%**. No new whole gate closes. Genuine native removal
  and bounded stuck-source retirement remain gate 4; the production Q01/Q02 qualification runner
  is next under gate 9. No budgets or denominators change and no M01 investigation restarts.

## M03 endpoint recovery checkpoint — 2026-10-29

- [Endpoint recovery](mapgyver-m03-render-foundations.md#independent-native-endpoint-recovery--2026-10-29)
  and [artifact packet](evidence/mapgyver-m03-endpoint-recovery-2026-10-29.json) record partial gate 4.
  Fixed output slots retry independently at 250 ms–2 s, retaining failed native objects until
  real completion and destroying them before replacement. Busy/occluded endpoints stay allocated.
- Sources withdraw publication/old guards and report Recovering before a possibly stuck drain.
  Cancellation and renderer acknowledgement retain GPU ownership; they do not certify retirement.
  Runtime Time/Backend faults resolve separately; close fences late recovery and exposes Stopping.
- **138 regular tests** and **seven explicit native tests** pass. The owned Spout collision test
  proves initial output failure isolation, healthy-path continuation and exact-name recovery on
  attempt 3 with the same renderer device. Source/image/region/app regressions and both real
  shared-host smokes pass. Strict Clippy, both-app compilation, generated drift and formatting pass.
- **M03 remains 7/9 = 78%, overall 3/10 = 30%**. No new acceptance item closes. Initial device
  allocation retry is next; genuine native removal and bounded stuck-source recovery/retirement
  remain required for gate 4, followed by unchanged gate-9 renderer qualification. No M01 or
  display/NVIDIA overlay investigation was reopened.

## M03 preview contract verification — 2026-10-29

- [Preview registration and geometry](mapgyver-m03-render-foundations.md#native-preview-registration-and-geometry--2026-10-29)
  and [artifact packet](evidence/mapgyver-m03-preview-2026-10-29.json) close gate 8. Rust generates
  the local protocol; Golden owns bounded leased metadata, region crop/clear and Svelte bindings.
  The shared host supplies actual WebView/window metrics through its optional bounded callback.
- The real Svelte/native consumer passes initial/clipped/hidden/unregistered/empty/disposed
  phases, 103 submitted frames and 206 accepted output calls on Intel. It requires the window
  output's own accepted token before uncovering a current installed generation. Independent
  output and retirement before HWND release pass; this is not scanout or physical alignment.
- **171 regular Rust tests** and **22 preview UI tests** pass. All **six explicit native tests**
  pass, including eight finite 4×4 region grids with unchanged 2/255 tolerance, crop/removal/clear,
  active visibility withdrawal without reallocation, and existing image/Spout/app regressions.
  The host window-only/Spout-only/resized regression also passes.
- Built-fixture browser regression covers real DOM callback ordering, covers, clipping/removal
  and browser-only editing. Svelte checks report zero errors/warnings; both UI lints, strict
  default/native Clippy, both-app compilation, generated drift and root/Core formatting pass.
  The first native timeout, DOM acknowledgement race and finite-readback helper correction are
  retained in the packet; no tolerance or qualification budget was relaxed.
- **M03 7/9 = 78%, overall 3/10 = 30%**. Gates 4/9 remain open. Production recovery and bounded
  retirement are next; specialized editors/full physical/DPI/input remain M05–M07. No new M01
  investigation or display/NVIDIA overlay changes.

## M03 Picture/asset verification — 2026-10-29

- [App assets and Picture resources](mapgyver-m03-render-foundations.md#app-assets-and-picture-resources--2026-10-29)
  and [native packet](evidence/mapgyver-m03-picture-assets-2026-10-29.json) close gate 7. App catalog
  nodes reuse persistent node UUIDs, persist locators only and use the existing atomic operation,
  history, sparse/full codec and QuickJS routes. Capture reads demanded leaves only, with no IO.
- PNG/JPEG worker decoding, explicit SDR/profile/orientation policy, per-source missing retry,
  targeted file-change retry, relocation and healthy-source preservation pass. Image leases and
  automatic fences span CPU preparation, final native installation and resource reconstruction.
- **128 regular tests pass**: 30 app, 16 Assets, 79 Video unit and 3 public Video. All **five**
  explicit native tests pass on Intel LUID `00000000:00016c2a`, including actual app PNG upload,
  shared channels, relink/reload/fresh runtime and finite Golden alpha-filter/recreation pixels.
  Upload counters stay fixed across subsequent frames; images consume zero Spout receiver slots.
- Rebuilt host smoke, strict default/native all-target Clippy, both-app default checks, generated
  contracts, UI check/lint, root/Core formatting and whitespace checks pass. No native performance,
  hardware-removal, physical/DPI or complete Media Manager qualification is inferred.
- **M03 6/9 = 67%, overall 3/10 = 30%**. No denominator/budget changes. Gates 4/8/9 remain open;
  dedicated manager controls, commands/callbacks/snippets and ready-file watching remain M04.

## M03 shared asset checkpoint — 2026-10-29

- [Shared asset foundation](mapgyver-m03-render-foundations.md#shared-asset-reference-and-preparation-foundation--2026-10-29)
  implements persistent references/validated locators, pure capture/relocation/relink, guarded
  immutable worker preparation and lifetime admission through every prepared consumer.
- **16 Assets tests** verify contracts, missing/replaced sources, relocation/relink, cancellation,
  source/codec/item/byte bounds and off-thread final disposal. The independent public Video test
  installs a prepared synthetic extent and rejects a stale scheduled replacement while keeping
  its live generation. The complete regular app/Assets/native-feature Video suite passes **109**.
- Strict default/native-feature Clippy, both-app default checks, root/Core formatting and
  whitespace checks pass. Native/UI implementations and qualification budgets are unchanged;
  no new hardware or Picture decode/upload qualification is inferred.
- Gate 7 remains partial pending app codec/history/Picture and GPU resource integration. M03 is
  still **5/9 = 56%**, overall **3/10 = 30%**, with unchanged scope/counts. Shared P15 plan evidence
  is updated; Chataigne's audio migration, collect/relink and scheduled-media acceptance stay open.

## M03 channel bridge verification — 2026-10-29

- [App channel publication bridge](mapgyver-m03-render-foundations.md#app-channel-publication-bridge--2026-10-29)
  and [small evidence packet](evidence/mapgyver-m03-channel-bridge-2026-10-29.json) record demanded
  leaf capture, history-retained-node invalidation, source-only warmup and public guarded submission.
  Preparation runs outside the actor/UI; per-frame counters do not trigger plan publication.
- **92 regular tests pass:** 17 app, 73 Video unit and 2 public Video. The explicit app hardware
  test verifies independent 2×2/3×2 channels, selector change, 4×3 resize, reconnect, undo, codec
  reload and controlled full runtime/device recreation on Intel LUID `00000000:00016c2a`.
  Ready assertions require latest-request installation and a submitted frame of that generation.
  Old project/device/layout observations are rejected; persistent references remain unchanged.
- Both explicit Golden hardware tests and the rebuilt shared-host smoke pass. Those finite
  Golden pixel assertions remain separate from the app bridge's metadata/installation assertions.
  Genuine hardware device removal, sustained performance, physical output and DPI are unqualified.
- Strict default/native all-target app/Video Clippy, both-app default checks, UI check/lint,
  generated-contract drift, root/Core formatters and whitespace checks pass. Lockfile changes
  add only two existing dependency edges. No display/overlay setting or qualification budget changes.
- Gate 5 closes: **M03 5/9 = 56%, overall 3/10 = 30%**. The denominator stays nine.
  Gates 4/7/8/9 remain open. Next is shared persistent assets/preparation; full Media Manager
  controls/commands/templates and production Screen/preview consumers retain M04/M05 ownership.

## M03 plan-owned subscription checkpoint — 2026-10-29

- Entered from published `a307eeac` on a clean `mapgyver` checkout. Repairing the native
  startup-only binding restriction was the next prerequisite for live app channel publication.
  Subscriptions now travel inside revisioned plans; startup configuration reserves explicit
  worker/storage capacity across all overlapping prepared/live/retiring sources.
- [Subscription evidence](mapgyver-m03-render-foundations.md#plan-owned-source-subscriptions--2026-10-29)
  and its [artifact packet](evidence/mapgyver-m03-plan-sources-2026-10-29.json) cover fresh
  sender replacement, conflicting-ID rejection, independent worker/storage ceilings,
  asynchronous capacity reuse and preserved live pixels/device identity. The original
  receive/resize/reconnect cases and shared-host output smoke also pass. Public source-pressure
  rejection preserves `Running` state, the live generation and continued frame submissions.
- **82 regular tests + 2 explicit hardware tests pass**. Strict native app/Video/smoke and
  default Video Clippy, both default app checks, root/Core formatting and whitespace pass.
  A transient test-only nonblocking readback failure is preserved; its bounded readiness
  handling uses the original five-second deadline and unchanged numerical pixel tolerances.
- M03 remains **4/9 = 44%**, overall **3/10 = 30%**, with no denominator or budget change.
  The app-owned demand/publication/observation bridge and resource-recreation proof are next;
  the native subscription prerequisite does not itself close gate 5. No M01 profiling,
  display/overlay changes or native performance qualification was performed.

## M03 media/channel authoring checkpoint — 2026-10-29

- Entered from published `44e3ecd9` on a clean `mapgyver` checkout. The app now registers
  separate media/channel nodes using persistent typed UUID references. Backend operations own
  initial channels, labels, source settings, size policy and validation; creation inserts one
  detached subtree and leaf changes use Golden's authored edit/history path.
- [Authoring evidence](mapgyver-m03-render-foundations.md#mediachannel-authoring-contracts--2026-10-29)
  records real project roundtrip, rename/reorder/remove/undo/redo, missing/wrong-owner rejection,
  per-channel settings, backend size resolution and production-actor save coverage. QuickJS
  uses the same app operation and receives the committed identity with one undo step.
- Rust emits the app intent/result/size contracts and canonical Golden Video descriptor/filter
  declarations; a byte-comparison drift test passes. The app UI's adapter uses its session
  interface and generated intents, with no frontend identity/default/domain behavior.
- **11 app tests + 70 Video tests pass** (68 unit + 2 public in Video). The two native hardware
  tests remain explicitly ignored in this data-contract run; their prior hardware evidence is
  retained. Strict native-feature app/Video all-target Clippy, strict optional smoke-target
  Clippy, both default app compile checks, UI check/lint and root/Core formatting pass.
- M03 remains **4/9 = 44%**, overall **3/10 = 30%**. Gate 5 is partial: live renderer binding,
  fenced source observations and app identity through actual resource recreation are still
  required. Picture/assets, dedicated controls/command nodes/script templates, Composition and
  Matrix retain their planned owners. No new native performance/display pass is claimed.

## M03 bounded input verification — 2026-10-29

- Entered from published `03c71423` on a clean `mapgyver` checkout. Golden Video now owns
  independent demand-driven Spout receiver workers on the explicitly selected adapter LUID.
  Prepared/live/retiring plans retain demand. Each source has a capture texture, three bounded
  GPU bridge slots and independent completion; renderer leases survive through matching GPU
  completion. No pixels or native handles enter the engine/control protocol.
- [Input evidence](mapgyver-m03-render-foundations.md#bounded-spout-inputs--2026-10-29) and its
  [exact-artifact packet](evidence/mapgyver-m03-spout-inputs-2026-10-29.json) retain two owned
  independent producer processes, public-runtime reception, six finite composition observations,
  prepared-demand cancellation, full-bridge backpressure, static-frame reuse, reconnect, admitted
  resize, oversized-source rejection and continued healthy-source progress. Actual extent is
  observed separately from the configured maximum; startup bindings remain immutable.
- Native-feature suites pass **161 regular tests**: IO 58 unit, Time 32 unit + 1 public, Video
  68 unit + 2 public. Both separately selected hardware tests pass on Intel LUID
  `00000000:00016c2a`, including the nine existing color/alpha/filter/blend cases. The current
  shared-host smoke again passes window-only, Spout-only and combined resized-client phases,
  with stopped/zero-reservation observations and retirement before HWND destruction.
- Default Video tests pass **62 unit + 2 public**. Strict default/native all-target Clippy,
  strict source-fixture/host-smoke Clippy, both default app checks and root/Core formatters pass.
  No dependency versions, generated protocol contracts, main app or frontend changed.
- This closes fixed native execution **gate 3**: M03 is **4/9 = 44%**, overall **3/10 = 30%**.
  The denominator and all qualification budgets are unchanged. Gate 4 device-loss/full recovery,
  gate 5 app contracts/dynamic binding publication, gate 7 assets, gate 8 previews and gate 9
  native measurements remain open. SDK receive/send acceptance is not proof of fresh delivery;
  its possible 67-ms mutex wait and transient foreign-alias import remain documented limits.
  The main app is still the workbench. Next is gate 5's app-owned media/channel contracts.

## M03 native checkpoint — 2026-10-29

- Entered this batch from published `db9a1e06`, independently verified equal to the remote
  with a clean checkout. The optional Windows D3D11 backend now runs the real compiler/worker/
  core path, four-mode composition/conversion, persistent fixed window and Spout outputs.
- Baseline admission precedes output-pool allocation; generation charges include scratch and
  converted output textures. Submission claims and preallocated query guards retain ownership
  on unwind. New baseline admission/drop/reset and submission-unwind regressions pass.
- [Native evidence](mapgyver-m03-render-foundations.md#native-d3d11-execution-and-owned-host-consumer--2026-10-29)
  and its [exact-artifact packet](evidence/mapgyver-m03-native-smoke-2026-10-29.json) retain nine
  finite pixel cases at `2/255` tolerance and the public shared-host smoke on Intel LUID
  `00000000:00016c2a`. Separate output phases and the combined shared-producer plan pass after
  client resize; stopped observations show zero reserved bytes and native resources retired
  before HWND destruction. API acceptance is not proof of delivery/scanout.
- Native-feature suites pass **158 tests**, with the one hardware test then run explicitly
  and passed. Default Video tests pass **62 unit + 2 public**. Strict default/native and smoke
  Clippy, both default app compile checks, root/Core formatters and whitespace checks pass.
- Gate 3 remains **partial**, M03 remains **3/9 = 33%**, overall **3/10 = 30%**. Bounded Spout
  receiver workers are next. Fixed DXGI stretch, descriptor-only native storage charges and
  the SDK's possible 67-ms mutex wait/true-on-skip remain explicit limitations. No physical
  output/DPI, native device-loss or sustained Q01/Q02 budget pass is inferred. Existing
  failures and qualification ownership remain unchanged; the main app is still the workbench.

## M03 verification — 2026-10-28

### Worker/null runtime and shared time

- `golden_time` owns monotonic/show/sample coordinates, checked rational conversion, immutable
  mappings and authority/discontinuity epochs. Golden Video consumes it for absolute frame
  cadence and activation; multiple readers/outputs do not advance show effects independently.
- Golden IO's payload-free wake/checkpoint wait supports renderer deadlines without mailbox
  polling or lost service-to-wait notifications. One preparation worker feeds the latest desired
  plan; reliable terminal outcomes retain bounded admission credits until drained.
- Worker-local resources stay charged through preparation, live use and retirement. Atomic
  install preserves the old scene on rejection, stale project/revision/device/time work is fenced,
  and resource destruction waits for completion or confirmed device loss. Close/drop never joins
  the actor/UI thread; a timeout does not certify native-window retirement.
- `cargo test --locked -p golden_video -p golden_time -p golden_io`: **58 video unit + 2 public,
  32 time unit + 1 public, and 58 IO unit tests passed**. Strict all-target Clippy passes for all
  three crates. Both `MapGyver` and `Chataigne2` pass `cargo check --locked` with the additive IO
  API. Root default selection remains Chataigne; no dependency versions are upgraded.
  The final 151-test run includes clock/mapping publication, old-frame status and close-before-
  activation regressions, with no ignored tests. Root and Golden Core formatting passes.
- Gates 2 and 6 close, bringing M03 to **3/9 = 33%**; overall remains **3/10 = 30%**.
  The [detailed evidence](mapgyver-m03-render-foundations.md) distinguishes simulated null
  counters/device loss from native measurements. Six gates remain open, including native
  backend/recovery/qualification, app channel contracts, assets and preview geometry. Shared
  Chataigne P14 audio/DMX acceptance stays open; P15 assets are not implemented by this batch.

### Immutable plan compiler checkpoint

- Defined nine fixed complete-scope gates before source work, replacing the provisional
  whole-phase item. All started open; the scope/native budgets are unchanged.
- Added the standalone `golden_video` workspace crate and [render/resource compiler](mapgyver-m03-render-foundations.md).
  Typed metadata and immutable operations validate all branches, reject cycles/missing references,
  compile demand in deterministic order and count shared logical textures once with checked bytes.
  Inactive storage is pruned/compacted; no native/engine/Tauri dependency is introduced.
- `cargo test --locked -p golden_video`: **27 unit + 2 public tests passed**. Strict
  all-target Clippy passes. Coverage includes 10k iterative depth, resize identity, raw work/
  dimension/storage limits, overflow, source color/alpha metadata and finite premultiplied values.
- Gate 1 closes: **1/9 = 11%**. Overall stays **3/10 = 30%**. Root/Core formatting and whitespace
  checks pass. The Cargo lock change only registers the new crate using existing `thiserror`.
- Shared IO/time/assets reconciliation identifies existing reusable worker/fence primitives
  and missing production time/persistent asset contracts. P14/P15 are not marked complete;
  the next batch must consume the shared owners rather than create app-local alternatives.
- No native GPU measurements, pixel correctness, runtime atomic-install/retirement, device
  recovery or native budget acceptance is claimed. All eight remaining gates and historical
  native failures remain open at their existing owners.

## M02 verification — 2026-10-28

### Native-presentation lifecycle delivery

- Golden exposes `NativePresentation<R>`, an unstarted `NativePresentationTask`, its stop
  token and two presentation-aware launch entrypoints through the public facade. Default
  configuration stays non-generic/cloneable; default/headless launches start no presentation.
  UI preparation, native clipping and window construction stay in the shared desktop host.
- Explicit exit requests cancellation, keeps the UI pumping while the task retires, then
  destroys the main window and waits for its `Destroyed` event before allowing exit. Joining
  happens after `run_return`. Spontaneous task completion leaves unsaved work/editor open;
  confirmed frontend destruction uses the same host gate. Direct Tauri destroy permission
  is removed from both app capabilities.
- `cargo test --locked --profile dev -p golden_host_desktop -p golden_core -p MapGyver`:
  **24 host tests passed**, one subprocess helper ignored; **1 public facade** and **2 MapGyver**
  tests passed. Coverage includes repeated exit, destroy acknowledgement/code preservation,
  preparation failure, worker cancellation, success/error/panic and worker-side capture drops.
- The feature-gated `native_presentation_smoke` binary passed on Windows through the public
  host: real WebView COM access on the UI STA, underlay clipping, delivered cancellation,
  responsive UI during retirement, live HWND at worker resource drop, and invalid HWND after
  host return. It uses an isolated app identity, loopback port and owned backend data directory.
  The agent needed native WebView profile access outside its filesystem sandbox.
- Two smoke findings were corrected: an integration-test target lacked Tauri's binary manifest
  and failed before main; the supported feature-gated binary receives it. The first native run
  left its HWND live after `run_return`; the explicit destroy/ack gate fixes that observed failure.
  The worker's independent completion was also corrected to preserve frontend save/cancel.
- M02 closes **7/7 = 100%** with its denominator unchanged. Overall is **3/10 = 30%**.
  Engine/protocol-only compilation, strict all-target Clippy, both default app builds and
  required root/Core formatting pass. Existing GPU/cadence/resource/DPI failures and budgets
  remain with M03/M05/M09; no renderer, full-process or physical-output quality pass is claimed.

### Earlier shell and app-isolation delivery

- [Implementation and live evidence](mapgyver-m02-shell.md) cover the app-owned shell, public
  configured host, generic discovery, app-scoped UI state and explicit developer commands.
  Both apps' bundled and development windows subscribed concurrently on their own ports;
  headless hosts independently reached ready read models. Own save/reopen succeeded; foreign
  content returned HTTP 400 without changing live generation, file metadata or node count.
- Full application projects now require `application_id`, checked before sparse expansion,
  callbacks or replacement. Missing/foreign identity is a hard failure without backup recovery.
  This deliberately breaks untagged app projects; no migration is supplied. Only four owned
  Chataigne test/qualification fixtures were tagged. Generic graph/Preferences codecs remain
  independent.
- Final review caught and fixed upload validation occurring after a destination write, and
  `--no-frontend` selecting/admitting another app's development origin. Rejected foreign or
  untagged uploads now leave existing primary/backup files byte-for-byte unchanged and never
  resolve a new destination, including recovery requests. The host keeps the configured app
  URL and admits only its exact origin. `cargo test --locked --profile dev -p
  golden_transport_server -p golden_host_desktop` passes: **86 transport**, **13 desktop** and
  **1 public headless** test; two existing tests remain ignored.
- The affected persistence, engine, desktop, transport, protocol, codegen and xtask test suites
  pass. Engine regression after module extraction: **655 passed, 3 ignored**. MapGyver's two
  project tests and Golden's public-host consumer test pass. UI suites: **Golden 213/213** and
  **Chataigne 193/193**. Both Svelte checks report **zero errors and warnings**.
- `cargo build --locked -p MapGyver -p Chataigne2` and strict all-target Clippy pass. Golden's project I/O and
  sparse codecs now have focused modules under 1,000 lines, preserving public APIs. The
  workbench's type-only session contracts are likewise separate. The npm lockfile only
  registers/hoists the new workspace; dependency version/integrity sets are unchanged.
- M02 closes **6/7 = 86%** with the original seven-item denominator unchanged. The optional
  typed native-presentation host extension stays open. Overall remains **2/10 = 20%**.
  Required root/Core formatting and whitespace checks pass. No native renderer, DPI, input,
  resource-retirement or release qualification is inferred; see the report's limitations.

### Earlier public-bundling checkpoint

- Extracted `golden_codegen_support::ui_bundle` and converted Chataigne's build script to
  configure it. The public config identifies workspace/UI/source roots, npm workspace and
  dependency-cache namespace. Bundle identity includes caller directory, config, toolchain
  and frontend environment; app-specific native SDKs, formulas, node generation and Tauri
  preparation remain in the app. No production host, protocol or UI behavior changed.
- `cargo test --locked -p golden_codegen_support --lib`: **19 passed**. Cases include
  workspace/caller cache isolation, input additions/removals, output damage, skip/prebuilt
  behavior and stable escaped asset paths. Strict all-target Clippy passes.
- `cargo check --locked -p Chataigne2 --no-default-features` passes with `GC_SKIP_UI_BUILD`,
  `GC_UI_ASSUME_BUILT` and `GC_FORCE_NPM_CI` unset for that process. Its build script reports
  a real frontend rebuild; the generated Rust embeds **35 assets**, including root `index.html`,
  with a valid content-cache stamp. This checks the existing desktop-host consumer's build,
  not native launch, default audio-feature coverage or the future MapGyver app.
- First M02 item closes: **1/7 = 14%**, with six items still open. Overall remains **2/10**.
  Required root/Core formatting and diff checks pass; no native/GPU capture or display/overlay
  action was performed. The next source batch is the MapGyver scaffold and app identity seams.
- The user approved proceeding from functional feasibility to the thin shell. The plan,
  native architecture, qualification contract and route decision now assign renderer budgets
  to M03, full output/DPI/input acceptance to M05, and complete-app qualification to M09 and
  owning features. The original failures, budgets and evidence artifacts are preserved.
- The four revised M01 items are supported by their scoped native evidence and recorded
  technology choice. M01 is 4/4; seven complete-scope M02 items are fixed before implementation
  and started at 0/7 before the source work above. Overall accepted milestones are 2/10. This is an approved denominator
  change, not a claim that the original full gate passed or that the shell already exists.

## M01 verification — 2026-10-28

- Consumer-control milestone: 95 deterministic Rust tests, strict default/desktop Clippy,
  release build, generated bindings and required root/Core formatting pass; seven hardware
  tests stay ignored by the standard suite. All 167 Python tests pass. The first 30+8-second
  paused native run completes with fenced, unchanged work/resource-marker counters and zero
  final ownership, but is **rejected** for 3834×2143 geometry and seven raw per-chain QPC
  reversals. Complete original streams and an unordered-only diagnosis are retained. No
  active/paused/active long series is run. The user-confirmed 4K display is detected on NVIDIA
  adapter 1; future qualification must explicitly record that new route and observe overlay
  Off before NVIDIA startup. One named adapter is required, not mandatory dual-vendor success;
  full product Q05 stays with its owning milestones/M09. Counts remain M01 1/8 and overall 1/10.

- GPU-stage milestone: 78 deterministic Rust tests and strict default/desktop Clippy pass;
  seven hardware tests remain ignored by the standard suite. All 123 Python tests pass,
  including raw-u64 stage identity, interval conservation, complete coverage and mode guards.
  Generated bindings, release build and required root/Core formatting pass. The predeclared
  Intel off/on/off controls and staged 30+120-second run retain every native/query/CSV stream;
  independent analysis recomputes all 31,461 rows and exact stage joins. Short controls vary
  and sustained still loses 75 output submissions, so no causal repair or qualifying series
  is claimed. M01 remains 1/8 and overall 1/10. See [stage evidence](mapgyver-m01-native-feasibility.md#same-context-gpu-stage-boundaries--2026-10-28).

- Bounded-import milestone: `cargo test --locked -p mapgyver_native_probe --features desktop`
  passes 69 deterministic tests; seven hardware tests remain ignored by that command. The two
  focused Intel bridge ownership/timeout/resize/pixel tests are explicitly run and pass.
  Strict default and desktop/all-target Clippy, generated binding verification, release build
  and required root/Core formatting pass. The Python suite passes 104 tests, including strict
  import counter/stage validation and configurable collector-tail coverage. Release Q02
  authored-card accuracy covers all 8,294,400 pixels; predeclared 30+8/30+120-second captures
  preserve exact packets and all 20,508 raw CSV rows. Independent recomputation verifies native
  streams, hashes, role/policy/loss observations, identical inputs/context, cache bounds and
  zero logical/alias ownership after cleanup. Both owned trace sessions are absent and the
  pre-existing service remains PID 5864. This verifies bounded import reuse, not a causal
  performance repair or cadence qualification; M01 remains 1/8 and overall 1/10.

- [Output-only policy analysis](evidence/mapgyver-m01-output-policy-analysis-2026-10-28.json)
  recomputes the [predeclared A/B/A](evidence/mapgyver-m01-output-policy-plan-2026-10-28.json)
  and all 48,886 raw rows. Frozen inputs, distinct sessions, chronological native intervals,
  per-role policy and matching actual window contexts verify. Native facts remain separate
  from capture completeness: A1/B live counters are zero; A2 status 4201 leaves them unavailable,
  and the strict analyzer exits 1 with factual runs retained. B is not a repair and the late
  baseline drift remains authoritative. Latest stays default, M01 remains 1/8 (13%) and overall
  remains 1/10 (10%). All three packets are promoted byte-for-byte; owned sessions/processes
  are gone and the existing collector service retains PID 5864.
  Sixty deterministic Rust tests, strict default/desktop Clippy and 88 Python tests pass.
  Generated UI bindings match; actual root/Core formatters, both format checks and whitespace
  checks pass. The derived analysis recomputes exactly and deliberately exits 1 for incomplete
  A2 observations. All 126 evidence JSON files parse and preserve Git input bytes, 231 local
  links in the three updated docs resolve, and all 80 Rust sources remain below 1,000 lines.
  Frozen source/assets/helper/tool/native fingerprints still match after final formatting.
- [Warmed 4K analysis](evidence/mapgyver-m01-presentmon-warmed-analysis-2026-10-28.json) verifies
  the [complete capture packet](evidence/mapgyver-m01-presentmon-warmed-intel-q02-2026-10-28.json),
  kept byte-for-byte through exclusive generated output and promotion. All 4,537 raw rows retain
  SyncInterval 0 and native role identity. The sample has 480 renderer iterations, no missing
  source publications, 480 output Presents/one drop, and 475 editor Presents/no displayed drops.
  Five editor attempts coalesce at the readiness check; they are separate from DXGI Busy HRESULTs.
  The independent post-measurement query succeeds with zero event/log/realtime-buffer loss
  counters, while final capture and analysis loss remain open. Actual owned-window observations
  identify primary 240-Hz/125% and secondary 60-Hz/100% nominal modes; 4K client extent still
  exceeds the secondary monitor. No cadence or physical scanout pass is inferred.
- [Saved-control diagnosis](evidence/mapgyver-m01-presentmon-role-drop-diagnosis-2026-10-28.json)
  recomputes both earlier traces exactly and sorts query-reap rows before CPU-iteration bracketing.
  The two-second warmup includes source resize/absence/return: 397 post-return Q02 iterations
  contain 80 editor/20 output drops and mean scoped renderer GPU time 9.953 ms. This association
  motivates the warmed control; it does not prove a GPU/compositor cause or replace retained failures.
- [Read-only conditions](evidence/mapgyver-m01-warmed-display-conditions-2026-10-28.json) preserve
  OS/driver/WebView/power observations. Controller-level mode fields are not per-output evidence;
  DXGI still lists both attached outputs on Intel and none on NVIDIA. The
  [absent-session control](evidence/mapgyver-m01-etw-absent-query-2026-10-28.json) records status
  4201 and unavailable statistics on its identified earlier query build, rather than zero counters.
  Fifty-five deterministic Rust tests, strict default/desktop Clippy and 67 Python tests pass.
  Generated UI bindings match; actual root/Core formatters, both format checks and whitespace
  checks pass. Both embedded diagnoses reproduce exactly, all 121 evidence JSON files parse,
  227 local links in four updated docs resolve and all 79 Rust sources stay below 1,000 lines.
  The warmed capture's source/tool/native fingerprints still match after final formatting.
  MapGyver evidence JSON disables Git newline normalization so Windows checkouts preserve
  byte-level packet/artifact checksums; existing evidence contents remain unchanged.
  M01 remains 1/8; overall remains 1/10.
- [Exact native-role analysis](evidence/mapgyver-m01-presentmon-role-analysis-2026-10-28.json)
  verifies short Intel [Q01](evidence/mapgyver-m01-presentmon-role-intel-q01-2026-10-28.json)
  and [Q02](evidence/mapgyver-m01-presentmon-role-intel-q02-2026-10-28.json) captures on one
  new release fingerprint. All 2,368 raw CSV rows match queried native addresses and Present-QPC
  lifetimes, with separate editor/output summaries; each retains a display tail beyond the
  recorded Present interval. Q02 preserves 85 editor/20 output drops despite 480 accepted Presents per role.
  No collector loss is reported; independent zero loss, physical outputs and sustained cadence
  stay unverified. Both owned sessions are absent, native/collector exit 0 and no owned probe
  remains. Independent review recomputes raw streams, reports and analyses exactly. Fifty-two
  deterministic Rust tests, strict desktop Clippy and 54 Python tests pass. Generated UI bindings
  match; actual root/Core formatters, their checks and whitespace checks pass. All 116 evidence
  JSON files parse, 213 local links in the three updated docs resolve and all 75 Rust sources
  stay below 1,000 lines. Final captured input fingerprints still match. M01 stays 1/8 (13%);
  overall stays 1/10 (10%).
- [NVIDIA/PresentMon handoff audit](evidence/mapgyver-m01-nvidia-presentmon-handoff-2026-10-28.json)
  verifies eight exact card captures, the first official trace/analysis, source epochs, packet
  integrity, bindings, guardrails and cleanup. All 44 Python tests pass. The unchanged native
  source/executable retains its prior 52-test checkpoint. Required root/Core formatters and
  whitespace checks pass; overlay Off remains unchanged and the existing collector service
  retains PID 5864. M01 remains 1/8 (13%); overall accepted milestones remain 1/10 (10%).
- [NVIDIA/cross-adapter card analysis](evidence/mapgyver-m01-card-accuracy-nvidia-analysis-2026-10-28.json)
  validates all eight fixed procedural/authored captures, exact native integers, common native
  inputs, full pixel coverage and cleanup. Current overlay Off is observed before/after and
  left unchanged. The initial JavaScript-rounded copies remain archived; canonical files were
  restored from exact captured stdout, with no rerun or measurement change. The wrapper now
  defaults to opaque checksummed packets; tests preserve GPU ticks above 2^53 and adjacent ticks.
- [Official PresentMon route](evidence/mapgyver-m01-presentmon-intel-official-route-2026-10-28.json)
  and [independent analysis](evidence/mapgyver-m01-presentmon-official-route-analysis-2026-10-28.json)
  retains 1,192 raw PID-filtered rows, two unlabeled chains and exact eight-second windows:
  480 Presents/Display observations per chain, no reported drops or missing display timing.
  Shared compositor timestamps are identical and cannot be pooled as independent physical
  cycles. Recorded interval p99/max 20.8141/20.8759 ms remain; no timing budget passes by inference.
  No loss is reported; that historical capture has no native role metadata. Independent zero-loss
  proof, sustained series and physical latency remain open. The owned session is absent after
  exit; the existing Intel service stays running.
  The vendor silent failure and help-preflight failure remain preserved separately.
- [ETW access recheck](evidence/mapgyver-m01-etw-access-2026-10-28.json): after the user's
  administrator VS Code relaunch, the release probe creates and stops its empty owned session
  successfully outside the sandbox. The sandbox account still returns status 5. The development
  account is already a Performance Log Users member; no group or policy change was made here.
  Non-elevated access and actual display events remain unverified. No acceptance item closes;
  dashboard counts remain M01 1/8 and overall 1/10.

- [Procedural 1080p](evidence/mapgyver-m01-procedural-card-intel-q01-2026-10-28.json) and
  [4K](evidence/mapgyver-m01-procedural-card-intel-q02-2026-10-28.json) verify every fixed-card
  pixel through the actual shader, independent processes and real ingest bridge. Maximum error
  stays 1/255, with no mismatches/exclusions against tolerance 2/255. Each producer draws once
  with 32 uniform bytes and no CPU pixel upload; renderer/ingest readbacks stay zero. Child
  identities, bounded reports, exact copy costs and six-device cleanup validate. Authored-image
  controls share that captured checkpoint's inputs. Fifty-two deterministic Rust and thirty-six Python tests,
  strict default/desktop Clippy and actual root/Core formatting pass. Broader gates remain open:
  M01 stays 1/8 (13%), and overall accepted milestones stay 1/10 (10%). The
  [saved shader audit](evidence/mapgyver-m01-procedural-card-intel-validation-2026-10-28.json)
  verifies capture/input hashes, full coverage, cleanup, guardrails and dashboard scope.
- [Cross-process 1080p](evidence/mapgyver-m01-authored-card-processes-intel-q01-2026-10-28.json)
  and [4K](evidence/mapgyver-m01-authored-card-processes-intel-q02-2026-10-28.json) verify every
  pixel across two producer children, the existing ingest bridge, renderer and independent
  consumer child. Maximum error stays 1/255, no mismatches/exclusions against tolerance 2/255.
  All four PIDs are distinct, all children are reaped, bridge pools stay at three, explicit copy
  costs match publications, renderer SDK receives/readbacks are zero, and all six ledgers drain.
  Matching local controls share that earlier checkpoint's fingerprints. Forty-eight deterministic Rust and
  thirty-five Python tests, strict default/desktop Clippy and actual root/Core formatting pass.
  Static image accuracy across this route is established; procedural cards, previews, NVIDIA,
  sustained timing and physical retirement remain open. The
  [saved process-route audit](evidence/mapgyver-m01-authored-card-processes-intel-validation-2026-10-28.json)
  verifies all four capture hashes/inputs, generated bindings, guardrails and cleanup.
- [Intel authored-card 1080p](evidence/mapgyver-m01-authored-card-intel-q01-2026-10-28.json) and
  [4K](evidence/mapgyver-m01-authored-card-intel-q02-2026-10-28.json) compare every composite pixel
  with maximum BGRA error 1/255, no exclusions/mismatches against fixed tolerance 2/255, exact
  initial upload/readback costs, renderer readback/upload zero, unregistered senders and all four
  post-fence ownership ledgers zero. Both saved reports match the earlier local-only release inputs and
  wrapper hash. Forty-six deterministic Rust and thirty-four Python tests, strict default/desktop
  Clippy, generated bindings and actual root/Core formatting pass. This is the finite static
  transfer/sampling subset only; other native accuracy/timing/retirement gates stay open.
  [Saved audit](evidence/mapgyver-m01-authored-card-intel-validation-2026-10-28.json) records
  evidence hashes, exact coverage, cleanup, local links, source limits and validation scope.
- [NVIDIA editor isolation](evidence/mapgyver-m01-editor-isolation-nvidia-analysis-2026-10-28.json)
  verifies 61.0434 seconds of busy JavaScript and 61.0238 seconds on `about:blank`, with output
  progress and valid independent-consumer frames in every one-second interval. Fresh session 3
  restores both previews. All 8,126 renderer dependency joins validate; all three contexts have
  zero admission misses and tracked ownership drains. Three startup-invalid B markers and 29
  A-absence observations remain recorded. The normal control accepts 480 calls/path. All four
  final Intel/NVIDIA captures share identical inputs at that lifecycle checkpoint, now historical.
  [Checkpoint validation](evidence/mapgyver-m01-editor-isolation-nvidia-validation-2026-10-28.json)
  retains source/tool hashes. Overlay Off is natively verified
  before/after measurement; original On is restored at 08:17:51 UTC. Full-card/scanout/GPU-path
  qualification remains open. No runtime code changed in this evidence turn.
- [Final Intel editor isolation](evidence/mapgyver-m01-editor-isolation-final-intel-analysis-2026-10-28.json)
  validates 61.0285 seconds busy and 61.0144 seconds on `about:blank`, with no frontend status
  polls, native/Spout progress and fresh consumer pixels in every one-second interval. Reconnect
  restores two previews under session 3/revision 2. Startup B-invalid markers (3), A absence (31)
  and all 8,123 renderer dependency joins remain recorded; no admission misses occur. This is
  no 60-fps pass. The host owns session invalidation through its page-navigation callback. A separate
  normal-launch control validates geometry, 480 calls/path and complete query/dependency coverage.
  Both final captures match the lifecycle checkpoint inputs. Intermediate lifecycle captures and older performance
  cohorts remain historical. Forty deterministic Rust and twenty-nine Python tests, strict
  default/desktop Clippy, generated bindings, UI checks/build and actual root/Core formatting
  pass. See the [final validation](evidence/mapgyver-m01-editor-isolation-validation-2026-10-28.json).
- [Pre-lifecycle NVIDIA analysis](evidence/mapgyver-m01-submission-stages-nvidia-analysis-2026-10-28.json)
  retains three full release samples each for Q01/Q02, all with 7200 calls per renderer/source
  path, no admission misses, complete context query accounting, valid consumer markers and
  drained ownership. All 43,200 renderer dependencies and 161,768 raw context spans validate.
  CPU/per-context GPU/consumer-age targets meet their scoped budgets; full GPU path is unqualified.
  The short and timestamp-free controls share the same inputs with these six runs and the
  earlier eight Intel runs. Original overlay On is verified restored at 06:51:44 UTC.
- Both GPUs collected full Q01/Q02 cohorts on the same pre-lifecycle native inputs, with 86,395 admitted
  dependencies and 323,619 raw GPU spans across twelve runs. This does not replace the earlier
  failures, close physical/display/retirement criteria or qualify another hosting architecture.
- [NVIDIA/cross-adapter checks](evidence/mapgyver-m01-submission-stages-nvidia-validation-2026-10-28.json)
  verify all sixteen current run inputs, saved stream/envelope/tool hashes and the twelve release
  samples' scoped call/CPU/context-query/age targets. Thirty-seven Rust and twenty-six Python tests,
  strict default/desktop Clippy, bindings, actual root/Core formatting and checks pass. Verified
  136 local links, 61 Rust source limits and 88 tool source whitespace checks. No owned probe
  remains. Runtime source is unchanged; original overlay On is restored.
- [Split-stage Intel analysis](evidence/mapgyver-m01-submission-stages-intel-analysis-2026-10-28.json)
  retains three full 30+120-second Q01 and Q02 samples with identical current inputs. All
  43,195 admitted renderer batches join their exact dependencies, with 161,851 raw GPU spans
  verified. Q01 sample 1 retains five renderer/source-A and one source-B sampled admission
  misses; 7193/7193/7195 accepted calls meet the scoped 0.1% shortfall target. Other samples
  accept all 7200 calls/path. Q02 sample 2 retains 5/4/1 warmup misses. CPU/context query
  accounting, independent consumer markers and all post-fence ledgers validate.
- Current recorded CPU/per-context GPU/consumer-age targets meet their scoped budgets, but the
  full GPU critical path remains unqualified. Signal reaches 3.6933 ms, Flush 0.6641 ms and
  profiling end markers 0.040 ms. The earlier 207.798-ms combined QueryEnd failure is not
  reproduced or attributed. Q01 sample 1 retains a 203.290-ms correlated GPU-span outlier.
  The timestamp-free control completes 480 ticks without a final clear. All eight current
  runs match their native input hashes; no NVIDIA test or setting change occurs in this turn.
- [Final split-stage checks](evidence/mapgyver-m01-submission-stages-validation-2026-10-28.json)
  verify all eight current inputs, saved stream/envelope/tool hashes, scoped targets for all six
  release samples, 135 local links, 61 Rust source limits and 88 untracked tool whitespace checks.
  Thirty-seven deterministic Rust tests, twenty-six Python tests and the native Intel held-reader
  test pass. Strict default/desktop Clippy, generated bindings and actual root/Core formatting
  plus checks pass. No owned probe remains; M01 is still in progress.
- [Correlated Intel analysis](evidence/mapgyver-m01-pipeline-intel-analysis-2026-10-28.json)
  retains three identical historical-input Q02 release samples at 30+120 seconds. All 21,583 admitted
  renderer batches have complete source dependency coverage; all context queries, raw consumer
  markers and post-fence ledgers validate. Sample 3 accepts 7186/7187/7188 editor/output/Spout
  calls against 7200, failing 0.1% cadence. A 209.139-ms render tick contains 207.798 ms in
  QueryEnd; its internal cause remains open. All sustained consumer-age p95 values meet 50 ms;
  the short control's 56.480-ms A failure remains. The full GPU budget stays unqualified.
- A paced collector preserves the 3,615,951-byte replacement stream across 56 indexed chunks,
  with whole-stream SHA and all envelope lengths/hashes verified. The prior stale-binary and
  terminal-truncated/cancelled attempts remain recorded, not counted toward the series.
  Thirty-seven deterministic Rust tests, twenty-four Python tests and native Intel bridge
  ownership pass; strict default/desktop Clippy and generated bindings pass. No NVIDIA test or
  overlay change occurs in this turn; original On restoration remains the last observed state.
- [Final pipeline checks](evidence/mapgyver-m01-pipeline-validation-2026-10-28.json) verify
  its then-current runtime inputs, saved analysis/artifact hashes, 80,885 raw GPU spans, 129 local links,
  all 61 Rust source-file limits and 88 untracked tool source whitespace checks. Actual root/Core
  formatting and checks pass. No owned native probe remains; M01 stays in progress.
- [Timeline-completion analysis](evidence/mapgyver-m01-timeline-completion-analysis-2026-10-28.json)
  retains headless negative controls: NVIDIA event-only modes admit 4/480 ticks, while timestamp
  modes admit 480/480. The final binary reproduces event-warmup 4/480 and timeline-warmup 480/480;
  timeline admission and retirement also complete 480/480 without the final diagnostic clear.
  Intel timeline controls complete 480/480 too. No pixel readback, CPU image upload or dummy GPU
  work is added to the runtime. Modern D3D11 fence interfaces are required explicitly.
- Native NVIDIA 30+8-second Q01 (intermediate admission repair) and Q02 (final completion repair)
  controls each accept 480 calls per renderer path with zero admission misses and complete query
  coverage on all three contexts. Final Intel 2+8-second Q01 also accepts all 480 renderer calls;
  source contexts record 477/480 sampled receives with full query accounting and no admission
  misses. These short controls leave repeated 30+120-second final-variant series open.
- The final NVIDIA 100-rebuild control verifies all 100 independent generation markers, drains
  101 renderer/202 ingest scopes and retains raw consumer markers. Generation-10 baseline is
  595.520 MiB; maximum positive live private growth is 21.184 MiB, absolute peak 616.703 MiB.
  The earlier failing capture peaks at 609.410 MiB, so this is no memory-root-cause closure.
  Stable checkpoints plus the final live drain have 1055–1067 handles against baseline 1062;
  partial-setup sparse samples remain separate. Final live drain is 10.016 seconds and scoped
  teardown is 0.515 seconds. Full Q05/physical retirement remains open.
- [Current native conditions](evidence/mapgyver-m01-query-progress-conditions-2026-10-28.json)
  verify overlay Off before/after NVIDIA tests, then original On restored at 05:17:26 UTC.
  Those native controls, reconstruction and headless controls match their recorded release
  fingerprints. Intermediate/historical inputs remain distinct. Thirty-five deterministic Rust
  tests, sixteen Python tests and the native Intel bridge ownership test pass.
- [Final local checks](evidence/mapgyver-m01-timeline-completion-validation-2026-10-28.json)
  verify final capture fingerprints and analysis hashes, strict default/desktop all-target Clippy,
  generated Rust/TypeScript bindings, actual root/Core formatting plus checks, 122 local links,
  all 59 Rust source-file limits and 82 untracked tool source whitespace checks. No owned probe
  remains. This validation does not close the outstanding native qualification criteria.
- [Repaired NVIDIA analysis](evidence/mapgyver-m01-source-worker-stop-order-nvidia-analysis-2026-10-28.json)
  retains three 30+120-second samples each for Q01/Q02, with 7,200 accepted calls per path,
  zero sampled late/busy outcomes, full query coverage on all three contexts and scoped CPU/
  consumer-age targets met. All six retain 254–255 source-A warmup admission misses. The full
  asynchronous GPU critical path remains unqualified; warmup lifecycle/instrumentation effects
  were investigated by the later headless controls. All nine runs have identical historical inputs.
- The repaired performance captures retain 920 renderer rows, 1,812 ingest rows and 44,159 raw
  consumer observations, with all eight renderer/sixteen ingest/twenty-four child ledgers drained.
  The reconstruction adds 240 rows and 12,175 observations, with 101 renderer/202 ingest scopes
  retired. The [native overlay conditions](evidence/mapgyver-m01-source-worker-nvidia-conditions-2026-10-28.json)
  record Off before startup and after tests, then verified restoration of original On. No
  desktop snapshots occur during measurements; no display or other settings change.
- Historical checks validate all nine repaired captures against their then-current release inputs after
  formatting, all saved envelope/artifact/analyzer hashes, the preserved old failure and
  failed private-growth target, 116 local documentation links and 58 Rust source-file limits.
  The 35 deterministic Rust tests, fourteen Python tests, native Intel bridge test, strict
  default/desktop Clippy, generated UI bindings and root/Core formatting pass. No owned probe
  remains; NVIDIA overlay is verified restored On.
- The pre-repair [NVIDIA Q01 cohort](evidence/mapgyver-m01-source-worker-nvidia-q01-release-2026-10-28.json)
  retains two successful timed samples and a third shutdown failure with no final native report.
  Stop was visible before reader release. The [ordering regression](evidence/mapgyver-m01-source-worker-stop-order-regression-2026-10-28.json)
  fails under the old order and passes after reader/mailbox release precedes the Release-store.
  The repaired release controls accept all 480 calls per path; 35 deterministic Rust tests,
  fourteen Python tests and strict desktop Clippy pass. No numeric budget is relaxed.
- The repaired [NVIDIA 100-rebuild capture](evidence/mapgyver-m01-source-worker-stop-order-nvidia-recreation-2026-10-28.json)
  retires 101 renderer scopes and 202 source workers, verifies generations 1–100 and drains all
  tracked ledgers. Compared with generation 10's live checkpoint, handles range 1054–1067
  against baseline 1062. Maximum positive live private growth is **47.926 MiB**, failing the
  32-MiB target; final settling falls below the baseline. Post-release private observations
  separately reach +69.176 MiB and are not substituted for live comparisons. Final live
  settling is 10.017 seconds and scoped worker/window teardown is 0.531 seconds. Ownership
  success does not prove physical retirement or a complete resource-budget pass.
- [Source-ingest analysis](evidence/mapgyver-m01-source-worker-analysis-2026-10-28.json) retains
  three full 30+120-second Intel Q02 runs and two short release controls with identical inputs.
  Every sampled Q02 path accepts 7,200 calls, with full renderer/source query coverage and
  scoped CPU/consumer-age and individual-context GPU targets met. Full GPU critical path stays
  unqualified. Sample 2 retains a 16.862-ms GPU event; sample 3 a 4.876-ms query-end/flush
  operation and a warmup GPU-progress stall: 57 renderer coalesces at clock-origin seconds
  24.141–25.191, plus 50/46 source coalesces. There are no event omissions. The measured cap
  works under native pressure; neither sampled success nor isolation establishes a root cause.
  Source-ID repetitions remain explicit and are separate from displayed frames.
- Five renderer scopes, ten ingest reports and fifteen external child reports drain tracked
  ownership in those runs; evidence retains 470 renderer rows, 924 ingest rows and 22,558 raw
  consumer observations. A separate same-input [ten-rebuild control](evidence/mapgyver-m01-source-worker-recreation-2026-10-28.json)
  retires eleven renderer scopes/22 ingest workers, independently verifies generation markers
  and alpha, and keeps the final scene live for 10.017 seconds. Readiness is 157–191 ms and
  scoped worker/window teardown 0.335 seconds. Full Q05 and physical/complete-process retirement
  remain open.
- Input SDK receivers now own separate source-thread devices/contexts and recover the same
  explicit sender names. A bounded three-slot NT-handle/keyed-mutex bridge exposes GPU images
  without moving SDK instances or issuing renderer copies/readbacks/uploads. Each source also
  has independent two-batch GPU admission. Renderer reads fence before reader release and worker
  joining; every source ledger is reported separately and included in the conservative payload
  upper bound. Full asynchronous GPU critical path remains unqualified, preserving its budget.
- [Final short source-isolation controls](evidence/mapgyver-m01-source-worker-controls-2026-10-28.json)
  retain Intel Q01/Q02 at 2+8 seconds, unchanged identical inputs, exit 0 and empty stderr.
  Both accept all 480 calls per path while source A has 68.318/68.152-ms receive maxima on its
  worker. Renderer CPU maxima are 1.333/2.272 ms, with zero SDK receives on that context.
  Worker pools and every context's admission/ownership drain after fences. The native Intel
  boundary test holds three readers across 100 producer reuse rejections, then verifies one
  released slot's safe reuse and changed content. Only exact S_OK grants keyed-mutex ownership.
- The release build, 34 deterministic Rust tests, fourteen Python tests, explicit native bridge
  test, strict default/desktop Clippy and generated UI binding checks pass for source isolation.
  Schema 11 preserves worker timings and bounded admission; stale binaries, excess capacity,
  hidden renderer receives/copies, mismatched timing and retained ownership are rejected.
- Final checks validate all six release captures against current inputs after root/Core
  formatting, all saved envelope/artifact/analyzer hashes, 102 local documentation links and
  57 Rust source-file limits. The 34 deterministic and fourteen Python tests pass again;
  generated UI bindings and strict Clippy remain clean. No owned probe remains. Displays and
  NVIDIA settings are unchanged throughout this Intel-only batch; original overlay On remains.
- [GPU-admission analysis](evidence/mapgyver-m01-gpu-admission-analysis-2026-10-28.json) retains
  three independent 30+120-second Intel Q02 release samples, each with 7,200 accepted calls per
  path, full query coverage, zero sampled outliers and scoped CPU/GPU/consumer-age targets met.
  Each peaks at two batches and drains all 8,990 submissions. Native coalescing is not observed
  in this cohort; forced backlog is covered by policy/accounting tests. The earlier failures
  remain evidence and a root-cause fix is not claimed. All five control/series inputs match;
  verification covers 470 renderer rows, 22,560 raw consumer observations and zero tracked
  post-fence ownership in five renderer scopes and fifteen child reports.
- Final verification reruns all 33 deterministic Rust tests and thirteen Python tests, checks
  generated UI bindings, root/Core formatting, 90 local documentation links and source limits.
  Saved envelope/analyzer hashes and all five current release fingerprints still match after
  formatting and binding checks. No owned probe remains; NVIDIA overlay and displays were
  untouched during this Intel-only batch.
- Independent two-slot renderer admission uses FIFO completion markers outside the profiling
  ring. GPU-busy ticks issue no receive/draw/send work and remain Busy on all three cadence
  paths against unchanged targets. Runtime submission owns the final flush, including frames
  without profiling slots; sparse pipeline counters and post-fence draining are reported.
  This does not bound fixture/driver/display queues or remove blocking SDK lifecycle calls.
- [Two short release controls](evidence/mapgyver-m01-gpu-admission-controls-2026-10-28.json)
  retain Intel Q01/Q02 at 2+8 seconds, exit 0, empty stderr and unchanged inputs. Both peak at
  two batches and drain every submission. Each admits 477 sampled ticks without GPU coalescing,
  preserves one swapchain readiness miss per window and a roughly 68-ms lifecycle stall.
- Release build, 33 deterministic Rust tests, thirteen Python tests and strict default/desktop
  Clippy pass for the admission implementation. Tests exercise a blocked two-slot pipeline,
  FIFO reuse, error propagation, coalesced trace/cadence accounting and stale schema-9 rejection.
  Historical captures and their failures remain bound to their original fingerprints.
- [Operation-timing evidence](evidence/mapgyver-m01-operation-timing-analysis-2026-10-28.json)
  retains two 2+8-second Intel release controls and a separate three-sample 30+120-second Q02
  series with identical inputs. The new series' first sample has 18 CPU and six GPU outliers,
  twelve skipped queries and 150.230/140.561 ms CPU/GPU maxima. Its 68/68/48 call shortfall and
  772.239/294.210 ms consumer-age maxima remain failures. No CPU/GPU event is omitted.
  Other samples do not replace it; the second retains one editor readiness miss.
- Per-operation attribution puts the large intervals around receive A/B (111.582/103.566 ms),
  Spout send (52.063 ms) and query completion/flush/accounting (106.093 ms). Resource sampling
  stays below 0.04 ms and Present below 0.5 ms. Query gaps precede long CPU intervals. The short
  controls separately retain ~68 ms input-A lifecycle calls and the SDK cleanup-sleep finding;
  neither implies an established driver/mutex root cause for the long-run stalls.
- Release build, 30 deterministic Rust tests, twelve Python tests and strict default/desktop
  Clippy pass. Rust-owned trace DTOs retain separate warmup/sample caps, absent operations,
  maxima, omissions and frame/QPC identities; GPU records reuse existing queries. The independent
  verifier checks counts, ordering, spans and raw ticks without synthesizing historical data.
  No production runtime/UI/protocol or workload/acceptance-budget changes occur.
- Final evidence checks validate five immutable-input release runs, 470 renderer rows and
  22,527 raw consumer observations. All fifteen child reports and five renderer scopes drain
  tracked ownership; all trace events fit their caps with no omitted events. Current release
  fingerprints and analyzer/artifact hashes match. Generated UI bindings, root/Core formatting,
  101 local links, fences/whitespace and source limits pass. No owned probe remains.
- [Intel repeated-release analysis](evidence/mapgyver-m01-intel-release-analysis-2026-10-28.json)
  retains all six full-duration sample distributions and maxima. All runs exit 0 with empty
  native stderr, unchanged identical inputs, zero sampled geometry mismatches and verified
  per-chain queue limit 1. Scoped CPU and consumer-age p95 targets pass. Recorded GPU
  percentiles are within target, but Q02 sample 1 has 16 skipped queries and cannot qualify
  its complete GPU budget; the other five scoped GPU results pass.
  Q01 records 7,200 accepted calls per path in each sample. Q02 sample 1 retains 7,155 attempts,
  24/23 readiness misses, seven recorded late calls per path and large timing/age maxima.
  No full cadence or native-gate pass is claimed; repetitions/skipped source IDs remain visible.
- Lossless envelopes retain reports larger than the terminal limit. Q02's known stderr
  interleaving is explicitly demultiplexed and full decoded length/SHA-256 validated. The
  read-only analysis rederives all 43,165 raw consumer observations and checks 900 renderer
  resource rows. All six renderer scopes and eighteen child reports end at zero tracked
  ownership; scoped worker/window teardown is 0.419–0.441 seconds. Physical residency,
  displayed cadence and complete-process shutdown remain unqualified.
- The release build, 27 deterministic Rust tests and ten Python tests pass. Tests retain
  high integer clocks, corrupt envelopes/markers, failed/incomplete series and large reports.
  Favorable GPU percentiles cannot qualify incomplete query accounting.
  Analyzer/artifact hashes and the six-sample native input identity are retained in the saved
  analysis. Desktop capture stays inactive throughout; no display/NVIDIA setting changes occur.
- Final saved-evidence verification checks all six envelopes against current release inputs,
  analyzer/artifact hashes, 43,165 raw observations, 900 renderer rows and eighteen child fences.
  Q02's shortfall, maxima and sixteen missing GPU samples remain explicit failures/gaps.
  Root/Core formatting, 95 local links, fences, whitespace and source line limits pass;
  no owned native probe remains.
- [Secondary-display evidence](evidence/mapgyver-m01-secondary-native-ui-2026-10-28.json)
  records 32 native keyboard/mouse inputs plus one activation restore on the 100% secondary
  display, with settled menu/modal/clipping/dock/scroll/zoom observations and outside-region
  releases. Hidden and minimized intervals last 65.458 and 79.263 seconds respectively;
  independent native output remains visible and submission/send counters progress. No displayed
  cadence, freshness, physical-pixel alignment or quantitative latency pass is claimed.
- The 608.936-second interactive process exits 0 with matching final revision 63 and scoped
  teardown 0.486 seconds. Terminal truncation loses the middle of its large report; the
  [invalid captured JSON](evidence/mapgyver-m01-secondary-terminal-truncation-2026-10-28.txt)
  remains retained. Intact header, complete observer report (610 resource rows) and footer
  decode separately; source/renderer sections stay unavailable. Both owned windows close and
  capture resets. Input fingerprints are unchanged through the interactive run.
- The runner now captures explicit diagnostic underlay sessions with lossless row compaction
  and distinct interactive labeling. Five Python tests reject invalid qualification combinations
  and preserve observation/null-segment data. A separate eight-second native control exits 0
  with empty stderr, unchanged inputs and eight renderer rows; it does not replace missing
  long-run rows or repeat the gestures. The original and runner-control inputs remain distinct.
- Saved evidence verification redecodes the intact native fields from the retained truncated
  capture and reconciles all 610 observer rows. The later control matches current fingerprints;
  only the intentional runner edit separates it from the original interactive inputs. Root/Core
  formatting, 88 local documentation links, fences, whitespace and source line limits pass.
  No owned probe remains. Rust/UI runtime sources and the tested executable are unchanged.
- [Presentation-admission evidence](evidence/mapgyver-m01-present-admission-2026-10-28.json)
  preserves an initial Intel control, native ETW permission result and four final immutable-input
  Intel/NVIDIA Q01/Q02 debug controls. All exit 0 with empty stderr and unchanged inputs; all 80
  sparse swapchain observations report maximum frame latency 1. Whole-generation admission
  counters reconcile independently from sampled cadence. The renderer acknowledges a layout
  revision only after an accepted editor Present; this does not acknowledge physical visibility.
- The final complete suite passes 27 deterministic and five native tests, including all four
  reconstruction subruns. Four Python tests, strict default/desktop Clippy and generated UI
  binding checks pass. Original NVIDIA overlay On is restored and verified, settings closed
  and capture reset. Readiness-busy/late observations stay retained; no full cadence pass is claimed.
- Saved evidence checks reconcile the 40 sparse resource rows, admission counters and all
  twelve child zero-ownership fence reports. Current source/assets/binary fingerprints match
  the four final controls. Root/Core formatting and final whitespace/link checks pass; no owned
  probe remains. ETW reports status 5 / `0x80070005`, no owned session, providers or trace file;
  displayed-frame, physical latency and repeated release qualification remain open.
- [Child-resource evidence](evidence/mapgyver-m01-child-resources-2026-10-28.json) retains an
  initial Intel control and four final immutable-input Intel/NVIDIA Q01/Q02 controls, all with
  empty stderr and unchanged input checks. Their 160 child snapshots include all source phases;
  independent source A/B/consumer logical peaks are 19.775/7.031/7.949 MiB at Q01 and
  79.102/7.031/31.680 MiB at Q02, matching across GPUs. Every child drains tracked allocations
  after its final fence. These descriptor payload figures do not measure physical residency.
- Sources now use typed readiness/resource/completion messages and the same bounded JSON
  reader as the observer. The final suite passes 24 deterministic and five native tests,
  including phase-byte, copy-byte, source-upload, reaping and pipe-loss checks across both GPUs.
  Four Python tests, strict default/desktop Clippy and UI binding drift checks pass. Overlay
  Off is verified before/after native runs, then original On restored; settings/capture closed.
- Saved evidence checks rederive all 160 child snapshots, phase payloads, source uploads and
  explicit copy totals, including twelve zero-ownership post-fence reports. Current fingerprints
  match the final cohort. Root/Core format checks, whitespace and 73 local links pass; no
  source file exceeds the line limit and no owned probe remains.
- [Frame-age evidence](evidence/mapgyver-m01-frame-age-2026-10-28.json) preserves the initial
  456-sample drift-prone Intel control, the reproduced JSON precision failure and four final
  immutable-input Intel/NVIDIA Q01/Q02 controls. Each final control exits 0 with empty stderr,
  479–480 raw samples, zero invalid markers and one 20 KiB observer log readback. Renderer
  readback/uniform-upload counters remain zero. Query polling, source absence, repetitions,
  skipped source IDs and timing outliers remain explicit. Overlay Off is verified before/after
  native runs, original On restored, settings closed and capture reset.
- The final complete probe suite passes 19 deterministic and five native tests, including all
  four reconstruction subruns and source pipe-loss cleanup. Three Python tests and strict
  default/desktop Clippy pass; generated UI DTOs remain in sync. Exact summary comparison is
  preserved by the probe-local `serde_json` `float_roundtrip` feature, with a new regression.
- Final root/Golden Core format checks, whitespace and 70 local links pass. An independent
  saved-evidence check rederives all 1,918 marker checksums, sample clocks, frame counts and
  quantiles; current source/assets/binary fingerprints match the final four-control cohort.
  No owned probe remains.
- [Live-scene retirement evidence](evidence/mapgyver-m01-scene-retirement-2026-10-28.json) retains
  all 101 generation/checkpoint/fence records, 100 independently verified markers and 236 sparse
  process/allocation samples, including ten during final live settling. Release inputs remain
  unchanged; native exit is 0 with empty stderr. NVIDIA overlay Off was verified before process
  startup and after capture, then original On restored. Handle/private-growth targets pass for
  this reconstruction subtest; source/observer logical totals and physical residency remain open.
- [Settling controls](evidence/mapgyver-m01-settling-controls-2026-10-28.json) retain two completed
  positive controls and a native close during the observed settling phase. Cancellation returns
  exit 1 and no success report. The explicit reconstruction test passes all four GPU/scenario
  subruns; 14 deterministic tests, two Python tests, strict default/desktop Clippy and UI/generated
  checks pass. The final ten-second interval extends the watchdog by ten seconds, without
  reducing any operating interval, workload or numeric budget.
- Root/Golden Core formatting and format checks pass. Saved evidence table/count integrity,
  overlay restoration records, 66 local documentation links and whitespace checks pass;
  no owned native probe remains. Subsequent source edits only clarify comments and format the
  Python runner/tests; captures retain the exact pre-edit fingerprints used during each run.

## M01 verification — 2026-10-27

- The pinned sys binding builds statically and the linked SDK reports `2.007.017`. All 29
  packaged SDK files match the upstream tagged commit after CRLF normalization; exact hashes
  and package/license/ownership findings are retained in the [M01 report](mapgyver-m01-native-feasibility.md).
- The explicit native integration test passes on Intel and NVIDIA and checks software-adapter
  rejection. Final standalone smoke reports verify live resize, reconnect, content and fixture
  registration cleanup, with four diagnostic readbacks per adapter; counts are poll observations,
  not a throughput claim. [Evidence](evidence/mapgyver-m01-spout-smoke-2026-10-27.json).
- Strict probe/all-target Clippy, root/Core format checks and Chataigne default-member/dependency
  isolation checks pass. No Chataigne/Golden production source or generated protocol changed.
- The [underlay plumbing evidence](evidence/mapgyver-m01-underlay-plumbing-2026-10-27.json) records
  eight-second Intel/NVIDIA runs, each with 473 preview presents and 474 native output presents /
  Spout sends, matching host/browser 1.25 scale, clean worker/fixture/window shutdown and empty
  stderr. This proves submissions and control geometry, not visible layering or frame cadence.
- The Svelte 5 fixture builds with zero check errors/warnings. Rust-owned DTO generation has an
  independent executable and a passing drift check, including while the native probe is live.
  Three geometry tests and the final two-adapter native recovery test pass. Default and desktop
  strict Clippy pass. No app launch defaults changed.
- The [independent consumer evidence](evidence/mapgyver-m01-composite-consumer-2026-10-27.json)
  records four color/lifecycle checkpoints on Intel/NVIDIA, five finite two-pixel diagnostic
  readbacks per run, missing-output rejection and complete bounded GPU-query accounting.
  Seven deterministic and two native tests pass. Corrected CPU/GPU p95 values are retained with
  startup maxima; the superseded timing submission error is documented rather than counted.
- The [fixed-workload evidence](evidence/mapgyver-m01-fixed-workloads-2026-10-27.json) retains
  first warmed Intel release samples for Q01/Q02, each with 7,200 CPU/GPU samples, 150 sparse
  resource observations, exact output/unclipped preview geometry, zero omitted queries and
  empty stderr. CPU p95 is 0.567 / 0.639 ms and GPU span p95 is 1.257 / 4.807 ms respectively.
  The 4K client rectangle is allocated beyond the available monitor; 4K scanout is unqualified.
- Eight deterministic Rust tests, three explicit native tests across Intel/NVIDIA and two
  Python evidence-integrity tests pass. Fixed-mode coverage checks geometry, alpha and warmup
  exclusion. Release build, default/desktop strict Clippy, UI checks/build and DTO drift pass.
- Device reconstruction adds persistent-session / per-device scopes, independently observed
  generation markers, bounded GPU retirement fences, post-release process snapshots and
  explicit clean source exit. A source-readiness handshake fixes an observed startup race;
  parent-pipe EOF cancels fixtures. The current suite passes nine deterministic Rust tests,
  five explicit native tests and two Python evidence tests. Short Q01/Q02 reconstruction
  checks pass across both GPUs; genuine device removal/reset remains separate.
- Visual underlay correctness, full alpha/reference-card checks, mixed DPI/input, full Q01/Q02 timing/memory/latency, device recreation
  and sustained cleanup remain only partially qualified. M01 stays in progress and M02-M09 remain unstarted.
- [100-cycle release evidence](evidence/mapgyver-m01-device-recreation-2026-10-27.json) verifies
  every replacement generation on Intel/NVIDIA. Intermediate post-release private growth is
  9.875 / 52.180 MiB and handles range 346–354 / 439–831 respectively. NVIDIA retirement is
  unresolved; no 10-second driver drain or complete Q05 acceptance is claimed.
- [Native UI evidence](evidence/mapgyver-m01-native-ui-2026-10-27.json) records the menu layout
  failure and repair, 31 completed native inputs on the 125% primary display, >60-second hidden
  preview content/submission observations and a 125%→100% monitor transition. A secondary
  full gesture sequence, OS 150%/200%, negative origins, physical-pixel alignment and latency
  remain open. The 900-second interactive probe exits cleanly with matching revision 60;
  interactive capture timing is not Q01/Q02 performance evidence.

- [Headless ownership isolation](evidence/mapgyver-m01-ownership-isolation-2026-10-27.json)
  retains 17 runs / 2,600 scopes and a ten-second drain per run. Intel handles stay flat;
  NVIDIA texture creation retains handles even after a zero final device Release observation.
  Private memory largely drains, so it is tracked separately. Driver/overlay cause remains open.
- [Validation follow-up](evidence/mapgyver-m01-isolation-validation-2026-10-27.json) preserves
  failed sample-count/content checks before a computer-use session reset and a successful short
  Q01 control afterward. The subsequent complete suite passes nine deterministic and five native
  tests across both GPUs. Two Python evidence tests, default/desktop strict Clippy and root/Core
  format checks pass. No thresholds, retry limits or production teardown behavior changed.

- [Termination controls](evidence/mapgyver-m01-generation-termination-2026-10-27.json) reproduce
  an incorrect early-close success after 28.273 seconds of a requested 122-second Q01 run.
  The repaired worker records one typed end reason before cleanup and rejects interrupted
  qualification or recreation watchdog expiry. A native repeat returns cancellation; the
  interactive underlay still closes cleanly with `window_closed`. The redundant reconstruction
  boolean was removed. Two regression tests cover early close and incomplete final intervals;
  native scene/reconstruction tests assert successful reasons. Eleven deterministic tests,
  five native tests, two Python tests, strict default/desktop Clippy and root/Core format checks
  pass. Reconstruction was re-exercised after the report cleanup. M01 remains open.

- [Allocation/cadence evidence](evidence/mapgyver-m01-allocation-cadence-2026-10-27.json) retains
  four short Q01/Q02 debug controls, one immutable input set and all 40 sparse observations.
  Both GPUs record 55.010 / 179.595 MiB logical renderer peaks and zero tracked allocations
  after the final context fence. SDK calls, descriptor ownership, raw DXGI errors/counters and
  per-output acceptance/deadline metrics remain separate from displayed frames and physical residency.
  The final suite passes 14 deterministic and five native tests plus two Python tests and strict
  default/desktop Clippy. A standalone initial-black-frame failure is retained; all fixture
  startup paths now use the existing first-submitted-image readiness handshake.
- [Overlay controls](evidence/mapgyver-m01-nvidia-overlay-control-2026-10-27.json) retain the
  approved on/off/on test and verified restoration. Handles grow 242→422 with overlay enabled
  before/after, and stay at 221 when disabled. Capture DLL version 11.0.9.565 remains loaded
  throughout; its installed file hash and probe inputs stay unchanged. This isolates an
  environment-dependent failure, without proving NVIDIA's internal handle ownership/type.
  The user approved overlay-off as a qualification prerequisite, with no budget/workload changes;
  overlay-on evidence stays retained and the actual user setting remains restored to On.

## M00 verification — 2026-10-27

- Read-only `tools/mapgyver/collect-baseline.ps1` executed successfully twice under noninteractive
  Windows PowerShell. Parsed JSON records actual OS, WebView2, GPU driver, toolchain and locked
  host versions; unavailable native observations are explicit. Evidence capture preceded the
  branch switch and therefore correctly records `main` as its historical branch.
- PowerShell parser, evidence schema/lock-hash checks, local Markdown links, balanced code
  fences and whitespace checks passed for the M00 files.
- Root and Golden Core `cargo fmt --all` and final format checks passed. The six remaining
  unrelated dirty root/architecture documents were checked by SHA-256 and preserved byte-for-byte.
  Chataigne runtime/P11 changes are now in the externally integrated checkpoint `40acd67d`.
- No Rust/UI runtime changed; no runtime test-suite, native frame, Spout interop, DPI, frame-time
  or floor-OS pass is claimed. The chosen Windows 11 24H2/build 26100 floor is a qualification
  target; only build 26300.9711 is inventoried locally.

## Planning verification

- Ownership/dependency review and native-rendering source accuracy review completed.
- Ten touched documentation files: 118 local links resolve; code fences and whitespace checked.
- `cargo fmt --all -- --check` and the equivalent Golden Core manifest check passed.
- No application code changed and no native rendering probe ran in this planning session.
