# MapGyver implementation status

## At a glance

**Updated: 2026-10-28 · Overall: `██░░░░░░░░░░░░░░░░░░` 10% · 1/10 phases complete**

**Current topic:** M01 scope review and a bounded owned-output fixture correction.
The native route has functional evidence, but the original full gate has not passed. The
placement correction passes compile/deterministic checks; native verification remains open.
New profiling and native captures are stopped while the proposed dependency change below
awaits the user's decision. The user confirms NVIDIA overlay is Off; no overlay investigation
or setting change is needed for this review.

**Next:** decide the explicit [scope correction](#next-action), then follow that decision.
**Blocker:** the current plan requires the full native gate before M02. Cadence, resource
growth, physical geometry and remaining display/input checks prevent that acceptance.
The proposed functional-feasibility gate does not change the current 1/8 count or budgets.

| Phase | State | Progress | Done / total items |
| --- | --- | --- | --- |
| [M00 Baseline/workloads](mapgyver-m00-baseline.md) | Complete locally | `████████████████████` 100% | 1/1 |
| [M01 Native feasibility](#m01-acceptance-items) | In progress | `███░░░░░░░░░░░░░░░░░` 13% | 1/8 |
| [M02 Multi-app seams/shell](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M03 Rendering/channels](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M04 Source media](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M05 Screens/Surfaces/editors](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M06 Composition/editor](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M07 Matrix/editor](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M08 App lifetimes](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |
| [M09 App/artifact qualification](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |

Overall counts fully accepted milestones: `1 / 10 = 10%`; partial phases do not advance it.
Phase percentages count closed acceptance items, rounded to the nearest whole percent
(`M01: 1 / 8 = 13%`). Bars use 20 cells, rounded from the exact fraction. Each phase uses
its whole plan acceptance as one item unless a fixed breakdown is recorded here before
implementation continues. M01 uses the eight existing [gate criteria](mapgyver-m01-native-feasibility.md#remaining-gate).
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
| D3D11/Spout ownership, two sources, resize/restart smoke | Done | Limited native smoke accepted; production supervision belongs to later delivery and full Q01/Q02 remains below. |
| Ordinary underlay, DOM layering/clipping/input | Open | Primary/secondary native sequences recorded; complete configuration and latency coverage remains. |
| Native rectangle output and composited Spout send | Open | A physical 4K/150% display is now connected on NVIDIA adapter 1. The paused Intel smoke spans monitors and yields 3834×2143, so explicit output placement/DPI sizing and physical-output accuracy remain. Historical fixed-card results retain their scopes. |
| Mixed DPI, negative origins, drag/resize/docking/zoom | Open | 100%/125% observations recorded; 150%/200%, negative origins and measured alignment remain. |
| Minimized/disconnected-editor independence | Open | Intel/NVIDIA busy/blank-document continuity and reconnect pass at recorded fingerprints; full displayed/minimized qualification remains. |
| Color/alpha, copies, cadence/latency, queues and memory | Open | Fenced paused-consumer proof and cleanup pass, but its smoke fails geometry and raw CSV chronology; no long comparison is run. Historical stage timing and fixed-card results keep their fingerprints. Cadence, final/analysis loss, other routes/lifecycle cards, GPU critical path, scanout and resources remain. |
| Device recreation, sustained retirement and shutdown | Open | Controlled native rebuilds recorded; native-harness lifecycle budgets, physical retirement, genuine removal/reset where testable and full-process shutdown remain. Full product/Matrix Q05 belongs to its owning milestones/M09. |
| Production host/backend/binding decision | Open | D3D11/sys binding and ordinary underlay remain provisional pending the full gate. |

Evidence: [native feasibility report](mapgyver-m01-native-feasibility.md) and
[fixed qualification contract](mapgyver-qualification-contract.md). A passed smoke item does
not approve production architecture or close the seven broader items.

## Detailed status

Updated: 2026-10-28. Entry point: [MapGyver implementation plan](../plan/MapGyver_Implementation_Plan.md).

**State: M00 complete locally; M01 native feasibility in progress.**
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
| M01 Native feasibility | IN_PROGRESS | [Native evidence](mapgyver-m01-native-feasibility.md): Intel authored-card 1080p/4K accuracy compares every pixel through independent processes and the existing source-worker bridge; local controls match. Intel/NVIDIA editor continuity retains earlier fingerprints. Procedural cards, NVIDIA accuracy, previews, GPU critical path, retirement/latency and other display/input configurations stay open |
| M02 Multi-app seams/shell | NOT_STARTED | Existing lifecycle/workbench extension points identified; product leaks documented in plan |
| M03 Rendering/channels | NOT_STARTED | D3D11 recommended; SDK binding, host presentation and runtime contracts require implementation |
| M04 Source media | NOT_STARTED | Solid Color, Picture, concurrent Spout scope defined |
| M05 Screens/Surfaces/editors | NOT_STARTED | Output/crop/pin/calibration contracts defined |
| M06 Composition/editor | NOT_STARTED | Instance/canvas/dependency contracts defined |
| M07 Matrix/editor | NOT_STARTED | Layers/groups/exports/transition contracts defined |
| M08 App lifetimes | NOT_STARTED | Neighbor demand and owned-process lifecycle specified |
| M09 App/artifact qualification | NOT_STARTED | Requires implemented product and fixed native workloads |

## Next action

The user requested an explanation of M01's duration and instructed the agent to avoid further
drift on 2026-10-28. New captures, consumer-control cohorts, overlay investigation, import-cache
optimization, internal SDK/driver attribution and additional profiling are stopped. The scope
review checkpoint changed no source and made no native run. The bounded source correction
below is needed under either scope and does not adopt the proposal. The 4K display is already connected;
its availability and the user-confirmed overlay-Off setting are not unanswered questions.

**Proposed scope correction — awaiting user approval, not an accepted gate change:**

| Milestone | Proposed responsibility |
| --- | --- |
| M01 | Record functional feasibility and a host/backend/binding choice from the existing native evidence: ordinary Tauri/Wry underlay, D3D11, pinned Spout binding, two GPU sources, composition, editor independence and scoped ownership cleanup. Explicitly retain limitations and failed qualification results. |
| M02 | Proceed with the thin app shell and public multi-app/host seams after that functional decision. This does not approve production editors or claim qualified rendering. |
| M03 | Own reusable renderer scheduling, measured GPU/CPU budgets, resource bounds and applicable recovery/retirement fixes before accepting those mechanisms. |
| M05 | Own correct physical output placement, preview alignment, quantitative input/layout latency and the full display/DPI/input matrix before accepting those features. |
| M09 and owning features | Run the complete applicable sustained/recovery qualification against the actual app; preserve the fixed Q01/Q02 budgets and historical failures. Full product/Matrix Q05 remains here and with the features that introduce its behavior. |

This proposal changes the plan's explicit M01→M02 dependency, so it requires the user's
decision before implementation resumes under it. The original gate remains open at **1/8**;
it is not retroactively passed. If the user retains the original gate, the current result is
**not passed**. Agree a bounded corrective batch and a pass/fail checkpoint before further
native work; a failed checkpoint produces a decision, not another automatic diagnostic loop.

Internal root-cause attribution, mandatory dual-vendor success, exhaustive SDK-internal copy
accounting, physical residency proof and optical scanout research are not additional gates to
creating a thin shell. Unknown observations remain unknown. The fixed required measurements
and budgets are not waived by this distinction. The full native gate still applies until the
proposal is approved and its owning plan/architecture/contract documents are revised together.

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
The scope proposal remains pending, the original eight gates and budgets remain in force,
and M01 remains **1/8**. This source fix does not establish a cadence, geometry or resource pass.

### Historical diagnostic backlog — superseded as an execution queue

The following retains the previous checkpoint's rationale. It is not authorization to resume
experiments while the scope decision above is pending.

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
