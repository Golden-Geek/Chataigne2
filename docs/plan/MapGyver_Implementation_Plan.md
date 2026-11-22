# MapGyver implementation plan

MapGyver is a video mapping app alongside Chataigne. MapGyver owns product authoring and
behavior. Golden owns reusable engine, worker, GPU and workbench mechanisms. Windows x64 is
the existing native rendering target.
See the [architecture](../architecture/mapgyver.md) and [current status](../progress/mapgyver-status.md).

## M19 — Cross-platform delivery

Work on macOS first, then desktop Linux, then Raspberry Pi (armhf and aarch64). Track Windows
ARM64 separately. The existing Windows x64 qualification queue resumes at its recorded return
triggers. The desktop workbench and project format stay shared across all hosts. When the
current platform's remaining qualification requires unavailable hardware, record the missing
checks and return trigger before starting the next platform's dependency-ready work.
Mac and desktop Linux continue their physical qualification when external displays are available.
Begin Raspberry Pi aarch64 with a native ARM64 package and software checks on the matching Debian
userland. The hosted desktop Linux rerun can finish independently. Raspberry Pi OS and HDMI
qualification remain required on Pi hardware.
The armhf package installs and supports headless authoring under emulation. Return to its
desktop and media checks on a Pi running 32-bit Raspberry Pi OS. While that hardware is
unavailable, begin the Windows ARM64 package through its native FFmpeg SDK.

| Gate | Acceptance |
| --- | --- |
| M19/1 macOS workbench | The default build discovers native FFmpeg. On Apple Silicon, the native desktop starts, the bundled workbench connects, and the headless fixture proves authoring, history and Save/New/Load. |
| M19/2 macOS video | Golden provides the GPU renderer and desktop presentation, with live preview and physical Screen output from an authored project. Verify local media, reconnect and an independently runnable macOS bundle. Qualify Apple Silicon and Intel targets. |
| M19/3 Desktop Linux | A native build and install pass the same project-authoring, preview, physical output and recovery workflows on the supported desktop session types. |
| M19/4 Raspberry Pi aarch64 | Native build, launch, media and output pass on a Pi under its supported OS and display stack. |
| M19/5 Raspberry Pi armhf | Native build, launch, media and output pass on 32-bit Raspberry Pi OS under its supported display stack. |
| M19/6 Windows ARM64 | Native build and install with ARM64 dependencies, workbench authoring, media preview, Screen output and recovery. Validate SDK integrations against ARM64 binaries. |

Golden owns GPU execution and host presentation. MapGyver continues to submit its media and
Screen plans through those shared contracts. Spout is a Windows integration. Qualify platform
texture-sharing and NDI support against their native SDKs before advertising these capabilities.

## User feedback delivery cadence — 2026-11-17

- The simple status file above is the user's single progress view. Keep exact gates, evidence,
  return triggers and the execution board in the [agent ledger](../progress/mapgyver-ledger.md).
  Reconcile both before continuing; update both at each delivered milestone.
- Deliver functional **M15 color/Group feedback (12–15) → geometry alignment (M15/9) →
  Video fade-loop (M10/9) → Shared Texture defaults (M15/11) → Video time cropping (M10/10) →
  subtitles (M10/8) → SGF Matrix feedback (M16/7–14) → earlier SGF feedback (M16/1–6) →
  extended Video formats (M10/3) →
  Golden Gradient (M15/10) → NDI Media and Screen Output (M17) → Feedback.md tasks (M18) →
  deferred deep verification → M09**.
  This user revision supersedes older per-feedback full-matrix and acceptance-first ordering.
- For each small feedback slice, finish all owning-layer changes and run one targeted validation
  cycle: formatting, relevant behavior/type/contract checks and a short app smoke when needed.
  Compile the required target once; reuse compiled artifacts and passing unchanged-layer evidence.
- Batch full cross-app regressions, Intel/other-GPU profiles, sustained performance, physical
  combinations, recovery and edge-case matrices later. Do not make them routine prerequisites
  for every UI or parameter tweak. Broaden checks when the actual changed contract warrants it.
- Functional milestones may advance to the next feature with deep qualification explicitly Pending.
  Deferred requirements remain release-blocking and are never represented as Accepted.
- Reproduced ordinary-use crashes, data loss or broken required behavior still interrupt delivery.
  Fix the owning behavior without turning the slice into an unrelated optimization investigation.
- No existing requirement, failure record, resource/timing budget or final release check is waived.
  Commit and push each coherent milestone before the next implementation batch.

This is the single execution entrypoint. The revised proposal is merged here as of
**2026-11-02** and takes precedence over conflicting earlier requirements.
The **2026-11-03 feedback** is incorporated in **M11** below and supersedes conflicting
presentation, source-authoring and resource-capacity requirements. **All M11 gates take
priority over M10 and M09**. Earlier acceptance is evidence for its tested contract; changed
behavior requires M11 acceptance. This planning update claims no implementation or new acceptance.
Implementation is active under the user's MapGyver goal. Planning awards no acceptance.

The user's **2026-11-05 feedback**, supplied with recordings/screenshots dated 2026-11-04,
is incorporated in **M12**. **M12 was prioritized ahead of Video (M10) and final
qualification (M09)**, and supersedes conflicting M06/M07/M08/M11 presentation and authoring
contracts. M11's accepted evidence retains its original tested scope; the newly reported
regressions and changed behavior require M12 acceptance. No implementation is claimed here.

The user's **2026-11-06 feedback** is incorporated in **M13** as fourteen acceptance gates
covering all 22 bullets. M13 supersedes conflicting source defaults/ownership, Matrix controls,
transition authoring, picker and presentation contracts in M04–M12. **All M13 gates precede
Video (M10) and final qualification (M09).** Earlier acceptance retains its tested scope;
this plan update implements nothing and accepts no gate. Preserve the ledger's current active
gate and suspended desktop state; select M13/1 at the next product-delivery boundary after
checkpointing that gate, retaining every unfinished qualification gate and its return trigger.

The user's **2026-11-08 feedback** is incorporated in **M14** as ten acceptance gates.
**Video first pass (M10/1,2,4: common codecs, HAP and bundled integration)
comes first, then M14, then extended formats (M10/3) and remaining acceptance closure.**
This explicit priority overrides the earlier all-feedback-before-Video ordering above and below.
Unrelated M03/9, M08/4, M12/8 and M13 qualification no longer prevent first-pass Video product
delivery; their complete requirements, failures and investigation limits remain in the return
queue and still gate M09. A reproduced correctness, ownership or integrity defect on Video's
actual dependency path must be fixed before building on it. M14 supersedes conflicting older
editor selection/navigation, Composition ordering/enabled, Screen defaults/targets and Matrix
enabled/stop contracts. Earlier evidence retains only its tested scope. Planning accepts no gate.

**User acceptance revision, 2026-11-08:** existing smoothness (M13/4) and existing
Raw/Smooth anti-aliasing qualification (M12/8) are accepted for now. Preserve the failed
numerical measurements as historical limits, without representing them as passing tests.
Do not schedule further qualification of those accepted scopes unless new user feedback or a
reproduced dependency defect warrants it. This supersedes their older return requirements and
closes two fixed gates by explicit user decision. M14/8's new 4x AA implementation and functional
checks remain required; this acceptance does not implement that feature. Continue M10 first
pass, M14 and extended formats in the order above. Other ownership, recovery, correctness,
memory and final-product requirements remain unchanged.

The user's **2026-11-09 Video feedback** is incorporated in **M10/5–8** below.
**High priority: transport Parameters (5), smooth paused/playing seek (6) and Matrix transport
policy (7), all required before first-pass Video acceptance. Low priority: subtitles (8), after
M14 and extended formats, before M09.** Keep M10/4 as the sole active gate and checkpoint its
current work before selecting the new high-priority gates at the next product boundary.
Earlier M10/1–2 evidence retains its tested scope; it does not verify the changed controls or
reported seek regression. This planning amendment adds four pending gates and implements nothing.

**User execution-order revision, 2026-11-14:** implement **M14 next**, after the accepted
M10/1,2,5,6,7 functional contracts. M10/4 remains unfinished and release-blocking, with every
original age, complete GPU-path, shared-consumer, profile/repeated/performance-tier, memory,
retirement and recovery requirement retained. This explicitly supersedes gate-4-before-M14
ordering above and below; it accepts no gate and changes no budget. Return M10/4 at a scheduled
qualification boundary after M14 and M15 or for a reproduced blocker on the active product path,
using its recorded binaries/source identities and the original environment prerequisites.
Keep exactly one gate Active. The 2026-11-16 user priority revision parks noncritical
Chataigne-only Curve/Spatializer and other consumer improvements until Chataigne work resumes;
their findings and local patch are recorded in [the deferred handoff](../progress/chataigne-editor-followups.md).
M14/1 retains MapGyver editor acceptance and the shared contracts required by MapGyver, but
Chataigne-only qualification no longer blocks MapGyver feature delivery. M14/4 zero-scale
Composition and M14/5 enabled metadata are now
[Accepted](../progress/evidence/mapgyver-m14-composition-acceptance-2026-11-16.json), including
corrected shared write/history admission, the full actual-app browser contract and independent
output/preview pixels with resource retirement on both GPUs. On2026-11-16 the user checked Overlay Off and established
standing Off state until they report or authoritative evidence shows a change; do not request
confirmation before each cohort. M14/3 Composition actions/front-first order is now
[Accepted](../progress/evidence/mapgyver-m14-actions-acceptance-2026-11-16.json), including
all-caller actions, bounded paint/hits, both-GPU raster/retirement and the complete seven-type
creation contract after correcting Video status reads across authored-target retirement.
M14/6 allocation and inventory are functionally verified, including both-GPU no-external
desktop checks. Its one/multiple-external physical cases and native shortcut remain
[Blocked with return triggers](../progress/mapgyver-m14-screen-defaults.md): the active native
and WMI catalogs expose only an internal panel, and supported desktop input cannot connect.
M14/7 optional sender names is now [Accepted](../progress/mapgyver-m14-sender-names.md):
typed optional metadata, all-caller/history/copy/reload parity, actual Inspector and both-GPU
SDK discovery/pixels, exact collision/retry and old-owner retirement pass. Return to M14/1's
remaining MapGyver native/independent/lifetime acceptance, retaining every M14/6 requirement
before M09. Chataigne-only consumers stay deferred.
The recorded default-parallel
Video software failures remain required correction/rerun work before M09; sequential passing
evidence does not establish a full parallel-suite pass.
Retain unfinished MapGyver selection/navigation and every other gate in the return queue.
M14/3–5,7 acceptance adds four verified gates; the denominator is unchanged.
Then finish extended formats,
subtitles and retained acceptance before M09, subject to the M15 priority below. The user also
freshly observed native NVIDIA Overlay Off on this date; record that observation with any
actual run, not as qualification.

**Latest feedback and execution-order revision, 2026-11-17:**
[Latest Feedback.md](../../Latest%20Feedback.md) is incorporated in **M15** as eleven fixed
acceptance gates. **All remaining Video work follows this feedback:** M10/3 extended formats,
M10/8 subtitles and retained M10/4 qualification return after M15. The remaining M14 feature
order is **9 → 10 → 8 → 2**; all four are functionally delivered with qualification Pending.
M15/1–4 controls, Matrix selection/colors, appearance and configuration presets are Accepted
for their owning contracts. M15/5 feedback is functionally delivered with native presentation
qualification Pending. M15/6 startup and M15/7 Surface display are Accepted; linked Screen handles8 is next.
Deliver the M15 feedback
before returning to Video and final acceptance closure. Reproduced correctness defects
on the active product path remain prerequisites; the reported parameter-control crash is
M15/1's first correction, not optional optimization or deferred edge-case qualification.
The user's renewed feature priority keeps sustained performance tuning, recovery and unusual
scenario matrices in the second pass. Verify ordinary feature behavior before delivery;
reopen deferred work early only for a reproduced blocker on that behavior. Keep each deferred
requirement and its concrete return action in the status board; do not turn it into another
feature-delivery prerequisite or count it as accepted.
M15 supersedes conflicting Matrix selection/presentation, sender-default, editor-feedback
and gradient ownership contracts. It extends M14's shared editor work without accepting or
removing any unfinished M14 requirement. Existing acceptance retains its original tested
scope; this update adds **eleven gates and one phase**, implements nothing and accepts no gate.

**User priority revision, 2026-11-18:** add four fixed M15 gates (12–15) for custom colors
on Screens/Surfaces/Medias/Folders, Matrix color-menu actions, compact Layer Groups and readable
colored nested Groups. **Keep Screen-handle links (M15/8) as the current task through completion.**
These additions are **next in line afterward: 12 → 13 → 14 → 15**, before remaining
M15/9–11 or Video work. Do not interrupt or park M15/8 to start this feedback.
Keep one gate Active. This supersedes older next-feature ordering, adds no implementation or
acceptance, and preserves every existing requirement and return trigger. M15 now has 15 gates;
overall scope is **121**, with **99 accepted (82%)** and **8/16 phases complete (50%)**.

**User Video amendment and task order, 2026-11-18:** add **M10/9 Video auto-smooth looping**.
After the current color/Group tasks M15/12–15, deliver **geometry alignment (M15/9) →
Video fade-loop (M10/9) → Shared Texture defaults (M15/11) → subtitles (M10/8) →
extended Video formats (M10/3) → Golden Gradient (M15/10)**, then retained qualification
and M09. This supersedes the earlier ordering of M15, subtitles and extended formats.
M15/9 is Accepted. M10/9 is functionally verified, with full qualification
retained, and M15/11 is Accepted. The amendment adds one gate (**121 → 122**). The 104 accepted
gates at the geometry checkpoint count as **85%** across this scope. M10 is **5/9 (56%)** and
phase completion is **8/16 (50%)** before the SGF amendment.

**SGF Feedback priority, 2026-11-18.** Deliver [M16](#m16--deliver-sgf-feedback-after-subtitles)
immediately after M10/8 subtitles and before M10/3 extended formats. M16 adds six Pending
gates and one phase, bringing acceptance at that amendment from
104/122 (85%) to **104/128 (81%)** and phase completion to **8/17 (47%)**.

**User Video time-cropping amendment, 2026-11-18.** Deliver **M10/10** after M15/11 Shared
Texture defaults and immediately before M10/8 subtitles. This adds one Pending gate within M10.
Overall acceptance is **104/129 (81%)**, M10 is **5/10 (50%)**, and phase completion is **8/17 (47%)**.

**New SGF Matrix feedback, 2026-11-18.** Deliver **M16/7–14** after the subtitle functional
checkpoint, followed by **M16/1–6**, starting with audio output and routing. This adds eight gates
and brings the plan to **137 gates** across **17 phases**.

**NDI amendment, 2026-11-19.** Add [M17](#m17--deliver-ndi-media-and-screen-output) for NDI
Media and NDI Screen Output, both with audio. Queue it after the current extended Video work and
before remaining acceptance closure and M09. M17 added two pending gates, bringing the plan to
**139 gates across 18 phases** at that checkpoint.

**Resume order, 2026-11-19.** Start M17/1 while M10/3 extended Video qualification is deferred.
M17 builds on the delivered Shared Texture and virtual audio routing paths. M10/3 remains in the
qualification queue for M09.

**Feedback.md amendment, 2026-11-19.** Deliver [M18](#m18--deliver-feedback-after-ndi) in six
separate tasks after M17 Media and Screen Output feature delivery and before the deferred
qualification pass. The six pending gates bring the plan to **145 gates across 19 phases**.

## Entry, order and delivery

1. Read AGENTS.md, this plan and the compact status page. Reconcile the checkout, current
   shared Golden work and existing evidence. Preserve unrelated edits and completed work.
2. Use branch `mapgyver` and `origin/mapgyver`. Commit and push each coherent milestone
   after relevant checks and status updates, under the standing repository authorization.
   Do not force-push, release or deploy.
3. Continue the selected gate in the status board. Record its concrete deliverable, owner,
   proving check and stopping time in the status page's single replaceable batch entry.
   Default to 45-minute slices; investigations have the AGENTS.md 60-minute/two-experiment
   cumulative limit. Finish or checkpoint the same gate at cutoff rather than expanding scope.
4. Execute the revision in this order: **M04 ownership/Resolution/Reference → M04 Media Preview
   and M05 navigation/Screen interaction → M06 Composition → M08 reusable app-control boundary
   and M07 Matrix redesign together → delivered M11/M12/M13 functional contracts →
   M10 functional Video (1 → 2 → 5 → 6 → 7, accepted) →
   remaining M14 features (9 → 10 → 8 → 2) →
   current M15/8 Screen-handle links → M15 next color/Group feedback (12 → 13 → 14 → 15) →
   M15/9 geometry alignment → M10/9 Video fade-loop → M15/11 Shared Texture defaults →
   M10/10 Video time cropping → M10/8 subtitles → M16/7–14 SGF Matrix feedback →
   M16/1–6 earlier SGF feedback → M10/3 extended formats (DXV and MKV Video, still WebP Picture) →
   M15/10 Golden Gradient → M17 NDI Media and Screen Output → M18/1–6 Feedback.md →
   remaining acceptance closure → M09 final qualification**.
   M08's shared configuration/runtime contract precedes dependent Matrix app controls; this
   dependency does not authorize two active gates. Deliver their coherent slices sequentially.
   M10/1,2,5,6,7 are accepted for their tested scopes. The 2026-11-14 user revision selects
   M14 now while M10/4 qualification stays in the explicit return queue and blocks M09.
   The 2026-11-17 revision puts M15 before every remaining M10 gate; it preserves the
   user-deferred M14/1 and blocked M14/6 return requirements for acceptance closure before M09.
   M13/4's existing failed numerical evidence remains historical under the user's acceptance
   above; do not requalify that scope without new feedback or a reproduced dependency defect.
   Retain every unfinished M12/M13 and qualification gate; this priority change awards no acceptance.
   M08/4 retains its [blocked complete-retirement qualification](../progress/mapgyver-m08-retirement.md),
   alongside the named M03/M05 return lanes. M08's functional boundary and ordinary-app integration
   are verified; independent M11 stability/editor work does not depend on unverified external-target
   cleanup. This 2026-11-04 ordering change uses rule 6 below and accepts or waives no gate.
   A reproduced defect blocking the selected product path still takes precedence. Do not start
   M09 while M08/4 or any M11–M18 gate remains unaccepted. The 2026-11-08 Video priority
   supersedes the former M10 prerequisite here. The 2026-11-05 user priority
   moves unfinished M05/8 qualification to Pending with its existing evidence and exact return
   action; it does not waive that gate or restart its investigation allowance. That historical
   M12/1 selection is complete; the current ledger and M14 amendment above govern continuation.
   Finish or hand off any already-running check before changing work.
5. The unrestricted finite Surface-warp correction is already delivered and manually accepted.
   Preserve it and its evidence; do not restart that correction.
6. Functional dependencies permit product implementation before unrelated physical/timing
   qualification. Retain M03/M05 blockers with concrete return triggers. A reproduced correctness,
   ownership or integrity defect blocking the selected path must be fixed before building on it.
   Apply AGENTS.md's product/diagnostic allocation during feature delivery; switch to its
   acceptance-closure rules once completing the implemented revised scope.

M04/1–8 are now accepted, including the retained source qualification returned to after
Reference and Preview delivery. [M04/6 and the complete phase audit](../progress/mapgyver-m04-source-parity.md)
close source caller/Picture/project/resource parity. [M06/1&#39;s child-media model](../progress/mapgyver-m06-model.md)
now passes the complete degree/structural/API/Parameter/history/project contract.
[M06/2 dependency/sharing](../progress/mapgyver-m06-dependencies.md) also passes all-media cycles,
missing/Undo, deep/large graphs and compiled production sharing. The five-producer/six-alias
software proof satisfied the retained return trigger. [M06/3 native Composition](../progress/mapgyver-m06-native.md)
now passes all nine original stages on both GPUs, including the active black preview canvas,
with unchanged scenes/quantization/error tolerance/budgets. [M06/4&#39;s anchored editor](../progress/mapgyver-m06-editor.md)
also passes actual native resize/rotation/cursor, pointer/history/zoom and tab lifetime.
[M06/5&#39;s independent panels](../progress/mapgyver-m06-panels.md) now pass bounded selected lookup,
persisted follow/lock/page choices, actual reconnect/project/tab/native lifetime and the current
generic Inspector/File Open/New regressions. M06 is complete. [M08/2 configuration/demand](../progress/mapgyver-m08-configuration.md#configuration-and-demand-acceptance--2026-11-03)
is now accepted, including typed Column/Cell settings, standalone removal, normalized sharing,
independent activation and retained demand through disabled rendering, interruption, history,
project replacement and final-use completion. [M07/1 model acceptance](../progress/mapgyver-m07-model.md#m071-acceptance--2026-11-03)
is complete: typed Layer/Group/Column/Cell ownership, backend 3×3 defaults, actual child media,
bounded Inspector and complete authored API/history/project/copy behavior.
[M07/2 playback acceptance](../progress/mapgyver-m07-playback.md) is also complete: precedence,
independent/atomic activation, disabled/empty policy, interruption and shared-time/final-use
demand pass, with current native fade observations on both GPUs. [M07/3 grid
acceptance](../progress/mapgyver-m07-grid.md) is complete: actual contextual creation/removal,
inline rename, structural copy/delete and virtualized drag sorting/reparenting, bounded shared
Inspector/keyboard/resize/project lifetime and truthful native submission/retirement feedback pass.
[M07/4 native/scale/lifetime acceptance](../progress/mapgyver-m07-native.md) completes M07 at 4/4:
both GPUs pass the complete current raster, source recovery/sharing, sparse bursts/reload,
interruption and final-retirement matrix. App Control lifecycle requirements now avoid per-owner
whole-document snapshots during bulk typed creation. [M08/3 actual runtime acceptance](../progress/mapgyver-m08-runtime.md)
now verifies both native readiness modes, shared owned processes, cancellation, completion-based
retention and stale-project rejection. [M08 ordinary-app integration](../progress/mapgyver-m08-app-integration.md)
also verifies picker, commands, scripts/callbacks and project/recovery/shutdown. M08/4 remains
Blocked on [complete singleton retirement](../progress/mapgyver-m08-retirement.md); remaining M14
features and M15 own the next product delivery before all remaining Video. Retained physical/timing
qualification and the original Matrix fixture budgets are unchanged.

[Typed Layer/Group settings](../progress/mapgyver-m07-model.md) use real Parameters and
the generic Inspector, with atomic Configure/history/copy/project behavior. Layer position,
canvas and duration now drive actual rendering and per-Layer playback; native position/resize/
Undo passes on both GPUs. Cell v3 now owns typed relative position/opacity/enabled with no size;
its verification is recorded in the accepted model report. **M12/7 supersedes the no-Cell-size
contract with typed Cell placement size.** The accepted M07/1 fixture
starts with three Layers and three Columns, sparse empty addresses and four paired outputs in
one backend tree; M11/8 changes default exposed outputs while preserving internal compositing.
Catalog creation, exact Undo/Redo/reload and whole-Matrix copies pass; queued reads after deletion
return typed absence and recover on Undo. Full grid/playback/native/scale gates remain unchanged.

M04/7's default Reference/Alt-copy drops established actual Composition/Matrix child-media
containment before Reference/Preview acceptance. Reuse that ownership/copy/drop foundation.
M06/M07 retain their complete transform, editor, playback, scale and native acceptance;
this dependency work does not accept those later gates.

M04/7 verifies the Reference contract below through typed callers, bundled app interaction and
the production native renderer. Whole-app Q04 qualification remains in M09, including M06's
bounded Composition providers and M07's revised grid/active-demand workloads. Reference's 10k
backend receipt fixture supplements its functional evidence; the existing whole-app budgets,
larger authored counts, UI, memory and output requirements remain required in their owning gates.

The unqualified phrase “implement the plan” still selects Chataigne under AGENTS.md.

M05/10 is [accepted](../progress/evidence/mapgyver-workbench-status-2026-11-03.json): responsive
bounded Screen/Surface row supplements and native runtime footer are verified. M05/6–7 are now
[accepted](../progress/evidence/mapgyver-native-zoom-sync-2026-11-03.json): native image/decoration
paint, an internal Source landmark, output-accepted moving-frame hits and projected DOM events
are verified alongside the complete bundled editor input/history/lifetime suites. A verified
owned-foreground desktop capture route supplies native pixels; moving-hit metadata is not a
physical latency measurement. M05/9 [stable output-window bounds](../progress/evidence/mapgyver-output-bounds-2026-11-03.json)
is also accepted: one visible HWND through move/resize/Undo and both actual display targets,
with pending allocation fenced and completion-safe backing replacement. M05/4 [concurrent
fullscreen/rectangle/Spout](../progress/evidence/mapgyver-concurrent-output-2026-11-03.json) now
passes actual editor close, OS-confirmed minimize/restore and changed physical/Spout pixels.
M04/4 [Spout source acceptance](../progress/mapgyver-m04-spout.md) also passes full lifecycle,
cross-adapter recovery and RGBA pixels on both GPUs, plus declared input capacity/retirement.
[M04/5 source UI acceptance](../progress/mapgyver-m04-source-ui.md) passes the complete retained
interaction matrix through typed Channels and generic Inspector controls, including fresh
permissions, safe deleted-channel reads and context-menu history. [M04/6 source parity](../progress/mapgyver-m04-source-parity.md)
also passes revised callers, all-source copies/history/project and the complete both-adapter
native Picture recovery/recreation matrix, completing M04. M05/5,8 remain
in the physical return queue; every original matrix and acceptance requirement remains unchanged.

Historical batches, pauses and execution instructions in the
[evidence archive](../progress/mapgyver-evidence-through-2026-11-02.md) are non-operative.

## Product contracts

- Use the thin app and Golden desktop/headless host, generic Inspector, shared selection,
  persisted manager folders, history, scripting, transport and persistence.
- Every authored setting is a typed Parameter child. Collections use real child nodes.
  DTOs project these values; UI stores never become a second authored document.
- Rust owns control/protocol types and generated TypeScript. Change DTOs, generation and all
  consumers together. No pixels or native handles travel through persisted/control data.
- Ordinary parameters use the generic Inspector. Specialized editors fill their panels,
  use compact accessible icon actions, and send explicit backend intents. The backend owns
  defaults, identity, validation, geometry, structural edits and playback/app policy.
- Every module slice includes command nodes, script functions, callbacks and app-owned snippets.
  Validate UI/script/headless parity, rejection, grouped Undo/Redo, copy and project roundtrip.
- Keep demand, decode/preparation and GPU work bounded and off the engine/UI thread. Consumers
  share rendered targets and decoder ownership. Endpoint integrations need recovery/backoff,
  cancellation, project/device fencing and completion-based final-use retirement.
- No compatibility layer is planned. Version changed document structures explicitly and reject
  unsupported old records clearly; never reinterpret old records silently.
- M10/5–10 own the Video feedback; M15 owns the latest feedback contract and explicitly
  supersedes conflicting M04–M14 behavior. Unchanged M13/M14 requirements remain binding.
  Authored/playback controls are real Golden nodes and
  Parameters, including triggers and optional-Parameter metadata. Interaction feedback must
  remain smooth at the display refresh rate independently of media/output frame cadence.

## Milestones and dependencies

IDs remain stable. Accepted M10 functional gates **1,2,5,6,7 → remaining M14 features →
M15/12–15 → M15/9 → M10/9 → M15/11 → M10/10 → M10/8 → M16/7–14 → M16/1–6 → M10/3 → M15/10 →
M17 → M18/1–6 → retained acceptance closure → M09** is the current execution order.
M10/4 remains unfinished and release-blocking, with return in retained acceptance closure.
Earlier evidence retains its tested scope. M16 owns the new SGF contracts.
Counts refer to the numbered acceptance gates below, not commits or elapsed work.

| ID  | Outcome                                                           | Functional dependency                                                      | Gates |
| --- | ----------------------------------------------------------------- | -------------------------------------------------------------------------- | ----- |
| M00 | Reconciled baseline and qualification workloads                   | Explicit product selection                                                 | 1     |
| M01 | Functional native host/backend/binding decision                   | M00                                                                        | 4     |
| M02 | Public multi-app seams and runnable thin shell                    | M01 functional decision                                                    | 7     |
| M03 | Reusable rendering, channels, time, assets and preview            | M02                                                                        | 9     |
| M04 | Source variants, Resolution, Reference and Media Preview          | M03 gates 1–3, 5–8                                                         | 8     |
| M05 | Screens, Surfaces, navigation and physical output                 | M04 working revised contracts                                              | 10    |
| M06 | Child-media Composition and transform editor                      | M04 and M05 working contracts                                              | 5     |
| M07 | Child-media Matrix, playback and virtualized grid                 | M06; M08 configuration for app controls                                    | 4     |
| M08 | Reusable optional app control and shared runtime demand           | Existing Matrix playback; revised M04 ownership                            | 4     |
| M11 | Feedback: stable rendering, simpler authoring, shared UX          | Existing M04–M08 product contracts; one gate at a time                     | 10    |
| M12 | Feedback: stable panels, Matrix groups/layout, polish             | Existing M04–M11 contracts; one gate at a time                             | 11    |
| M13 | Latest feedback: smooth interaction, Matrix controls, media UX    | Existing M04–M12 functional contracts; one gate at a time                  | 14    |
| M10 | Tiered Video playback, transport, seeking, fade-loop, time cropping and subtitles | Functional 1,2,5,6,7 accepted; remaining features follow the 2026-11-18 task order | 10    |
| M14 | Shared 2D interaction, Composition, Screen and Matrix feedback    | Accepted M10 functional gates 1,2,5–7; retained qualification stays queued | 10    |
| M15 | Latest feedback: controls, colors/Groups, presets, editors, Gradient | Public Golden contracts; 12–15, then 9; 11 and 10 interleave with Video as ordered above | 15    |
| M16 | SGF Feedback: Matrix transport and UI, audio, OSCQuery and editor fixes | M10/8 subtitles → M16/7–14 → M16/1–6                                | 14    |
| M17 | NDI Media and Screen Output with audio                            | M04/M05 Shared Texture contracts and M16/1 functional audio routing         | 2     |
| M18 | File drops, Project Settings, File Explorer, Shader Media, Inspector selection and browser previews | M17 feature delivery, then M18/1–6 in order | 6 |
| M09 | Actual-app qualification and Windows artifact                     | All retained feature/qualification gates, including M10–M18                | 1     |

### M00 — Reconcile and define evidence

**Gate 1:** retain the reconciled checkout/public-boundary inventory, exact available native
routes and machine/toolchain/OS/WebView/GPU/driver/display identities, fixed workloads and
budgets. Preserve Chataigne's root-default launch. Evidence:
[baseline](../progress/mapgyver-m00-baseline.md) and
[qualification contract](../progress/mapgyver-qualification-contract.md).

### M01 — Decide the functional native rendering route

Retain the user-approved functional scope and decision: ordinary Tauri/Wry native underlay,
D3D11 and pinned `spout2-sys =0.1.1`. Full performance/display qualification stays in M03/M05/M09.

1. Observed ordinary underlay clipping, layering and DOM/native input on recorded configurations.
2. Caller-owned D3D11 devices and two differently sized GPU-only Spout inputs compose to
   native and Spout outputs; binding/SDK ownership is audited.
3. Scoped resize/restart, editor/output independence and owned-resource/process cleanup smoke.
4. Recorded host/backend/binding decision, limitations and owners of continuing qualification.

Retain the [functional decision](../progress/mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28),
the [native research](../architecture/mapgyver-native-rendering.md) and the approved
[NVIDIA overlay prerequisite](../progress/mapgyver-qualification-contract.md#nvidia-overlay-prerequisite).
This remains four functional gates, not a pass of the original broad native matrix.

### M02 — Establish the second app without product leaks

The shared host owns bootstrap, native windows and COM coordination. App-specific resources,
identity and lifecycle registration stay in MapGyver. Never copy Chataigne's private modules.

1. Public UI-bundling API adopted by Chataigne.
2. Thin MapGyver host/UI scaffold, identity, project specifications and resources.
3. App branding and browser-state isolation for layout, selection, preferences and recent files.
4. Generic generated discovery contracts and endpoint isolation.
5. Explicit app-selected Cargo/npm/xtask/watch/package commands; Chataigne remains the root default.
6. Optional typed native-presentation host lifecycle extension.
7. Independent/concurrent bundled/dev launches and headless/engine/protocol public-boundary checks.

Retain the [shell evidence](../progress/mapgyver-m02-shell.md); native quality is a separate gate.

### M03 — Deliver render/resource and channel foundations

**Manual qualification priority (2026-11-06):** while the user is present, finish physical
acceptance before returning to autonomous capture work. M05/5 hotplug/live arrangement is
accepted. M03/4 source/device recovery and completion-safe blocked-work retirement are now
accepted; M03/9 production qualification remains open. M05/8 translucent Source-menu reference
passes on both GPUs, with broader reference/timing scope retained. The user has returned to
autonomous work and explicitly authorizes needed development resets, while retaining the
acceptance-first order at that time. The 2026-11-08 amendment now prioritizes first-pass Video;
all original requirements and M12/M08 prerequisites for final qualification remain unchanged.

Golden Video owns app-neutral immutable render plans, scheduling, opaque resources and native
execution. MapGyver projects its authored media into guarded plans. Reuse Golden Time and Assets.

1. Deterministic DAG/demand/resource-budget compilation, cycle/resize/rejection and explicit
   texture/color/alpha contracts.
2. Worker runtime/null path, atomic install and project/revision/device fences; bounded desired,
   prepared, in-flight and retiring work with completion-based resource ownership.
3. Exercised D3D11/Spout compositing/conversion and shared-host native presentation; explicit
   caller-device/adapter ownership.
4. Production source/output observations, device-removal/recreation, isolated healthy paths,
   cancellation and bounded recovery/retirement.
5. Stable media/channel IDs and typed references, authoritative backend size/default operations,
   generated contracts, history/reload/resource-recreation identity.
6. Shared monotonic production-time and activation/discontinuity authority.
7. Persistent asset IDs/locators, worker preparation/cancellation, relocation/relink and
   stale-generation/resource admission.
8. Reusable preview registration/geometry and generated control contracts exercised natively.
9. Production Q01/Q02 CPU/GPU/cadence/frame-age/admission/memory/handle/shutdown and applicable
   recovery qualification against the retained contract.

Gates 4 and 9 remain open. Existing clock/timing proposals are unapproved; no budget or physical
requirement is waived. Use [render foundations](../progress/mapgyver-m03-render-foundations.md)
and the [qualification contract](../progress/mapgyver-qualification-contract.md), not a new probe chain.

### M04 — Deliver source variants, Reference media and Media Preview

**A. Media/source ownership (gates 1–6).** Channels are stable concurrent output variants in a
media's **Channels** section. Hide channel nodes from the Medias outliner and remove
channel creation from manager `+` actions. Edit variants as real typed child nodes. Shared
settings belong to the media; variant differences are Solid Color color, Shared Texture sender,
Picture asset and Video file. Composition initially has one output; channel retargeting is future work.

Every media has a disableable **Resolution** typed Vec2 in pixels, disabled by default, with
integer constraints, unit steps and backend bounds **1–16384 per axis**. Disabled resolves to
source-native extent or **1920×1080** for procedural canvases. Enabled forces every channel to
that extent. Under M11/4, applicable media use one shared **Fit Mode: Fit / Fill / Stretch**
when Resolution is enabled; no per-channel authored size. Hide Fit Mode for automatic size and
omit it for Solid Color. Remove authored Kind/SourceKind and generic Filter/Sampling controls;
source kinds remain typed backend identity and sampling defaults to Linear. Keep each channel's
actual source difference directly editable in expanded, compact Inspector rows.
M11/5 adds media-level Manual/Auto Shared Texture selection with typed per-channel fields.
Remove authored Shared Texture format, encoding, alpha and maximum dimensions:
discover format/encoding at the native boundary, use straight alpha, and retain internal
allocation limits and recovery. Remove useless `+` actions from declared Parameter folders
through their container capabilities.
M13/1–2 extend sender defaults and automatic format handling; M13/9 replaces Picture's
indirect asset-authoring workflow with self-contained File Parameters. M17/1 uses Shared Texture
as the reference for NDI Media authoring, previews and receiver lifecycle.

1. Default Medias, detached creation/import, authoritative identity/defaults and complete
   command/function/callback/snippet API under the revised media/variant ownership.
2. Concurrent variants with distinct native source sizes, disabled/enabled media Resolution,
   shared consumers and stable channels through resize/reorder/history/reload.
3. Worker Picture decode/color/alpha and source/pixel/decode/cache bounds; preserve applicable
   native Picture evidence and bounded upload/cancellation/retirement.
4. Exact independent Spout sender selection/discovery, runtime metadata, absence/restart/rename/
   resize, isolated failure and explicit adapter mismatch without fallback.
5. Actual manager/Channels/generic Inspector interaction, typed field ownership and folder
   capabilities; no ordinary custom forms or channel creation in the manager.
6. Complete revised UI/script/headless parity, missing/relinked/replaced Picture behavior,
   duplication/Undo/Redo/project roundtrip and functional native resource recreation.

**B. Reference media (gate 7).** A typed target reference mirrors target channels/content,
using target channel identities for synchronization. It owns no source settings, transport or
decoder. Default drops into Composition/Matrix create Reference children; **Alt+drag** copies
the entire media subtree with independent identities. Share target rendering; an explicitly
enabled Reference Resolution adds a shared-source resampling pass.
M13/7 separately defines existing-Cell drag operations; it does not replace this external-media
drop contract. M13/14 supersedes conflicting Reference target/channel selection and all-channel
mirroring assumptions with the media-first, explicit-channel contract.

7. Reference channel add/remove/reorder synchronization, missing/recovered targets, mixed
   Reference/Composition/Matrix cycle rejection, shared rendering/decoder ownership, resampling,
   default-drop/Alt-copy semantics and full Parameter/API/history/project parity.

**C. Media Preview (gate 8).** Add a dockable panel that follows selected media or descendants,
with channel choice/all-channel grid and independently persisted lock/channel/panel state for
multiple instances. Remove Inspector media previews. Reuse native leases/retirement.

8. Actual independent preview instances, selection-follow/lock/grid, reconnect/project fencing,
   close/lock retirement and uninterrupted independent outputs; bundled/native interaction.

### M05 — Deliver Screens, Surfaces and shared navigation

Retain crop followed by perspective warp, calibration, ordered alpha composition, fullscreen/
physical rectangle/virtual Spout outputs and display identity/recovery. Finite invalid quads
remain editable with red edges/dim red fill; omit only unrenderable layers and recover when valid.
Invalid crops reject atomically. Screen/Surface settings stay in generic Inspector.
M13/5 adds an initial Screen to every fresh project; M13/13 specifies editor backgrounds and
M13/14 simplifies Surface source targeting. M13/4 extends interaction acceptance to high-refresh
displays without replacing M05/8's original physical/latency requirements.
M17/2 uses the Shared Texture target as the reference for an NDI Screen Output with routed audio.

Golden's reusable viewport animates zoom toward the requested scale around a stable pointer
anchor. Screen, Surface Source and Composition share it. Native imagery, overlays and hit testing
stay aligned during animation; disposal/document replacement cancels obsolete work.

1. Atomic Screen/Surface authoring, stable routing/targets/crops/quads, default manager and
   complete module API/history/project lifetime.
2. Reference-pattern crop/warp correctness, finite invalid intermediate editing and safe recovery.
3. Calibration without a source restores routing; overlapping Surfaces preserve order/alpha.
4. All three target modes work concurrently and continue while editors close/minimize.
5. Display identity survives hotplug, mixed DPI and negative origins.
6. Screen editor: actual Surface/shared Inspector selection, background deselection and editor-local
   individual handle selection. Under M11/6, unselected Surfaces have only a thin border;
   selected Surfaces alone have a light overlay, slightly thicker border and handles. Arrow keys move
   handles **1 output pixel**, **10 with Shift**; held repeat is one Undo gesture. **Alt+drag**
   translates all four corners. Backend **Fill Screen** stretches the quad; **Fit to Screen**
   centers it with source aspect preserved. Verify pointer/keyboard input and smooth zoom alignment.
7. Surface Source crop/mapping/calibration, full-source native imagery, smooth zoom, selection,
   grouped history and public-caller parity.
8. Retained native physical matrix: exact output/preview geometry, alpha/color/reference patterns,
   100/125/150/200% and mixed DPI, negative origins, overlays/capture, measured ≤1-physical-pixel
   settled alignment and contract input latency. Unavailable configurations stay open.
9. In-place native output bounds updates for rectangle position/size/display reassignment:
   output stays visible with the same window identity and completion-safe GPU resize/retirement.
10. Responsive Screen outliner supplements for output/target, resolution and FPS; Surface source
    media supplements, plus a public Golden footer contribution hook for actual FPS, active GPU,
    frame time and resource usage. Use bounded runtime observations rather than frame-rate engine
    polling; M10 adds Video observations through the same hook.

### M06 — Deliver child-media Composition and editor

Canvas items become actual child media nodes, replacing reference-only instance authoring.
Default drop inserts a Reference; Alt+drag copies independently. Transform Parameters belong
to those children: placement, scale, pivot, rotation, opacity and blend. Rotation is authored
and publicly edited in **degrees**, converted only at the rendering boundary.

1. Single-output Composition, ordered owned media children, typed transforms and structural
   copy/reorder/removal; complete API/Parameter/Undo/Redo/versioned project behavior.
2. All-media dependency/cycle validation, missing target/deletion/Undo, nested/deep/large graphs
   and shared target rendering without source/decoder duplication.
3. Correct native nested/shared-source output, canvas resize, alpha/blends, pivots, signed scale
   and degree conversion, with one render per demanded output.
4. Backend-authoritative selection/geometry/history; corner resize fixes the opposite corner,
   edge resize acts on one axis, outside-corner rotation uses a rotation cursor. Remove the
   anchor-adjacent rotation control. Verify actual pointer input, smooth zoom and grouped history.
5. Multiple independent Composition panels follow selection or lock per panel, restore state
   and survive project/tab lifetime. Remove internal headers and permanent creation/refresh bars;
   M11/6 places relevant actions beside shared navigation and M11/7 adds contextual child creation.
   M12/6 adds last-relevant-content retention and the shared tab lock; M12/7 extends the spatial
   editor to Matrix without merging their domain rules.

### M07 — Deliver child-media Matrix, playback and grid

New Matrices have **three layers and three columns**. Use a full-panel virtualized square-cell
grid, clearer layer/group separation and distinct selection/requested/actual runtime markers.
M12/5 replaces the navigation toolbar with trailing creation headers and adaptive grid sizing;
M12/4 adds Column Groups, distinct from rendered Layer Groups.
Remove the internal header and custom property forms. Selected offscreen objects remain
editable through bounded backend inspection independent of visible rows.

| Owner  | Typed authored settings                                                                                                                               |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Layer  | Position, size (layer canvas), opacity, blend, transition duration, **Expose Channel (default false)**; retain enabling/group behavior          |
| Column | Optional transition override, optional App file and conditional app-control settings, including integer pre/post                                      |
| Cell   | Position and placement size relative to Layer (M12/7), opacity, enabled, optional transition override, optional App file and conditional app controls |
| Group  | Typed opacity/blend/enabling, **Expose Channel (default false)** and actual hierarchy/order; a rendered blend owner, not just a folder          |

Populated Cells own actual child media; empty addresses stay sparse until content/settings need
a Cell node. Cell media retains its own source size; M12/7 adds independent placement size.
Transition precedence is **Cell override → Column
override → Layer duration**; retain the **250 ms** default. Disabled Cells leave active layer
content unchanged. Enabled empty Cells retain transparent-transition policy. Explicit individual
Cell triggering does not change the current Column anchor; Column launches remain atomic.
M13/3,6–7,11 supersede conflicting grid order/selection, trigger/stop, Cell-drop and transition
override contracts below. The 250 ms default and existing precedence remain unless explicitly
changed there; a separate override boolean is replaced by transition-time Parameter metadata.

1. Revised stable sparse document/derived layer/group/final identities, grouping and actual
   child media; 3×3 defaults, default Reference/Alt-copy, complete typed API/history/project behavior.
2. Deterministic transition precedence, independent Cell triggers, atomic Column launches,
   disabled/empty policies, interrupted blends and final-use outgoing demand under shared time.
3. Actual virtualized-grid interaction: drag sort/reparent Layers/Groups, sort Columns, double-click
   rename, subtle empty-cell affordance with hover/focus `+`/context Add Media and hover/focus `X`
   removal under M12/5; shared
   Inspector survives virtualization. Complete caller parity and truthful native playback feedback.
4. Native layer/group/final pixels, ordering/blend/alpha/interruptions, source loss/Undo/recovery,
   shared consumers, bounded sparse large-grid plans, lifetime and owner retirement under revised behavior.

M11/8 separately verifies opt-in Layer/Group output exposure and media snapshot/name feedback
in Cells. Existing default-output/model evidence does not prove those changed contracts.

### M08 — Deliver reusable optional app control

Remove the **Applications panel**, standalone authoring workflow and obsolete association UI.
Keep a reusable MapGyver app-control system on Golden's generic owned-process primitive,
available to future media; sequences remain outside scope. Columns/Cells have an optional
executable **App file**; show other controls only when populated. Arguments, environment and
other collections are typed child nodes.

Share one owned runtime for identical normalized executable plus launch/control settings.
Combine demand across Cells, Columns and Matrices. Cell pre/post distances are relative to its
Column position, clamped and non-wrapping. Direct Cell activation and outgoing transitions retain
demand. Preserve readiness, timeout, restart/backoff, cancellation, project fencing, graceful/
forced stop, tree cleanup and final-render-use retention. Never adopt/kill unrelated processes by name.
M12/3 changes fresh app-control Window defaults to Normal and verifies repeated zero-pre/post
entry/exit recovery. M12/4 extends this same configuration/demand boundary to Column Groups.
M13/10 removes the App Control runtime section from the Inspector while retaining lifecycle
supervision and truthful runtime outcomes outside ordinary authored settings.

1. Retain the public app-neutral Golden IO primitive: explicit executable/arguments/environment/
   directory, suspended launch/Job Object before resume, cancellation/drop safety, truthful
   root/tree/unsupported/access outcomes and bounded process retirement.
2. Reusable MapGyver configuration and runtime-key boundary, typed conditional Column/Cell settings,
   removal of standalone Applications/associations, aggregate normalized demand and neighbor
   semantics through jumps/reorder/delete/direct activation; fake-time/process and history/project checks.
3. Actual owned-process readiness/first-frame, timeout/crash/backoff, shared-demand deduplication
   without transient restarts, launch cancellation, final-render retention and stale-project rejection.
4. Complete UI/script/headless commands/callbacks/snippets and actual app picker/lifecycle,
   singleton/external/elevated/launcher/breakaway outcomes, project/shutdown/recovery/cleanup.
   Retain unverified retirement/access-denial cases; authored Undo is distinct from OS side effects.

### M11 — Resolve the 2026-11-03 feedback before Video and final qualification

MapGyver owns media, Matrix, Screen/Surface and test-pattern policy under `apps/mapgyver`.
Golden owns reusable GPU admission/adapter/preview mechanisms, generic Inspector presentation
and shared viewport/workbench controls. App registration supplies branding and product behavior;
do not put MapGyver-specific policy or assets in Golden packages.

Source: the user's **MG Feedback.txt** (23 top-level bullets), reviewed with `flashes.mp4` (2.03 seconds,
852×954, 30 fps), `bad view item.png`, `Screenshot 2026-11-02 152703.png` and
`Screenshot 2026-11-02 154226.png`. The video illustrates repeated Inspector/channel expansion
and transient redraws; it does not establish a root cause. The captures show the long display
device path, existing edge-positioned navigation and the desired Golden graph toolbar reference.
The supplied root `mapgyver_icon.png` and `apps/mapgyver/ui/src/lib/icons/nodes/*.svg` are the
asset sources. These are reported defects/design references, not passing runtime evidence.

Each numbered gate requires its complete listed behavior and affected caller/history/project
and lifetime checks; do not award partial acceptance. Execute in order, in bounded coherent
slices, with one active gate. M11/4 establishes Fit Mode for current applicable sources and the
contract M10 Video will consume. M11 never requires implementing Video ahead of M10.

1. **Stable interface and preview transitions.** Owner: shared Golden workbench/Inspector and
   native-preview/host lifecycle, with app integration. Reproduce the supplied interaction on
   a recorded build before choosing the fix. Clicking controls, expanding/collapsing Inspector
   folders and clicking Matrix Cells must not flash unrelated UI. Media selection changes must
   not tear down/reload the panel or show a transient blank/loading flash; preserve valid native
   content until its replacement is ready and fence stale selections/project generations.
   Verify repeated recorded real-input/native-frame sequences, independent live outputs,
   rapid selection, tab close/reopen, reconnect and New/Load. A DOM-only test cannot close this
   gate. Prior permission for a brief unavailable preview on restore does not waive these defects.
2. **Useful concurrent 4K capacity with bounded ownership.** Owner: Golden Video resource
   accounting/admission/native allocation and MapGyver demand projection. Reproduce the reported
   4096×4096 Solid Color rejection: charge **138481672 B**, reserved **405631384 B**, limit
   **536870912 B**. Determine real texture/pool costs, shared-target reuse, live/prepared/retiring
   overlap and release before changing policy. Replace an arbitrary fixed 512 MiB product ceiling
   with adapter-aware admission and explicit safe headroom; do not hide allocation failures,
   remove accounting or duplicate resources per consumer. The planned discrete-GPU fixture is
   **at least eight independently demanded 4096×4096 Solid Color outputs**, plus Reference aliases
   and live replacement, with exact formats/counts/steady and peak reservations recorded.
   Verify native content, sustained bounded memory, overload rejection/recovery and final-use
   retirement. Also exercise the integrated adapter against its reported safe capacity; record
   hardware limits explicitly. This new capacity requirement supersedes the fixed allocation
   ceiling only; timing, private-growth and retirement budgets remain required.
3. **Discrete GPU by default, explicit user choice.** Owner: reusable Golden adapter enumeration/
   selection and shared desktop host, registered through MapGyver options. Prefer a suitable
   NVIDIA/AMD high-performance GPU over an integrated adapter on hybrid/Optimus laptops.
   Expose a persisted typed application option for automatic preference or an explicit available
   adapter, report the actual active GPU and any restart requirement, and handle missing selected
   devices/recovery truthfully. Respect explicit choice; never silently move a shared-texture
   connection across incompatible adapters. Verify cold starts, both available GPUs, persisted
   selection, unavailable preference and recovery; controls must agree with the rendering device.
4. **Minimal typed media/channel Inspector.** Owner: MapGyver media Params/authoring and Golden
   generic Inspector hooks. Remove user-facing Kind/SourceKind and generic Filter/Sampling
   Parameters, keep backend kinds typed and default to Linear. Use expanded compact channel rows:
   one Color per Solid Color channel, one File per Picture channel, and one File per future Video
   channel; ordinary edits must not require opening nested Parameters folders. Preserve real
   child nodes, labels, CRUD, bounded inspection/virtualization and shared selection. No authored
   per-channel size. Add a typed media-level **Fit Mode (Fit / Fill / Stretch)** only where source
   sizes can differ; hide it while Resolution is automatic and omit it for Solid Color. Verify
   different-aspect Picture channels in all modes against native pixels and a single forced
   Resolution; Auto preserves source-native extents. All time-related Parameters across existing
   media, transitions and app control use Golden's existing time-style UI and preserve units.
   M10 must apply the same style to Video. Cover generic/API/script/headless/history/copy/project
   parity; change versioned schemas, codegen and consumers together where contracts change.
5. **Shared Texture Manual/Auto discovery.** Owner: MapGyver typed media/channel authoring and
   source supervision; Golden supplies live sender discovery and reusable Parameter adornments.
   A media-level Manual/Auto setting chooses the channel control. Manual uses a String Parameter
   with inline **Detect**, showing a live sender list and setting that same String on selection.
   Auto uses only a dynamic enum Parameter populated from available senders, retaining a missing
   selected value through absence and automatically recovering when the exact sender returns.
   Sender-list changes update open controls without resetting authored choice. Verify multiple
   channels, appearance/disappearance/restart/rename, mode changes, isolated failure, adapter
   mismatch, history/project roundtrip and equivalent script/headless edits. Discovery and
   recovery stay off the engine/UI thread; do not implement reconnect policy in UI stores.
6. **One workbench canvas/navigation experience.** Owner: shared Golden viewport/graph/workbench
   controls; MapGyver registers contextual actions. Remove every extra internal header from
   Matrix, Composition, Surface and Screen panels and remove Refresh View. Reuse Golden graph's
   panel/view navigation, manipulation mechanisms and shortcuts wherever applicable. If graph
   domain assumptions prevent whole-view reuse, extract/reuse its public toolbar/navigation/
   shortcut components; do not build another lookalike. Put selection-dependent icon actions
   at the **top center, on the same line as navigation/zoom**, following the supplied graph
   toolbar reference. Preserve editor-specific domain gestures, keyboard focus, smooth anchored
   zoom, native/DOM alignment, reduced motion and pointer/history lifetime. In Screen editor,
   unselected Surfaces show a thin border with **no blue fill**; only selected Surfaces show a
   lighter overlay, slightly thicker border and handles. Hover must not reintroduce selected
   decoration. Verify all four panels and Golden graph, multiple selections, narrow panels,
   native composition and the retained invalid-quad indication without covering unrelated areas.
7. **Create owned media directly in Composition.** Owner: MapGyver backend catalog/child-media
   creation intents and Composition editor context menu. Right-clicking the canvas offers every
   catalog media type supported by Composition, creating an actual owned media child with
   backend defaults, placement, identity and selection. This coexists with default Reference/
   Alt-copy drops; it must work without a pre-existing item in Medias. Reuse atomic detached
   trees, cycle validation and the public authoring boundary. Verify actual menus, native output,
   complete module commands/functions/callbacks/snippets, script/headless callers, single-gesture
   Undo/Redo, copy/reorder/delete and project roundtrip; unavailable types have truthful states.
8. **Matrix source feedback and real Group outputs.** Owner: MapGyver Matrix Parameters,
   authoring/rendering/catalog and virtualized grid. Populated Cells show a media preview snapshot
   and label **`media [channel]`**, rather than just the channel. Reuse bounded preview leases/
   snapshots for visible Cells; no per-cell source/decoder duplication or frame-rate engine
   polling. Layers have a typed **Expose Channel** option, **off by default**. Groups are actual
   compositing owners with typed **Blend Mode** and the same opt-in channel exposure, not folders.
   Existing blend fields must be wired/verified rather than duplicated. Backend output discovery,
   References and demand honor exposure; internal Layer/Group composition and Final output
   continue without exposure. Verify nested Group blend/alpha pixels, default and toggled
   discovery, stable identities/history, existing/missing Reference recovery, copy/projects and
   large sparse viewport/source-loss/preview retirement. Do not accept a UI-only channel filter.
9. **Consistent identity, tabs, outliners and footer.** Owner: MapGyver registration/assets/UI
   contributions and public Golden workbench hooks. Use the supplied node SVGs for Media rows
   and corresponding panel tabs. Generate app/window/taskbar/installer/package icon variants
   from root `mapgyver_icon.png`, including the test-pattern asset; replace placeholder MapGyver
   branding. Rename **Screen Manager → Screens** and **Media Manager → Medias**. Screen rows show
   the display's human-readable name, never its long internal ID/path; retain stable backend IDs
   and truthful missing-target state. Matrix, Composition, Surface and Screen tab titles follow
   the currently displayed object as Inspector does, respecting locked/independent panels,
   rename, selection and document lifetime. Footer statistics align **right**, contain no `...`
   or `submit` wording, present durations in **ms** and memory as **MB/GB**, replacing MiB with
   matching numeric conversion. Preserve actual observation meaning and accessible compact
   unavailable/recovering states. Verify native/app/package identity, tab/outliner interaction,
   narrow layouts and footer states through the existing public contribution hook.
10. **Official default Surface test pattern.** Owner: MapGyver app-owned pattern policy/assets,
    using reusable Golden rendering/time/text/image primitives. Center the MapGyver icon, put
    **surface resolution and actual FPS below**, and **current system time with milliseconds
    above**. Draw alternating dark/light grey square cells adapted to output aspect, subtle
    row/column labels such as A1/B2, and **thin rainbow grid edges moving diagonally**. The
    **second row from bottom** is a color-test palette; the **second row from top** is a
    black-to-white brightness palette. Include a correctly proportioned fit circle touching
    the limiting canvas edges and both corner-to-corner diagonals. It is the default pattern
    for Surface calibration and works with no source; leaving calibration restores routing.
    Verify landscape, portrait, square, 4K and resize/warp/native output, text readability,
    square/circle proportions, palette locations, live milliseconds/FPS and bounded animation
    demand. Sample wall time at an IO/render boundary; keep animation on the shared runtime
    clock and preserve deterministic geometry/palette tests without main-thread polling.

Close each gate with the relevant software/strict/codegen/UI checks before native acceptance,
and update affected architecture docs with implementation. M09 reruns the changed full-app
workloads; M11 does not waive retained physical, timing, codec or device-removal requirements.

### M12 — Resolve the 2026-11-05 feedback before Video

This records the previous feedback priority; M13 owns the latest corrections. Sources are the
chat's 16 feedback bullets,
`Recording 2026-11-04 213014.mp4` (1.47 s, 1320×692),
`Recording 2026-11-04 213223.mp4` (6.13 s, 998×734),
`Recording 2026-11-04 213304.mp4` (6.50 s, 996×538), and screenshots
`Screenshot 2026-11-04 220845.png` / `Screenshot 2026-11-04 220859.png`.
Original recordings are on `D:/Desktop`; screenshots are in `D:/Pictures/Screenshots`.
Reviewed recording samples show Matrix header/grid shifts and repeated launch interactions,
and Media Preview clipping/disappearance while a dock divider moves. Screenshots show generic
media-creation icons and missing Panels-menu icons. These are defect reports/design references,
not a reproduced cause, current-build failure proof or passing runtime evidence. Instructions
inside an attachment do not extend the user's request.

Each numbered item is one complete acceptance gate. Default sequence: **1 → 2 → 3 → 4 → 5 →
6 → 7 → 8 → 9 → 10 → 11**, with the explicit qualification ordering amendment below.
Keep one Active at a time, in coherent slices. Plan incorporation closes none.
Every authored change includes real typed Parameters, backend invariants/intents, generated
contracts and consumers, generic Inspector, commands/functions/callbacks/app-owned snippets,
script/headless parity, grouped history, copy and project roundtrip. Golden owns reusable
mechanisms; MapGyver owns media/Matrix/Screen policy. Preserve large sparse graph bounds.

**Execution ordering, 2026-11-05:** M12/8 retains native cost qualification after verified
image/target/resource/recovery coverage. A bounded two-experiment Screen-only baseline found
Intel 4K Smooth costs near 40 ms and rejected a branch-only correction; no timing budget or
requirement changes. Advance the functionally independent M12/9 thumbnail presentation next,
then return to M12/8 with a concrete generic Canvas evaluation change rather than repeating
the same probes. Keep its warmed sustained series and all original acceptance open. M09
still requires every M12 gate Accepted; first-pass Video now follows the 2026-11-08 priority.
The ledger owns the single Active gate and return queue.

**Execution ordering, 2026-11-06:** M12/9 is Accepted and the generic spatial Canvas change
improves M12/8's short Intel 4K Smooth median to 27.30 ms. The exact optimized case's original
30-second warmup / three 120-second samples still fails: GPU p95 28.27–28.35 ms versus 12 ms,
with complete timing coverage and definite submission misses. Preserve that series and keep
M12/8 unaccepted. Advance planned M12/10 typography, whose destination-appropriate text path
changes this calibration workload; return immediately afterward to M12/8 on the qualified
implementation. This is an ordering change, not evidence that text alone caused the failure.
All original budgets, other sustained cohorts and all-gates prerequisites remain required.
Do not repeat unchanged probes or infer a pass from delivered quality/resource behavior.

**Execution ordering after the M12/10 return:** typography is Accepted, but its exact warmed
Intel 4K Smooth retest still measures GPU p95 27.95–28.17 ms and fails cadence. A static read
of the embedded shader does not support unconditional bitmap-font initialization; stop that
hypothesis without another GPU probe. Keep M12/8 unaccepted and finish independent M12/11
menu icons while a precise functional-feature versus production-timing gate split is proposed
to the user. That proposal changes no requirement until explicitly approved. Return to M12/8
on the scope answer or a justified owning renderer redesign addressing the reproduced cost;
all other sustained cohorts, original budgets and M09 prerequisites remain required.

After M12/11 acceptance, return to M12/8's still-required canonical cost matrix while the
scope answer is pending. The known Intel 4K Smooth failure remains unaccepted; measuring
the remaining independent cases changes neither the original gate nor the proposed split.
Reuse the exact cost executable only after its selected execution-input hashes still match;
menu-only UI changes are outside that headless fixture's executed path.

After the complete eight-case sustained matrix (2026-11-06), seven cases pass and Intel
4K Smooth still fails its original GPU/cadence budgets. M12/8 stays unaccepted, with return
on the pending explicit feature/production scope answer or an owning renderer correction
meeting those unchanged limits. Do not repeat the measured cases or extend the rejected
shader investigation. Resume the retained M05/8 native animation/alignment queue while
that blocker remains; preserve every remaining DPI/input/physical requirement. This changes
execution order only: M09 still requires all M12 gates and retained M08/4 acceptance.
The later 2026-11-08 amendment prioritizes first-pass Video before unrelated qualification.

1. **Recorded glitches, flashes, panel reloads and Media Preview resizing.** Owners: Golden
   docking/Inspector/host/native-preview lifetime and MapGyver panel adapters. Reproduce the
   three supplied interactions on a recorded current build before choosing a fix. Cover Matrix
   selection, rename/launch markers, Inspector edits and repeated dock-divider resize, including
   a locked Media Preview. Keep panel/component identities, drafts, scroll and valid preview
   leases stable through background reads and layout changes. Native clipping, aspect/viewport,
   occlusion and DOM alignment must update together; no displaced/stale image, accidental crop,
   unrelated flash or panel remount. Do not guess the defect's cause or mask it with reloads.
   Require real-input frame sequences covering the supplied interactions, narrow/large panels,
   independent output continuity, tab close/reopen, reconnect and New/Load on both available
   GPUs after software prerequisites. Reuse M11/1 evidence only for unchanged tested behavior.
2. **Standard Inspectors and one Surface enabled state.** Owners: MapGyver Surface/Matrix
   authoring and public Golden Inspector contracts. Remove the redundant Surface `Enabled`
   Parameter; node enabled metadata is the sole authored enable control and must drive routing,
   rendering, scripts and headless operations. No UI-only hiding or second persisted boolean.
   Matrix, Column/Column Group, Layer/Layer Group, Cell and child-media selection use the
   standard Golden Inspector for ordinary settings. Remove custom explanatory/runtime text
   blocks (including the current Matrix runtime prose) and duplicate property forms. Preserve
   necessary actual/requested/error feedback as compact grid/status cues, with accessible
   details on demand. Verify standard editing, mixed/offscreen selection, Undo/copy/reload and
   all callers; selected definitions remain bounded and independent of visible rows.
3. **Reliable Matrix app re-entry and visible launch defaults.** Owners: MapGyver app-control
   demand/supervision and Golden owned-process lifecycle. Fresh Column, Cell and later Column
   Group controls default **Window = Normal**, never Hidden; an explicitly authored Hidden
   choice still works. Reproduce repeated Column entry/exit with **preload = postload = 0**,
   without changing the fixture to hide the failure. Resolve stale starting/stopping ownership,
   cancellation/readiness/backoff or retirement at the responsible boundary; every eligible
   re-entry must start/reuse its process and reach readiness again. Require at least 20 cycles,
   rapid exit/re-entry while stopping, normal/explicit-hidden windows, shared and independent
   controls, crash/endpoint loss/recovery, delete/Undo, New/Load and shutdown with truthful
   process/output state. Keep all M08/4 retirement requirements and its named blocker visible;
   ordinary-app recovery may proceed independently, and does not accept that older gate.
4. **Real Column Groups with aggregate App Control.** Owner: MapGyver Matrix document,
   authoring/playback and shared app-control demand. Add typed Column Group nodes containing
   ordered Columns (and nested groups through the same hierarchy), distinct from compositing
   Layer Groups. They are actual persisted structure, not UI-only header bands or duplicated
   Column lists. A group becomes active when any contained Column is active; switching between
   its Columns keeps group demand continuously active. Give it the same optional App file and
   conditional typed controls, contributing through the existing normalized shared runtime.
   Playback is still addressed to stable Columns; grouping preserves atomic launches, flattened
   Column order/pre-post distances and independent Cell-trigger semantics. Retain group app
   demand for outgoing final use, then release it once no child needs it. Specify requested,
   actual and stopping markers separately. Verify group/ungroup, nested groups, move/reorder,
   copy/delete/Undo/reload, same-group switching without process restart, cross-group release,
   shared executable ownership, rapid re-entry and bounded sparse large-Matrix behavior.
5. **Full-panel Matrix grid with quiet contextual actions.** Owners: MapGyver virtualized grid
   and public Golden menu/icon controls. Remove Matrix **Frame, Home and Zoom** navigation and
   the navigation toolbar entirely; Matrix remains a scrolling table. Add a trailing **`+`
   column header** offering **Column / Column Group**, and a trailing **`+` layer row/header**
   offering **Layer / Layer Group**, with insertion into the selected hierarchy resolved by
   backend intents. Empty/zero-row/zero-column Matrices retain both creation entrypoints.
   Small grids use the available panel area and adapt to visible layer/column counts; preserve
   readable minimum cell/header sizes in relative units, then scroll in either axis rather
   than shrinking indefinitely. Virtualization must not materialize every sparse Cell.
   Empty Cells show a subtle empty-state icon; reveal **`+` on hover or keyboard focus**.
   Removal uses **`X`**, revealed on hover/focus, with accessible names, tooltips and context-menu
   equivalents. Replace bin/trash delete icons throughout MapGyver's app-visible actions,
   including contributed generic menus, through public icon/action hooks; leave unrelated apps'
   policy independent. Verify resize, overflow, groups, drag sort/reparent, inline rename,
   keyboard actions, offscreen Inspector and sparse 1000×1000 lifetime without layout flashes.
6. **Last relevant content and a shared tab lock.** Owners: Golden workbench panel state/tab
   actions and MapGyver content resolution. An unlocked Composition panel follows a newly
   selected Composition or its relevant descendants, but **retains the last Composition when
   selection moves elsewhere**; unrelated selection must not empty or reload it. Apply the same
   relevant-target rule to the shared Matrix spatial adapter below. A lock freezes the displayed
   content even when a different eligible target is selected. Put the lock/unlock icon **in the
   tab, immediately left of Close**, only for selection-driven dynamic panels that support
   binding to content (Inspector, Media Preview and eligible editors); no lock on Logger,
   Dashboard or fixed manager panels. Remove duplicate body locks. Use one public capability
   and per-instance persisted document-scoped target/lock state, with truthful title/rename,
   keyboard/accessibility and existing independent-panel behavior. Verify unrelated/eligible
   selection, two panels with different locks, delete/Undo, layout restore, reconnect, New/Load
   and cross-project fencing; retained target identity must never become a stale document copy.
7. **One spatial editor for Composition and Matrix layout.** Recommended decision: share the
   **Composition panel's viewport, navigation, selection and manipulation shell**, with focused
   Composition and Matrix adapters; retain the separate Matrix grid for playback/structure.
   Both domains need the same native canvas/handles/history lifetime; separate full panels
   would duplicate those controls. Keep their backend geometry and queries distinct rather
   than converting Matrix to Composition or growing one orchestration object.
   Keep the existing Composition panel registration; its tab identifies the displayed target
   and layout level. Unlocked selection of a Matrix, Layer or Cell resolves the Matrix adapter;
   selecting unrelated objects retains the last spatial target, and locking prevents switching.
   The Matrix adapter offers whole-Matrix Layer layout and per-Layer Cell-content layout,
   with compact context/breadcrumb navigation; it edits **Layer position/size and Cell
   position/size**, including inactive authored Cells without triggering playback. Cell size is
   typed placement geometry relative to its Layer, independent of source-media Resolution;
   backend defaults preserve natural source size until an explicit placement size is authored.
   Define Fit/Fill/Stretch within that placement through typed backend policy and the standard
   Inspector; this is distinct from thumbnail presentation. Supersede M07's previous no-size
   rule explicitly in schemas/DTOs/render projection. Validate bounds/coordinates, opposite-
   anchor resize and grouped pointer/keyboard history at the backend boundary. Queries return
   a bounded visible page plus an independent selected object, including offscreen selections.
   Verify both domains, selection/lock modes, active/inactive Cells, native pixels, nested Layer
   Groups, aspect/alpha/blend, source resizing, history/copy/reload and large sparse projects.
8. **Raw or smoothed final rendering: proposal and acceptance.** Existing qualification is
   accepted for now by the user's 2026-11-08 decision above; retained failed measurements are
   historical limits, not a passing numerical claim. Recommended initial contract:
   a typed per-Screen **Anti-aliasing: Raw / Smooth (2×)** in the standard Inspector. **Raw is
   default** and preserves current rendering without a smoothing pass; it does not change
   media sampling to nearest-neighbor. Smooth supersamples final Screen composition/warp at
   twice the output width/height, then resolves once with linear-light, alpha-correct filtering
   to the exact output extent, covering physical and Spout targets without duplicating source
   producers. This addresses shader-generated edges as well as warped boundaries; a blanket
   post-blur/FXAA pass risks softening the sharp test-pattern text requested below. MSAA alone
   is not assumed to fix shader-generated edges in the current fullscreen-triangle renderer;
   see Microsoft's [rasterization rules](https://learn.microsoft.com/en-us/windows/win32/direct3d11/d3d10-graphics-programming-guide-rasterizer-stage-rules).
   This is a proposal, not measured performance or user approval of a new budget. Record the
   chosen approach before implementation. Account for four times the intermediate pixel count,
   adapter texture limits and all live/retiring allocations; no silent fallback or relaxed
   M03/M11 budgets. Verify Raw unchanged, smooth diagonal/warp/circle coverage, transparent
   edges without halos, text/palette sharpness, target parity, live switching/history/projects,
   both GPUs, 1080p/4K costs and overload/recovery. UI icons/thumbnail fit are separate concerns.
9. **Aspect-correct Matrix thumbnails.** Owners: MapGyver viewport presentation and reusable
   native tile placement. Fit each source thumbnail inside its Cell with aspect preserved and
   deliberate empty margins; **never stretch by default**. This is viewport presentation,
   independent of media Fit Mode, Cell placement size and final rendering. Verify portrait,
   landscape and square sources, live source-size changes, resize/scroll/virtualized reuse,
   group headers, labels, alpha and independent output pixels. Retain bounded leases and
   shared sources rather than adding one decoder/texture producer per Cell.
10. **Sharp, larger calibration typography.** Owners: MapGyver app-owned test-pattern layout/
    font assets and public Golden text rendering. Increase clock, resolution and FPS text size
    and use a clean bundled font with redistribution rights; keep minor cell labels subordinate.
    Render glyphs at destination-appropriate resolution (for example a cached scalable atlas or
    vector coverage), avoiding upscaled low-resolution bitmap text. Keep text crisp in Raw mode;
    final anti-aliasing must not be required to conceal blurry typography. Preserve all M11/10
    geometry, branding, palette and clock/FPS contracts. Verify native text at 100% output scale,
    landscape/portrait/square through 4K, resize/perspective warp, both quality modes when
    available, live values, font packaging/license and bounded off-thread preparation/resources.
11. **Correct icons in Panels and Media Add menus.** Owners: MapGyver registrations/assets and
    Golden public panel/catalog menu hooks. Every Panels-menu entry shows its registered panel
    icon, including generic panels; preserve the same identity as its tab. Every Media Add
    entry, including Composition's contextual menu, uses the same correct type icon as the
    Medias outliner, rather than the generic blue placeholder shown in `220845.png`. Reuse the
    existing app-owned SVG registry and public contributions instead of another icon mapping.
    Verify all entries/submenus, folder/reference/media variants, theme/scale, keyboard labels
    and menu/tab/outliner parity in browser and actual app against both supplied screenshots.

Close each gate only after its relevant Rust/strict/generated-contract/UI checks and the named
native behavior pass. Update owning architecture docs with implementation. **All eleven M12
gates are prerequisites for M09**; the 2026-11-08 amendment removes the former first-pass Video
prerequisite. M08/4 and retained physical/performance/device gates
remain required. The new scope adds eleven gates and one phase without awarding acceptance.

### M13 — Resolve the 2026-11-06 feedback before Video

Source: the user's 22 feedback bullets in this chat. Reported black RGBA Float textures,
limit-layout scrollbar glitches and roughly 15–20 Hz manipulation on a 240 Hz laptop display
are defect reports, not diagnosed causes or measured current-build evidence. This milestone
defines the required corrections; plan incorporation closes none of its fourteen gates.

Default product sequence is **1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12 → 13 → 14**.
Use one Active gate and bounded reviewable slices. Preserve current M05/8 software evidence,
its suspended desktop state, cumulative investigation limits and exact return action when
selecting M13/1 at the next product boundary. M03/9, M08/4 and M12/8 remain required, with
their existing blockers; this feedback authorizes no timing-budget reduction or gate split.

Golden owns reusable Parameter/trigger metadata, Inspector controls, manager selection,
filtered tree/picker capabilities, pending-operation feedback and viewport/native scheduling
mechanisms. MapGyver owns sender defaults, media/Matrix/Screen policy and backend operations.
Every authored/playback change includes real typed nodes/Parameters, backend validation and
intents, generated contracts/consumers, generic Inspector, commands/functions/callbacks and
app-owned snippets. Verify UI/script/transport/headless parity, atomic rejection, grouped
Undo/Redo, copy and project roundtrip where relevant. Preserve sparse/virtualized bounds,
independent selected-object inspection, endpoint recovery and completion-safe resource retirement.

1. **Shared Texture defaults and None label.** Owner: MapGyver source authoring/supervision,
   using Golden's live sender discovery. Rename Auto mode's **Unconfigured → None**. Fresh
   Shared Texture media default to **Auto** and select the **first available sender on creation**
   through the backend creation operation, without a follow-up UI edit. The first entry shown
   and the backend's selected sender must agree. Use None when no sender is available or it is
   explicitly chosen; preserve explicit/persisted choices and exact-sender recovery through
   discovery changes. Verify zero/one/multiple senders, all creation callers, mode changes,
   loss/reappearance, Undo/Redo and reload without resetting existing authored selections.
2. **Automatic Shared Texture/Spout format support.** Owners: Golden native texture import,
   format/conversion boundary and MapGyver source supervision. Automatically handle all
   available sender texture formats, including the reported **RGBA Float** case; do not require
   format settings or silently restrict reception to an 8-bit shortlist. Inventory every format
   exposed by the actual SDK/backend and its import/conversion path. Validate independent
   native pattern pixels, color/alpha and appropriate precision, including fractional/float
   content, both GPUs, live format/size changes and sender/device loss/recovery. A black image
   is a failure, not a Ready result. Explicit hardware/API limitations remain named unaccepted
   requirements; never hide them behind a fallback or a narrower claim of complete support.
3. **Stable Matrix layout, selection and front-to-back order.** Owners: MapGyver grid adapters,
   typed Layer settings/render order and reusable Golden scrolling controls. Reproduce and fix
   scrollbar glitches around limiting panel sizes, both-axis overflow, exact-fit thresholds,
   dock resize and grouped headers. Selection works across the **whole Layer and Column UI**,
   with embedded controls retaining their intended actions. Put Layer labels **top left**.
   Add compact inline **Opacity** and **Blend Mode** controls editing the same real Parameters
   as the Inspector. The **front Layer is first/top; the back Layer is last/bottom**, with
   hierarchy/reorder and backend compositing agreeing for all callers. Verify narrow/large/
   empty grids, repeated threshold crossings, keyboard input, inline controls, offscreen
   selection, nested groups, history/reload, native overlapping pixels and sparse 1000×1000
   lifetime; no oscillating scrollbars, jitter, selection loss or whole-grid materialization.
4. **Smooth, immediate interaction throughout the app.** Accepted for now by the user's
   2026-11-08 decision above; no further timing/alignment qualification is scheduled for this
   existing scope without new feedback or a reproduced dependency defect. Owners: Golden input/viewport,
   workbench/native-preview scheduling and pending-operation UI; MapGyver editor adapters.
   Cover Surface Editor, Screen Editor, Composition transforms and Matrix gestures, then
   shared scrolling, docking and ordinary controls. A **simple populated 3×3 Matrix** is a
   required regression fixture, alongside empty scenes and other media workloads. Interaction
   manipulation feedback currently requires **30+ fps**, with **60 fps preferred**, on a display
   supporting it (about 33.33 ms / 16.67 ms per update). The user's later 2026-11-07 revision accepts
   this 30+ fps minimum for now and removes the 240 Hz interface requirement. Neither media/output
   cadence nor engine acknowledgements may impose the reported 10–15 Hz feel. Keep local pointer/
   viewport feedback prompt while authoritative edits are bounded/coalesced with correct final
   state, history and cancellation; no per-gesture whole-document rebuilds or recurring
   millisecond engine/UI polling. Measure real-input presented frame pacing, long stalls and
   pointer-to-visible p95/p99 on the recorded refresh rate, alongside native/DOM alignment,
   independent output continuity, both GPUs and larger projects. Verify actual presented
   manipulation at the revised 30+ fps minimum, including missed updates/stalls; an engine FPS
   counter alone does not prove interaction smoothness. Retain M05's latency bound separately.
   **Display-aware output frame rate** is part of this gate: offer a real typed setting for
   following the selected output display's refresh rate or choosing an explicit frame rate.
   Keep rendering cadence independent of UI manipulation and source arrival. Verify selected
   cadence/pacing, display changes/recovery, both GPUs, persistence and all public callers;
   do not infer pacing from a configured value. Golden owns reusable scheduling/display
   mechanics; MapGyver owns the authored output policy. This revision changes this gate's
   scope without adding acceptance credit or changing the fixed gate denominator.
   **2026-11-07 synchronization regression:** the user's UI manipulation itself is acceptable,
   and live Spout image content is smooth. During direct Screen Warping / Surface Source
   manipulation, the rendered geometry lags/chops behind the smooth editing handles and catches
   up on release. Inspector opacity changes already give smooth video feedback; preserve that.
   Inspector corner edits update video promptly but editing handles lag. Inspector crop
   position/size updates video smoothly while the crop rectangle waits until release. Reproduce
   and verify both directions of parameter/video/editor synchronization continuously during
   input, including final release/cancellation. Passing overlay-only or source-FPS checks cannot
   accept this gate. The supplied 30 fps recording is symptom evidence, not an authoritative
   live update-rate measurement. Keep latency bounds: a higher average FPS cannot excuse
   geometry/handles trailing input or waiting for release.
   **Applying changes** appears only when a genuinely outstanding operation exceeds a
   documented delay, rather than on every edit or merely because a gesture is long. Coalesce
   overlapping work and stabilize its visible lifetime so corner drags/transforms never flash
   the notification; delayed work still shows truthful progress and failures remain visible.
   Verify fast acknowledgements, continuous drags, deliberately slow work, completion/error,
   reconnect and cancellation. Retain all original physical/performance budgets separately.
5. **A Screen in every fresh project.** Owner: MapGyver project initialization via the public
   Golden lifecycle/document API. Opening the app without a loaded project and **New File**
   each create one actual Screen with backend-owned defaults, ready for normal selection and
   editing. Construct the initial document atomically, without UI-triggered repair or duplicates.
   Loading an existing project preserves its authored Screen count, including an intentionally
   empty project. Verify desktop/headless initialization, repeated New, load/reload, project
   replacement, default routing and output/resource lifetime.
6. **Real Matrix play/stop triggers at every scope.** Owner: MapGyver Matrix nodes, playback
   and app-control demand on Golden's typed Parameter/trigger API. Remove the top-left
   **Installed** dropdown/label. Put a **Stop All** control there, and Stop controls on **Layers,
   Columns, Layer Groups, Column Groups and Cells**. Column Play and independent **Cell Play**
   are actual Golden Trigger Parameter children; all stop controls are actual triggers on their
   owning nodes as well. Every authored/playback control edits or invokes those real nodes,
   including generic Inspector, scripts and transport; no UI-only command state or duplicate
   hidden boolean. Define scope membership and backend stop/transition behavior explicitly:
   Stop All covers every playing Cell; scoped stops cover their members, including independent
   Cell launches, without stopping unrelated scopes. Preserve independent-Cell/Column-anchor
   rules, atomic launches, outgoing final-use retention and aggregate app demand. Verify every
   scope/caller, rapid play/stop/interruption, empty/disabled Cells, nested groups, visible state,
   history/project structure and completion-safe native/process retirement.
7. **Cell swap, move, copy and reference drops.** Owner: MapGyver atomic Matrix structural
   intents; UI supplies source/destination and the chosen operation. Dragging an existing Cell
   onto another Cell **swaps** them; dropping into an empty address moves it. **Alt+drag/drop
   opens a Move / Copy / Reference menu**, rather than immediately copying. Move transfers
   the Cell and clears its old address; Copy clones its settings and owned media subtree with
   independent identities; Reference uses linked source media without duplicating its producer.
   Specify occupied-destination handling before mutation so no content is silently discarded.
   Preserve relevant Cell settings, app-control demand and playback identity through the
   backend's atomic operation; reject cycles/invalid destinations without partial edits.
   This is distinct from dragging an external media into the grid (M04's Reference/Alt-copy).
   Verify populated/empty/same-address, cross-Layer and supported cross-Matrix drops, scrolling,
   keyboard/context equivalents, running Cells, cancellation, copy/reference lifetime,
   single-step Undo/Redo, reload and bounded sparse updates through every caller.
8. **Honest Medias expanders and selection after manager creation.** Owners: Golden public
   filtered-manager tree and creation-result/selection contracts; MapGyver tree registration.
   Leaf media such as Shared Texture have **no disclosure arrow** for hidden Parameters or
   Channels. Matrix and Composition expose their real child hierarchies, as do folders.
   Any node created from a Golden manager menu, including media, is **automatically selected**
   after authoritative creation succeeds, and immediately drives the Inspector. Apply this
   through the reusable manager contract, including nested/context menus, without UI identity
   allocation or selection changes on failed creation. Verify folders/leaf/container media,
   filtered/virtualized trees, all manager menu paths, history/reload and affected Chataigne
   managers. Script/headless creation must not acquire a dependency on a mounted UI.
9. **Self-contained Picture media.** Owner: MapGyver Picture Params/authoring and bounded
   decode resources. Choosing a Picture File edits **that Picture's own File Parameter**
   (per channel where applicable). It must not create another media, an asset/catalog node or
   any other authored object, and must not forward editing through another object's Parameter.
   Remove the implicit asset-import/secondary-authoring workflow for this action, superseding
   M11/4's forwarded asset File behavior. Internal decode/cache resources remain implementation
   details, with sharing/retirement bounds preserved. Verify document topology/count unchanged
   after File select/change/clear/relink, no extra selection, generic Inspector editing,
   independent copies, all callers, Undo/Redo/reload, missing/replaced files and native pixels.
10. **Clean App Control Inspector.** Owner: MapGyver app-control Inspector exposure using
    Golden's public metadata/hooks. Remove the **Runtime section** from the Inspector for
    every App Control owner, including Columns, Cells and Column Groups. Keep only relevant
    authored Parameters and conditional settings there; do not replace the section with
    another runtime prose block. Preserve backend supervision and compact runtime feedback in
    its existing grid/status locations. Verify every owner, generic/mixed/offscreen selection,
    clean empty/configured states and unchanged script/headless lifecycle behavior.
11. **Transition override metadata and Smooth default.** Owners: MapGyver typed transition
    Parameters/playback and Golden optional-Parameter metadata. Remove the separate boolean
    **Transition Override** Parameter; override/inherit is metadata of **Transition Time**
    itself and appears through the generic time control. Preserve **Cell → Column → Layer**
    precedence and the **250 ms** base default. Rename **Smoothstep → Smooth** and make
    Smooth the default transition curve, preserving the underlying smoothstep behavior.
    Change schemas, generation and consumers together, removing obsolete boolean handling.
    Verify enabled/disabled override metadata, zero/nonzero durations, all relevant owners,
    curve defaults, interrupted native fades, caller parity and history/copy/project roundtrip.
12. **Draggable/slidable time floats.** Owner: reusable Golden numeric/time Parameter UI.
    Float Parameters with time-style presentation support the same drag/slide interaction as
    ordinary number controls, while retaining formatted time entry, units, constraints, steps,
    keyboard/accessibility and grouped history. Use the existing real Parameter path across
    media, Matrix transitions and app-control times; M10 consumes this control for Video.
    Verify positive/zero/bounded values, fractional precision, unit formatting, cancellation,
    mixed selection and actual mouse/keyboard interaction in MapGyver and shared consumers.
13. **Grey editor surroundings, content-only black.** Owners: Golden themed viewport/native
    clearing/clipping and MapGyver Screen Warping/Surface Source adapters. Match Composition's
    **grey workbench background outside content bounds** in Screen Warping and Surface Source.
    Only the actual content rectangle is black or displays its content; do not clear the whole
    panel to black. Keep bounds, native imagery, overlays and hit testing aligned through
    pan/zoom/resize, missing sources, alpha, theme/scale, docking and project transitions.
    Verify real desktop pixels and clipping on both GPUs without changing output pixels.
14. **Media-first target and Channel dropdown.** Owners: MapGyver Surface/Reference typed
    targeting and channel resolution; Golden public filtered reference-picker/query controls.
    Use **one Media target**, followed by a dependent **Channel dropdown**; selecting a media
    automatically chooses its **first/default channel** through backend policy. Do not require
    two hierarchy selections or expose channel nodes in the target picker. Apply the same
    contract to Surface sources and Reference media; the channel chooses the referenced output,
    superseding conflicting all-channel Reference assumptions in M04/7. Preserve channel
    identity through rename/reorder, retain a valid explicit choice, and define truthful
    empty/deleted-channel recovery without silently rerouting an existing explicit binding.
    Root the picker at **Medias**, showing only its catalog media and organizing folders.
    Exclude unrelated document nodes and media nested inside Composition, Matrix or other media;
    the full document hierarchy must never be the browse experience. Keep queries bounded,
    ancestry/type validation in the backend and generic Inspector editing of the same typed
    Parameters. Explicit nested references created by Cell operations remain subject to their
    own backend validity/cycle rules; displaying a current binding must not reopen nested media
    as picker choices. Verify one/multiple/no channels, folders, exclusions, references,
    missing/recovered targets, source/Reference chains and cycles, all callers, copy/history/
    reload, large catalogs and native routing without duplicate rendering/decoding.

Close each gate only after its complete behavioral checks and relevant formatting, Rust tests,
strict lint, generated-contract/UI checks and named native interaction/pixel validation pass.
Inspect compile/software results before desktop/hardware work. Update owning architecture
docs with implementation, including Picture ownership and Reference channel semantics.
**All fourteen M13 gates remain prerequisites for M09.** The 2026-11-08 amendment prioritizes
first-pass Video before unfinished M13 qualification. This adds fourteen gates and
one phase, preserving every older unfinished acceptance requirement and numeric budget.

### M10 — Deliver Video playback in codec tiers

**2026-11-08 priority:** deliver **gates 1 → 2 → 4** as the first pass before M14; defer
**gate 3 (extended formats)** until after M14. Gate 4 qualifies the complete first-pass codec set
independently of gate 3. Gate 3 subsequently includes bundled integration/recovery/performance
for its added Video formats under the same gate-4 contracts. **2026-11-09 amendment:** add high-priority
gates **5 → 6 → 7** before gate-4 acceptance; complete low-priority gate **8** after M14 and
gate 3, before M09. Preserve the current M10/4 checkpoint and sole-active-gate discipline;
select gate 5 at the next product boundary, retaining gate 4's exact return requirements.
**Current override, 2026-11-17:** functional gates 1,2,5,6,7 are accepted; all remaining
Video work (3,8 and retained 4) follows M15. The older first-pass-before-M14 wording above
does not select the next gate. Preserve every bundled/runtime, recovery and performance
requirement; a reproduced correctness dependency on the active feature path still takes precedence.
**Video order override, 2026-11-18:** after M15/12–15, deliver M15/9 → M10/9 → M15/11 →
M10/10 → M10/8 → M16/7–14 → M16/1–6 → M10/3 → M15/10. See the current execution order above
for work after M15/10.
Video time cropping precedes subtitles. M10 has ten gates. Fade-loop and time cropping are
functionally verified with full qualification Pending.
Use existing verified functional contracts; retain unrelated open qualification in its explicit
return queue.
Video has **one transport per media and one file per channel**; References only mirror outputs.
Reuse M11's simplified Channels, shared Fit Mode,
draggable time-style Parameters, icons and stable-preview contracts, with M13's self-contained
File ownership and simplified media/channel targeting rather than restoring obsolete controls.
Use a shared monotonic media timeline and timestamp-based sampling. Defaults: paused at zero
unless **Play at Load** is enabled, **Speed** 1 and looping off. Shorter channels hold their
final frame until the shared timeline ends. M10/5 replaces the older editable Playing, Seek To
and Rate controls; Matrix activation follows M10/7 rather than a UI-owned transport policy.
Frame stepping uses the selected channel's timestamps; References never independently reset transport.

Use FFmpeg libraries through reliable Rust bindings for demuxing/broad decode and D3D11 hardware
frames where supported. Select/pin the actual bindings and redistributable build before coding
that boundary. Keep hardware frames GPU-resident. HAP performance playback uses the reference
decoder's compressed-texture route. Primary integration sources:
[FFmpeg codecs](https://ffmpeg.org/ffmpeg-codecs.html),
[FFmpeg D3D11 hardware contexts](https://github.com/FFmpeg/FFmpeg/blob/master/libavutil/hwcontext_d3d11va.h)
and [HAP developer guidance](https://hap.video/developers).
Library capability is not acceptance of any codec/profile in MapGyver.

1. **Transport/common codecs:** H.264, H.265 and ProRes; play/pause/stop/seek/frame-step/speed/loop;
   mixed-rate timestamp sampling, shorter-file hold and shared Reference transport. Complete
   Parameters, commands/functions/callbacks/snippets and UI/script/headless/history/project parity.
2. **Performance playback:** HAP, HAP Alpha, HAP Q, concurrent high-resolution files and shared
   consumers; compressed textures, alpha/color correctness and one decoder/render owner.
3. **Extended format compatibility.** Support DXV and Matroska (`.mkv`) Video playback
   with selectable embedded subtitle and audio tracks. As clarified on 2026-11-19, the
   WebP requirement is still-image import as **Picture**, including transparency and the normal
   File/asset workflow. Verify decode and native Picture pixels. Animated WebP throughput
   is outside this gate's scope. Existing Video compatibility is retained.
   Golden Video owns MKV demuxing and stream discovery. MapGyver Video exposes per-channel
   track selection through typed Parameters in the generic Inspector. Show embedded track
   names and languages when available, and define defaults and fallback behavior for missing
   or unsupported tracks. Reuse the M10/8 subtitle controls and M16/1 Golden Audio routing.
   Verify in the bundled app with MKV fixtures containing multiple distinguishable subtitle
   and audio tracks. Switch tracks during playback without resetting the shared transport,
   verify subtitle timing and audio/video sync through seek, crop and loop, and verify that
   selections survive project reload. Record supported container, codec and profile combinations
   individually. Unverified variants remain explicit follow-up requirements within this gate.
4. **First-pass bundled integration (after gates 1 and 2):** pinned redistribution/build/license
   dependencies, exact seek fixtures, bounded file/decode workers and queues,
   coalesced superseding seeks/stale-result rejection,
   missing-file relink/retry, device recovery and actual-app sustained 1080p/4K/resource performance
   for all common and HAP codecs. A playable prototype alone does not close the first pass.
   Acceptance also requires the changed transport, seek and Matrix behavior in gates 5–7.
5. **High priority — typed transport controls and timeline feedback.** Owners: MapGyver Video
   Parameters/transport projection on Golden's public Parameter and generic Inspector APIs.
   Expose **Playing** as read-only status and **Play**, **Pause**, **Stop** as trigger Parameters.
   Add authored boolean **Play at Load**, default false; enabling it starts playback on project/
   media load when the file is ready, respecting cancellation and current transport intents.
   Expose video **Length** as a read-only time Parameter, reflecting the longest loaded channel
   for uncropped media. Gate 10 defines the cropped Length and Current Position bounds.
   Remove **Seek To**. Replace it with one editable time Parameter, **Current Position**, which
   also receives actual transport feedback. For uncropped media, backend-owned bounds are
   **0 to the longest video duration across channels**, updated when files/channels change.
   The generic Inspector uses those bounds for its slider, and all callers use the same range.
   Suspend incoming position feedback while the user is seeking so it cannot fight the edit;
   resume authoritative feedback after the latest seek settles, without a stale-position jump.
   Unknown/unavailable duration must have truthful feedback and bounded edit behavior.
   Rename **Rate** to **Speed** everywhere. Runtime feedback remains distinct from authored
   state/history; every authored control uses real typed Parameters, never opaque node data.
   Verify read-only rejection, triggers, startup/load policy, empty/mixed-duration channels,
   changed duration/clamping, seek feedback arbitration, Inspector slider and UI/script/
   transport/headless parity, callbacks/snippets, independent copy, history and project reload.
6. **High priority — smooth instant seeking, paused and playing.** Owners: Golden Video's
   reusable decode/seek/GPU handoff and MapGyver's shared transport integration. User-reported
   reproducer: all-intraframe file `D:\Documents\MapGyver\test videos\ball_turn.mp4`.
   Paused seeking currently reports **"application made a call that is invalid > Enable D3D
   debug layer (0x887A0001)"**; playing seeks are choppy. This is reported evidence, not a
   confirmed root cause. Reproduce and fix the invalid-call path; enabling diagnostics alone
   does not satisfy acceptance. A paused seek must present the requested frame while remaining
   paused; playing seeks must remain responsive and continue from the latest requested position.
   Support continuous scrubbing with an immediate, smooth **25–30 fps feel** in both states,
   at least on this intraframe fixture. Measure input-to-present latency and distinct presented
   seek updates (target at least 25/s under continuous input on NVIDIA), alongside native visual review;
   a slider moving smoothly without fresh video pixels is insufficient. Preserve frame/timestamp
   accuracy, latest-request coalescing, stale-frame rejection, shared channel/Reference ownership,
   bounded queues and completion-safe D3D resource reuse. Verify rapid forward/backward seeks,
   endpoints, pause/play during scrubbing, repeated use and file/project/device recovery on both
   GPUs after software prerequisites. Retain cold/warm and codec-specific limits explicitly.
   **2026-11-11 user revision:** Intel may have lower smooth-seek throughput than NVIDIA.
   Record its measured rate/latency and limits; correct latest-frame seeking, transport state,
   continued progress during input, ownership and recovery remain required on Intel. This
   does not revise M10/4's separate rendering/performance budgets.
7. **High priority — Matrix Video transport policy.** Owner: MapGyver's Video transport and
   Matrix activation/transition/demand backend, consuming public Golden runtime contracts.
   Add typed enum Parameters for Video in Matrix: **On Activate = Play from Start (default),
   Resume, Nothing**; **On Deactivate = Stop (default), Pause, Nothing**. Play from Start
   restarts at zero; Resume continues from the retained position; Nothing sends no transport
   action. Default Stop waits until the outgoing transition ends before stopping the video;
   the transition must retain moving frames and demand until then. Specify Pause/Nothing timing
   explicitly with implementation and preserve the same transition-safe ownership boundary.
   Fence deferred actions on reactivation/interruption, deletion and project replacement; a stale
   outgoing completion must never stop a newly active or still-shared transport. References
   mirror their owner rather than create independent clocks or duplicate activation actions.
   Verify all enum combinations, zero/nonzero/interrupted transitions, same-video reactivation,
   shared consumers, manual transport interaction, Matrix enabled/Layer Stop contracts, and
   UI/script/headless parity, history/copy/reload with actual native playback observations.
8. **Low priority — subtitles per Video channel.** Deliver after Video time cropping (gate 10).
   MapGyver owns typed channel settings and source selection. Golden owns text extraction and
   rasterization. Each Video channel has its own disableable **Subtitles** folder, **Track** enum
   for that channel's source, and typed **Size**, **Font**, **Positioning** and local **File**
   settings. Channels share the Video transport and can display different subtitles at the same
   time. References reuse the selected channel's captions. Use the generic Inspector and the
   same channel reference for typed edits, commands and scripts. Define supported subtitle formats
   and track/local-file selection semantics before implementation, using a reliable library
   where available. Verify available/absent tracks, local-file selection and missing-file recovery,
   enable/disable, font/size/position rendering, source-time alignment during pause/seek/speed/loop
   on full and cropped ranges, native output/preview parity, shared consumers and all callers/
   history/copy/project roundtrip. Low priority changes delivery order only; this gate remains
   required before M09 unless the user revises it.
9. **Video auto-smooth looping (fade-loop).** Owners: MapGyver Video's typed transport
   authoring/policy; reusable timestamp sampling, decode preparation and GPU blending belong
   in Golden Video. Add a real typed **Auto-Smooth Loop** time Parameter, default **0** (off),
   in the generic Inspector. Disable the control and bypass its runtime effect when **Loop**
   is disabled; retain its authored duration for re-enabling Loop. While looping with a positive
   duration, overlap the ending with the beginning over that time, fading the ending out and
   the beginning in so unprepared videos loop smoothly rather than cutting or fading to black.
   Use complementary weights with a gentle **smoothstep** curve, `s(t) = t²(3 − 2t)` for
   normalized overlap progress `t` in `[0, 1]`, rather than a purely linear fade. Continue from
   the already-consumed beginning after the blend, avoiding a repeated segment or position jump.
   Keep one authoritative media transport and shared blend per channel for References and
   other consumers. Bound overlap to the available footage; define backend handling for short,
   mixed-duration and unknown-duration channels, speed changes, seek/stop and Loop toggles.
   Prepare head frames at the worker boundary with bounded queues/resources; no decode or
   recurring preprocessing in the engine loop. Verify zero duration, Loop off/on, short clips,
   repeated seams, curve weights and alpha/color, transport changes, missing/relinked files and
   device recovery. Prove native preview/output continuity and shared-consumer ownership, plus
   Inspector/UI/script/transport/headless parity, history, independent copy and project reload.
   [Functional checkpoint](../progress/evidence/mapgyver-m10-fade-loop-2026-11-18.json).
   Return in retained qualification for moving preview/output pixels, mixed/reverse playback,
   relink and device recovery, and the wider resource/consumer cases.
10. **Video time cropping.** Deliver after M15/11 and before gate 8. MapGyver owns the Video
    range and transport policy. Golden Video owns reusable timestamp sampling and loop preparation.
    Expose typed **Start** and **End** time Parameters in the generic Inspector for the shared
    Video transport. Default to the full source range. Validate the range in the backend against
    the available duration and define bounded behavior when files or channel durations change.
    **Current Position** uses crop-relative time. Position `0` maps to source **Start** and its
    maximum, `End − Start`, maps to source **End**. The read-only **Length** is `End − Start`.
    Apply those bounds to seek, scrub and frame stepping. Stop and Matrix Play from Start return
    to crop-relative zero.
    Treat **End** as exclusive during playback. At the exact maximum, a paused seek reports the
    endpoint and displays the last in-range frame. Use source presentation timestamps for a
    frame-perfect hard-loop seam, from the last in-range frame back to the first, without a
    duplicated or skipped boundary frame or cumulative drift. Auto-Smooth Loop uses the cropped
    tail and head, bounds overlap to the cropped duration, and resumes after the consumed overlap.
    Use frame-numbered fixtures to verify endpoint seeks and steps, playback with Loop disabled,
    repeated hard and smooth loops, crop edits while paused or playing, and mixed channel
    durations. Check native preview/output and Reference parity, all caller paths, Undo/Redo,
    independent copy and project reload.
    [Functional checkpoint](../progress/evidence/mapgyver-m10-time-crop-2026-11-18.json).
    Return for moving native Media Preview, mixed-duration and replacement cases, and wider
    reverse/native crop checks during retained Video qualification.

Use frame-numbered fixtures for exact seeks, mixed frame rates and stale-seek rejection.
Record cold/warm seek latency, timestamp accuracy, sustained throughput and per-format software
upload cost. Do not start native acceptance until prerequisite software/build checks succeed.
Audio delivery is defined in M16. Effects, mesh warp, edge blending, streaming and
cross-platform/cross-adapter guarantees remain outside the current scope.

### M14 — Apply the 2026-11-08 feedback after first-pass Video

Source: the user's editor, Composition, Screen and Matrix feedback in this chat.
Execute **now, after accepted M10/1,2,5,6,7**, under the explicit 2026-11-14 user revision;
retain all unfinished M10/4 qualification as release-blocking. M14 and M15 precede all remaining
Video work in the current task order above, with retained M10/4 qualification before M09.
The ten gates below cover every requested behavior; incorporation implements nothing and accepts no gate.

**2026-11-17 execution revision:** the user prioritizes feature delivery over further
interruption/recovery edge-case and optimization qualification. Implement the remaining usable
features in order9→10→8→2, then M15 and the remaining M10 features, using their functional
dependencies and relevant correctness checks. Keep incomplete selection, recovery, hardware and performance
acceptance in the explicit return queue for a second pass before M09; no requirement or budget
is waived. M14/1 is user-deferred, not accepted. Its current request-lifetime candidate and exact
failing regressions are preserved in [the deferred handoff](../progress/evidence/mapgyver-request-lifetime-deferred-2026-11-17.json).
Do not restore that candidate automatically while implementing features. Essential correctness
needed by a selected feature still belongs in that feature; unrelated qualification does not
block the next functionally ready feature.

Golden owns reusable 2D selection/input, viewport animation, workbench context menus and
enabled/optional-Parameter mechanisms. MapGyver owns editor target resolution, Composition
geometry/order, display allocation, sender naming and Matrix playback policy. Specialized
editors send explicit backend intents; do not implement domain defaults or structural edits
in UI helpers. Change typed Parameters, DTOs, generation and consumers together. Every owning
slice includes command/script/callback/snippet parity where relevant, atomic rejection,
Undo/Redo, copy/reload, bounded offscreen Inspector access and project/connection fencing.

1. **Consistent selection and hover in every 2D-like editor.** Owners: Golden public input/
   selection contracts and each app's editor adapters. Inventory all applicable editors,
   including graph canvases, Screen Warping, Surface Source, Composition and Matrix spatial
   editing, so this is not a fix limited to one panel. Select on primary **mouse/pointer down**,
   without waiting for mouse up or click. Background down deselects child elements and, where
   applicable, selects the edited owner (Composition → Composition, Screen Warping → Screen);
   resolve other owners explicitly through the adapter. Only selected elements show transform
   controls. Hovering an unselected element slightly highlights its contour without displaying
   handles or changing selection. **Escape deselects**. Share workbench/Inspector selection;
   background actions and Escape must not leave stale local handles. Preserve capture/cancel,
   multi-selection, locks and specialized tool behavior. Verify overlapping hit order,
   mouse-down-before-release, deselect/owner inspection, native/DOM agreement, offscreen
   selection, keyboard focus and two independent editors, including unavailable-source geometry.
2. **Smooth zoom, Frame and Home with common shortcuts.** Owner: Golden reusable viewport/
   animation mechanisms, consumed by every applicable 2D-like editor. Zoom is smooth;
   **F = Frame**, **H = Home**, and Frame/Home use a short, fast **ease-in/ease-out animation**
   instead of a camera jump. Toolbar and keyboard actions use the same path. Preserve pointer
   zoom anchoring, bounded frame lookup, focused text-field input, interrupted/repeated actions
   and the existing reduced-motion preference. Verify intermediate presented frames and final
   bounds in actual editors, native image/control alignment during animation, panel resize,
   sparse/large documents and both GPUs. Coordinate with retained M13/4 synchronization and
   M05/8 latency requirements; navigation animation does not close those independent gates.
3. **Composition actions, context menus and front-first order.** Owners: MapGyver Composition
   backend intents and editor contributions to Golden's toolbar/node context menu. Remove
   **Copy** and **Delete toolbar buttons**. Replace them with Screen-warper-style compact icon
   actions to fill the complete Composition extent or fit while preserving the source ratio;
   use shared backend geometry semantics. Add **to Front / to Back** icon buttons. The **first
   child is on top**, last is at the back; hierarchy, renderer, editor paint/hits and reordering
   agree for all callers. Right-clicking an item opens its ordinary Outliner node controls,
   extended with **Arrange → to Back / to Front** and **Transform → Reset / Fit**. Reset/Fit
   operate on the same typed placement Parameters as the Inspector. Preserve standard node
   controls in the menu; only toolbar Copy/Delete are removed. Verify toolbar/menu/command/
   script/headless parity, overlapping native alpha/order pixels and hit targets, mixed source
   ratios, history, nested folders where supported, copies and reload.
4. **Zero-scale Composition items remain valid.** Owners: MapGyver placement validation and
   Golden render-plan admission where needed. Allow exactly zero scale on either or both axes.
   Skip those degenerate draws without reporting **Source incompatible** or preventing other
   items from rendering. Keep the item selected/inspectable and recover rendering when scale
   becomes nonzero. Verify direct/Inspector/script/transport edits, mixed healthy/zero-scale
   items, zero crossings, source recovery, history/reload and independent native output pixels.
   Preserve rejection of nonfinite geometry; zero area must not poison the whole render plan.
5. **Composition uses media enabled metadata.** Owner: MapGyver media/placement projection
   on Golden's node metadata API. Remove **Placement → Parameters → Enabled** and use the
   child media's **enabled meta** as the sole authored enable state. Disabled media still
   appear in the dashboard and remain selectable/editable. Verify Inspector, dashboard, menu,
   scripts/transport/headless, render suppression/restoration, shared demand/final-use policy,
   history/copy/reload and absence of a duplicate Enabled Parameter or UI visibility filter.
6. **New Screens claim unused external displays.** Owners: MapGyver backend creation/default
   policy and Golden public display inventory/host output contracts. For **every creation path**,
   including startup and New File's automatic Screen, choose FullScreen on an available
   **external display not already used by MapGyver**. If none is free (including no external
   display), create **FullScreen with no display selected**. Exclude the built-in panel and
   already assigned displays; backend allocation must handle multiple Screens created together
   without duplicate claims. Prefer the lowest available Windows display number for fresh
   assignments. Keep saved display identity independent of enumeration order; preserve existing
   authored choices on load and do not retarget them on hotplug. Use current recovery
   behavior when an assigned display disappears/returns. Verify zero/one/multiple externals,
   all occupied, sequential/atomic multi-create, startup/New/menu/script/headless, history and
   reload, actual physical outputs and disappearance/recovery on both GPUs. Record any missing
   hardware case as open acceptance rather than treating a mocked topology as physical proof.
7. **Optional Shared Texture Target sender name.** Owner: MapGyver Screen target Parameters/
   sender supervision, using Golden's optional String Parameter metadata. Make **Sender Name
   disableable**, disabled by default. Disabled means the effective sender name is
   **MapGyver - <Screen name>**; enabled means the authored name is used. Preserve the dormant
   custom value across toggles. Screen rename updates the automatic name only; explicit mode
   retains its name. Do not add a separate boolean. Validate SDK naming/collision errors
   truthfully, retain endpoint retry/recovery, and retire old senders safely on rename/mode/
   target/project changes. Verify fresh defaults, actual discovery/consumer pixels, toggles,
   rename, multiple Screens, all callers and history/copy/reload without leaked senders.
8. **Screen anti-aliasing includes 4x.** Owners: MapGyver typed Screen quality contract and
   Golden reusable native render mechanism. Add **4x** to the existing choices and implement
   its corresponding rendering path, not just an Inspector label. Verify generated contracts,
   all callers, switching/history/reload, supported GPU capabilities, independent edge/alpha
   pixels and measured sustained 1080p/4K cost on both GPUs under the existing budgets. Report
   unsupported hardware explicitly. Keep retained M12/8 Intel4K Smooth failures open; adding
   4x neither waives them nor silently changes quality modes or performance limits.
9. **Matrix Layer and Cell use enabled metadata.** Owner: MapGyver Matrix authoring/playback/
   render projection on Golden metadata. Remove Layer **Render Enabled** and Cell **Enabled**
   Parameters. Use each node's **enabled meta** as its sole corresponding state. Disabled Layers
   still appear in the dashboard; disabled Cells retain their editable grid identity. Preserve
   the distinction between render suppression and play/stop/demand: disabling is not an implicit
   Stop or a new app-control policy. Define disabled-Cell launch behavior in the backend and
   preserve the existing verified lifecycle contract except for the requested state ownership.
   Verify UI/Inspector/script/transport/headless, groups, sparse/offscreen selection, active and
   outgoing Cells, native rendering, history/copy/reload and removal of duplicate settings.
10. **Layer Stop follows transition duration and overrides.** Owner: MapGyver Matrix playback
    resolver/runtime. Layer **Stop** fades using the resolved transition duration/curve rather
    than cutting immediately. Apply the established optional **Cell → Column → Layer** override
    precedence for the selected/playing Cell involved in that stop; selection in the UI is not
    the source of backend policy. Resolve and document independently launched or multiple
    outgoing Cells explicitly. Zero duration remains immediate. Retain outgoing media/app demand
    until transition completion and actual final GPU use. Verify default/Cell/Column overrides,
    disabled overrides/dormant values, zero/nonzero/Linear/Smooth, interrupted fades, repeated
    Stop and unrelated Layers through all callers, plus native timed alpha and completion-safe
    process/resource retirement. Preserve other scoped-stop contracts.

Close each gate only after the complete owning behavioral/software/strict/generated/UI and
specified actual-app/native checks pass. Update owning architecture docs when implementing;
planning does not describe these behaviors as already delivered. All ten gates and all eight
M10 gates remain required before M09. This amendment adds **ten gates and one phase** without
awarding acceptance or changing retained budgets.

### M15 — Apply the 2026-11-17 latest feedback before remaining Video

Source: [Latest Feedback.md](../../Latest%20Feedback.md) in the repository root, plus the user's
instruction that remaining Video work follows this feedback. Follow the M14 feature-first
execution revision above: selected M14 features9,10,8,2 are functionally delivered, with their
remaining qualification Pending in the return queue. M15/1–4,6–9,12–15 are Accepted; M15/5 is functionally
delivered with native presentation qualification Pending. The 2026-11-18 amendment retains
M15/8 as the current task through completion, then selects 12–15 before 9–11. M15/8 is now
complete; M15/12 custom colors is also complete; M15/13 Matrix color menus is also complete; M15/14 compact Layer Groups is also complete; M15/15 nesting rails is also complete; M15/9 geometry alignment is Accepted.
The current task order interleaves M15, Video and SGF Feedback. After M15/12–15, deliver
M15/9 → M10/9 → M15/11 → M10/10 → M10/8 → M16/7–14 → M16/1–6 → M10/3 → M15/10 → M17 → M18.
Retain user-deferred M14/1, blocked M14/6 and every other unfinished qualification gate with its
return trigger before M09.

Golden owns parameter admission, reusable gradients/animation, parameter bindings, shared
selection/viewport interaction and workbench presentation mechanisms. MapGyver owns Matrix
preset semantics, editor targets/geometry, startup defaults and internal-sender classification.
App UI consumes public Golden contracts. Authored preset entries, gradient stops and links use
real typed Parameters, references, metadata and child nodes; no opaque node data or UI-owned
document. Keep generated protocol and consumers synchronized. Each behavioral gate includes
backend/UI/script/transport/headless parity where applicable, atomic rejection, grouped history,
copy/project roundtrip and bounded offscreen editing. No frame-rate tree scans, per-element
polling, sequential subtree construction or unbounded selection/preset fan-out.

1. **External parameter control clamps safely.** Owner: Golden's authoritative typed Parameter
   admission/control evaluation, with MapGyver reproducers. Reproduce the reported crash when
   switching a new Surface's Opacity to Animation, the other opacity case and Video Speed;
   record the actual failing invariant before fixing it. Every external control path, including
   animation, bindings and scripting/transport inputs, clamps finite incoming values to the
   target's declared range before bounded conversion/application. UI-only clamping is insufficient.
   Preserve a defined safe policy for nonfinite/invalid typed inputs; never panic or corrupt
   state. Verify lower/upper overshoot, negative values for nonnegative targets, exact bounds,
   control-mode changes and changed constraints across numeric Parameter types. Verify actual
   Surface/media opacity and Video Speed, history/reload and healthy sibling controls; finish
   with actual-app animation edits that reproduce the original flows without a crash.
2. **Matrix uses ordinary selection and Golden custom colors.** Owners: Golden selection/color
   metadata and MapGyver Matrix projection/grid. Highlight only explicitly selected Layers,
   Columns, Cells and Groups with Golden selection styling. Remove cross-selected row/column/
   Cell outlines and extra header-label outlines; runtime active/playing state stays distinct
   from selection. Allow all four item kinds to use Golden's custom-color metadata and reflect
   it in the grid. Verify pointer/keyboard/shared selection, multiselection, nested Groups,
   disabled/offscreen items, Inspector color edits, history/copy/reload, theme contrast and
   independent Matrix panels without losing current inspection.
3. **Matrix presentation has subtle shape and active-state feedback.** Owners: Golden theme
   tokens and MapGyver grid presentation. Use small rounded corners for panels/Cells and no or
   very subtle unselected borders. Trial a subtle drop shadow on active Cells with a short
   transition; review it in the actual app before accepting the visual result. Selection,
   hover, custom color and playback must remain legible together. Verify light/dark themes,
   font/display scaling, narrow panels, virtualized sparse grids and interrupted state changes
   without clipping shadows, changing hit targets or rebuilding per-Cell native previews.
4. **Matrix configuration presets recall Layer/Column order and enable state.** Owner:
   MapGyver Matrix authoring/backend intents on public Golden typed-node/history contracts.
   Users can reorder and enable/disable Layers and Columns, capture that configuration as a
   named preset and recall it. Presets refer to stable object identities and store only their
   order and enable state; Cells retain their identities, settings and media, including when
   Layer/Column positions change. Reuse M14/9 Layer enabled metadata; define Column enable
   semantics once in the backend without duplicate booleans or UI policy. Define behavior for
   added/deleted/missing members, nested organization and recall during playback before coding.
   Provide create/update/rename/delete/recall through contextual UI and command/script APIs;
   ordinary entries use the generic Inspector. Verify one coherent history operation per recall,
   all-caller parity, exact Undo/Redo, copy/reload, disabled editing, active/outgoing playback,
   unchanged Cell contents and bounded sparse/large-Matrix structural work. This gate does not
   authorize implicit Cell playback snapshots or new play/stop policy.
   The [preset semantics](../architecture/mapgyver-matrix-presets.md) define membership/order,
   missing objects and live playback before implementation. Column render muting is delivered
   with software/native Inspector/history/reload proof. [Complete preset acceptance](../progress/evidence/mapgyver-m15-presets-acceptance-2026-11-17.json)
   verifies typed entries, all five contextual/command/named-script operations, committed
   callbacks, grouped history/rejection, copy/reload, disabled/outgoing playback and sparse
   million-address structure. Native capture/recall, paged member access and offscreen generic
   Inspector edits pass. M15/4 is Accepted; separate production/resource gates remain open.
5. **Shared editor feedback transitions and subtle handles.** Owners: Golden reusable 2D
   selection/hover presentation, consumed by Screen Warping, Surface, Composition and Matrix
   editors. Add short transitions for hover and selection instead of abrupt jumps; make Screen,
   Surface and Composition outlines/handles more subtle using shared theme/geometry tokens.
   Preserve accessible contrast and usable hit areas independently of visible handle size.
   Respect reduced motion, immediate input admission and native-image/control alignment.
   Verify intermediate presented states, fast hover/select changes, drag/cancel, zoom/resize,
   keyboard focus and independent editors in the actual app. Coordinate with M14/1–2 rather
   than duplicating selection stores; M14's unfinished correctness/lifetime requirements stay
   open. Earlier M13/4 acceptance remains historical; this new feedback requires the added
   presentation checks without silently changing retained timing budgets.
   [Functional feedback delivery](../progress/evidence/mapgyver-m15-editor-feedback-2026-11-17.json)
   adds shared120ms paint and subtler handles. Browser and native Screen/Source input checks
   pass; native paint packets interpolate and are accepted, but matching installation/presented
   frames and actual Composition/Matrix presentation remain unproved. Retain full M15/5
   qualification before M09; continue feature delivery with linked handles8 under the user's
   explicit deep-tests-later order. This checkpoint earns no gate credit or budget revision.
   [Second-pass verification](../progress/mapgyver-m15-editor-feedback.md) confirms native
   intermediate paint, keyboard focus, drag/cancel alignment and reduced motion. M15/5 is
   Accepted. Separate M14 selection and recovery and final app qualification stay open.
6. **Startup and New show the automatic Screen and Surface.** Owners: MapGyver document
   bootstrap/default editor targets through Golden's panel-follow APIs. On fresh startup and
   New session, Screen Warping displays the auto-created Screen and Surface Editor displays its
   auto-created Surface without a manual selection. Resolve actual created identities after
   atomic bootstrap, including when panels mount later. Preserve explicit loaded-project panel
   targets and follow/lock behavior. Verify fresh desktop/headless bootstrap, repeated New,
   hidden/reopened panels, empty/loaded projects and project fencing without synthetic authored
   edits, extra bootstrap history or stale previous-session targets.
   [Acceptance](../progress/evidence/mapgyver-m15-startup-targets-2026-11-17.json): the new
   candidate carries exact factory identities as an ephemeral backend hint, absent on load.
   Twelve startup/editor tests and real bundled desktop/headless checks pass initial/repeated
   New, hidden/locked/late/reopened panels, explicit saved targets/reload and empty loads.
   Bootstrap history stays zero; no authored Parameter or UI selection is synthesized.
7. **Surface Editor shows related Surfaces and source-free test patterns.** Owners: MapGyver
   Surface/source projection and editor, using public Golden canvas/preview contracts. Add an
   accessible compact navbar icon toggle to show all Surfaces using the selected Surface's
   source, across every Screen, for calibration in a common source-coordinate view. Toggle-off
   returns to the selected Surface. Define source identity in the backend, including shared
   Reference/channel relationships; an absent source must not group all unassigned Surfaces.
   Render the active test pattern in Surface Editor even without an assigned source.
   **2026-11-17 crop revision:** calibration uses the authored Surface crop in Screen Warping
   and native output just like media; Surface Source keeps the whole pattern for crop selection.
   Verify multiple Screens, source changes/loss/recovery, pattern on/off, geometry/selection/Inspector,
   independent panel targets, undo/reload and bounded queries for large Surface collections;
   do not allocate another decoder or source renderer per overlay Surface.
   [Complete acceptance](../progress/evidence/mapgyver-m15-surface-display-2026-11-17.json),
   retaining the [related-view proofs](../progress/evidence/mapgyver-m15-related-surfaces-2026-11-17.json):
   backend source choice, canonical related identity, bounded cursor pages and shared native
   pattern passes are implemented. Actual native pixels show source-free ColorBars/Grid,
   independent pattern/channel panes and related contours. Toolbar paging covers all70 peers;
   source/crop preservation, on/off/Undo/reload and source recovery pass. M15/7 is Accepted;
   retained Intel/GPU/resource/performance and M15/5 presentation qualification remain open.
8. **Surface-handle snapping within one Screen creates persistent shared links.** Owners: Golden public parameter
   binding/group contracts and MapGyver Screen geometry intents. Add a magnet-icon snap toggle
   to Screen Warping's toolbar. Dragging a handle near another Surface's handle in the same Screen snaps them
   together and creates a link, with a small link icon beside the group. Exclude same-Surface and other-Screen
   handles; support groups larger than two. Moving any member updates every linked Surface live, before release, including
   edits through Inspector/script/transport. Clicking the group's link icon removes all its
   links. Use the generic binding system without cycles, UI-maintained parallel links or
   nonterminating update propagation. Define common coordinate conversion, conflicting existing
   controls and snap-off behavior explicitly. Verify threshold/zoom behavior, multi-member moves,
   all-callers, link/unlink history, copy/delete/Undo, reload and missing/recreated targets with
   bounded group propagation and accurate native handle/output geometry.
   The first backend slice implements typed `LinkScreenCorners`/`UnlinkScreenCorners` through
   the existing script/transport operation and generic Golden binding groups. Its coordinate,
   control-conflict, copy and missing-target policies live in
   [Screen-handle links](../architecture/mapgyver-screen-handle-links.md). Toolbar/proximity
   behavior and native verification remain required; this partial delivery awards no gate.
   **2026-11-18 user correction:** linking is only between different Surfaces inside the selected
   Screen; the earlier cross-Screen interpretation was incorrect and is superseded. Use the
   user's `magnet-svgrepo-com.svg`, colored when active and grey when off, with no snap paging
   arrows. Candidate pages may load automatically within the editor's existing bounded scope.
   Verify held-pointer peer contours and actual output pixels, not only the released state.
   [Functional delivery](../progress/evidence/mapgyver-m15-surface-links-2026-11-18.json)
   passes focused software checks, five native drag/link/Undo cycles and held-pointer
   peer/output proof. The corrected fixture waits independently for native hit and authoring
   readiness. Guarded foreground pixels match all eight image edges to DOM geometry, and all
   four corner hits plus the unlink hit match. Three-member merge/unlink and normal shutdown pass.
   M15/8 is Accepted; retained high-refresh/Intel/presentation-motion qualification is unchanged.
9. **Surface, Composition and Matrix geometry align while moving/scaling.** Owners: Golden
   reusable viewport snapping hints and MapGyver authoritative geometry intents. Add a magnet
   toggle to the applicable editor toolbars. While moving or scaling an element, snap close X/Y
   edges/centers to other elements and show subtle alignment guides. Keep this transient
   alignment distinct from persistent Screen-handle links. Viewport-dependent proximity hints
   may be computed in UI; backend intents own the final typed geometry and grouped edit.
   Define candidate coordinates and exclusions per editor, support zoom/pan/mixed sizes and
   avoid self-snapping, jitter and full-document scans on pointer movement. Verify move/scale,
   one/both axes, toggle-off, unavailable sources, zero-scale items, history, native/DOM alignment
   and sparse/large layouts; inspector/headless geometry must remain valid through the same API.
   Accepted (see [geometry alignment evidence](../progress/evidence/mapgyver-m15-geometry-alignment-2026-11-18.json)). Bundled app
   move/resize, zoom/pan, exclusion, sparse layout and history checks pass. Held Windows
   capture places Composition edges within one physical pixel of the guides.
10. **Gradient belongs in Golden as a reusable node and Color animation control.** Owners:
    Golden typed nodes/Parameters, animation runtime and generic Inspector/editor hooks. Expose
    a standalone Gradient node analogous to Automation and use that same generic gradient
    model when a Color Parameter uses Animation control mode. Stops are typed child nodes with
    explicit interpolation/evaluation semantics; do not keep a second MapGyver gradient model
    or put app policy into Golden. Audit existing gradient ownership before moving it, update
    public registration/protocol/persistence and all consumers in the same slice, and remove
    obsolete app-private implementations. Verify stop edits/order, endpoints/interpolation/alpha,
    empty/single-stop policy, control attach/detach, commands/scripts/callbacks/snippets where
    applicable, history/copy/reload and actual MapGyver Color animation with no range crash.
    Keep unrelated Chataigne-only editor improvements deferred; shared regressions remain required.
11. **Shared Texture defaults exclude internal Screen senders.** Owner: MapGyver backend sender
    catalog/default policy through Golden's public discovery contracts. On creation, select
    the first external texture in discovery order or None when no external texture exists;
    never automatically choose an internally generated Screen-output texture. Label internal
    choices **(internal)** while preserving their stable identities and deliberate manual use.
    Reconcile M13/1's older first-sender contract for every factory/command/script/headless path
    and Auto-mode recovery: losing an external sender must not silently select an internal one.
    Preserve explicit saved/manual choices and existing reconnect/backoff/fencing. Verify empty,
    internal-only, mixed and changing catalogs, multiple own outputs, rename/loss/recovery,
    creation/history/copy/reload and actual SDK discovery with correct pixels; labels alone are
    not proof of sender provenance.
    [M15/11 acceptance](../progress/evidence/mapgyver-m15-external-sender-defaults-2026-11-18.json) records caller parity, headless reload and native pixels for competing registrations, exact-name takeover, rename and recovery.

12. **Screens, Surfaces, Medias and Folders support custom color.** Owners: Golden public
    typed custom-color metadata/Inspector mechanisms and MapGyver node declarations and
    manager/editor projections. Reuse the existing Golden color model without parallel UI-only
    settings. Users can set and reset color for each kind, including nested manager Folders;
    reset restores its normal default appearance. Reflect the same authored color consistently
    in relevant outliners/editor object cues while keeping selection and runtime state distinct.
    Verify all four kinds, generic Inspector and backend/script/transport parity, Undo/Redo,
    copy/duplicate and project reload, default/reset behavior and light/dark contrast. Keep
    projection and offscreen inspection bounded; custom color must not alter rendered media.
    [Accepted object-color delivery](../progress/evidence/mapgyver-m15-object-colors-2026-11-18.json) verifies all kinds,
    normal Inspector set/reset, public script methods, history/copy/reload, bounded editor
    accents and light/dark WebView appearance. M15/12 is Accepted; retained presentation
    motion/Intel/GPU/resource qualification is unchanged.
13. **Matrix contextual menus expose Set Color / Reset Color.** Owners: Golden reusable color
    actions and MapGyver Matrix contextual-menu registration. Include those exact actions for
    Layers, Cells and Columns, reusing their existing custom-color authority and picker rather
    than a separate Matrix color setting. Include Layer/Column Groups wherever these menus
    represent their corresponding item kind. Verify correct context target, existing
    multiselection conventions, default/custom/reset states, offscreen selection, Inspector
    synchronization, grouped Undo/Redo and copy/reload. Earlier M15/2 acceptance covers its
    original Inspector workflow only; it does not accept these new menu actions.
    [Accepted Matrix menus](../progress/evidence/mapgyver-m15-matrix-color-menus-2026-11-18.json) now verify all five kinds,
    shared picker/grouped history, sparse Cells, context-only multiselection, Inspector/offscreen
    synchronization, metadata copies and reload. M15/13 is Accepted; retained qualification is unchanged.
14. **Matrix Layer Groups default to a compact single-line header.** Owner: MapGyver Matrix
    presentation on Golden workbench/Inspector contracts. Hide inline Opacity and Blend Mode
    controls by default so a Group occupies one line. Preserve the authored typed controls and
    their rendering semantics; both remain editable in the generic Inspector. Keep essential
    name, expand/collapse, selection and contextual actions usable. Verify new/existing Groups,
    nested/collapsed/empty Groups, narrow panels and font/display scaling, correct virtualized
    row geometry/hit targets, and unchanged opacity/blend/history/project behavior.
    [Accepted compact Groups](../progress/evidence/mapgyver-m15-compact-groups-2026-11-18.json) verify the full functional
    Inspector/collapse/history/reload, old/new project and mixed-height geometry scope.
    M15/14 is Accepted; physical/GPU/resource qualification remains unchanged.
15. **Colored left-side wrapping makes Matrix Layer Group membership obvious.** Owner:
    MapGyver Matrix hierarchy presentation using Golden custom colors/theme tokens. Give each
    Group an elegant, visible left-side rail/bracket that wraps its visible children, colored
    from that Group item's custom color (with the normal theme fallback after reset). Combine
    restrained indentation and distinct nested rails so multiple nesting levels remain readable
    and each Layer's containing Group is obvious. Review the actual app for light/dark themes,
    selected/hovered/disabled states, custom/reset colors, multiple nested Groups, collapse,
    reparent/reorder and viewport clipping/scrolling. Keep rails coherent across virtualized
    rows without rendering all descendants, stealing hit targets or confusing selection/playback
    cues. Coordinate with the compact header in 14; do not remove Group rendering behavior.
    User visual amendment: follow Inspector container styling with tinted bodies, rounded
    returns and inset row cards; ancestor wraps must not cross child selection/hover borders.
    [Accepted Group rails](../progress/evidence/mapgyver-m15-group-rails-2026-11-18.json) verify nested ownership/colors,
    offscreen/clipped spans, light/dark/selected/disabled/reset states, collapse/reparent/reorder,
    clear controls and pointer transparency in the app. M15/15 is Accepted; retained qualification is unchanged.

M15/1–4,6–9,12–15 are Accepted with their owning software/native proofs;5,10–11 remain open.
M15/2 covers explicit/shared/modifier selection, nested/disabled/offscreen objects, Inspector
colors, history/copy/reload, representative theme contrast and independent panels. Wider app
stress/interruption and retained GPU/resource qualification return in the deferred pass before M09.
Functional delivery uses the user's targeted-check cadence above, retaining deferred deep
checks before M09. A gate is Accepted only when its complete owning scope is verified.
The source mapping is: crash/range → 1; Matrix UI → 2–3;
Matrix presets → 4; shared transitions/handles → 5; startup → 6; Surface display → 7;
Screen links → 8; geometry alignment → 9; Gradient → 10; Shared Texture → 11;
2026-11-18 custom colors → 12; Matrix color menus → 13; compact Groups → 14; nested Group rails → 15.
Planning changes the fixed scope from **106 to 117 gates** and **15 to 16 phases**; the accepted
numerator initially stayed **92**, making overall progress **79%** and phase completion **8/16 (50%)**.
M15/1 acceptance now raises the numerator to **93/117**; rounding still gives **79%**.
M15/2 acceptance raises it to **94/117 (80%)** and M15 to **2/11 (18%)**; the denominator is unchanged.
M15/3 presentation acceptance raises it to **95/117 (81%)**, M15 **3/11 (27%)**, with unchanged scope.
M15/4 preset acceptance raises it to **96/117 (82%)**, M15 **4/11 (36%)**, with unchanged scope.
M15/6 startup acceptance raises it to **97/117 (83%)**, M15 **5/11 (45%)**, with unchanged scope.
M15/7 Surface-display acceptance raises it to **98/117 (84%)**, M15 **6/11 (55%)**, with unchanged scope.
The 2026-11-18 feedback adds four fixed gates: **117 → 121**, with **98 accepted unchanged**;
overall acceptance becomes **81%**, M15 **6/15 (40%)**, and phase completion remains **8/16 (50%)**.
This is added scope only; no prior acceptance is removed and no new gate is accepted by planning.
M15/8 Surface-link acceptance raises the numerator to **99/121 (82%)** and M15 to **7/15 (47%)**;
phase completion stays **8/16 (50%)**. The denominator and all deferred gates remain unchanged.
M15/12 object-color acceptance raises this to **100/122 (82%)**, M15 **8/15 (53%)**.
No phase completes and no retained qualification is waived.
M15/13 Matrix color-menu acceptance raises this to **101/122 (83%)**, M15 **9/15 (60%)**.
The fixed denominator and retained qualification are unchanged by this acceptance.
M15/14 compact Group acceptance raises this to **102/122 (84%)**, M15 **10/15 (67%)**.
No phase completes; no retained qualification is waived.
M15/15 Group-rail acceptance raises this to **103/122 (84%)**, M15 **11/15 (73%)**.
The fixed denominator, remaining phase gates and all qualification budgets remain unchanged.
M15/9 acceptance raises this to **104/122 (85%)**, M15 **12/15 (80%)**.
M14/1,6 and all retained correctness, physical, recovery and performance requirements remain
required before M09. New features follow the user's feature-first cadence; a checkpoint or a
passing subset of checks never closes a complete gate.

### M16 — Deliver SGF Feedback after subtitles

After the M10/8 subtitles functional checkpoint, deliver gates **7–14 → 1–6** before returning
to M10/3 extended formats. Deferred subtitle qualification stays in the Video return queue.

Use typed Parameters and child nodes with the generic Inspector. Apply the existing validation
cadence and retain full acceptance in the ledger until verified.

#### Matrix transport and UI (M16/7–14)

7. **Matrix-level transport.** Add Previous/Next Column for Matrix-wide column launches and
   separate Previous/Next triggers for top-level Column Groups. Expose the Matrix-level
   triggers, including Stop, as accessible icon actions in the editor's top-left area.
   Verify ordinary and grouped columns, top-level group navigation with nested groups present,
   Stop, and parity across editor, script and transport calls.
8. **Layer-level transport.** Add Previous/Next Cell on each Layer. Step within that Layer
   and preserve the Matrix's column anchor and playback on other Layers. Verify independent
   Layer navigation with sparse, empty and disabled Cells and through all trigger entry points.
9. **Column Group transport.** Group Play launches the first Column in that Group. Group
   Previous/Next steps among its direct children in document order, handling both Columns
   and nested Groups as one step each. Verify mixed and nested child order, empty Groups and
   trigger behavior after reorder.
10. **Cell Auto Next.** Add a typed mode with **None**, **End of Media** and **Specific Time**,
    plus a time setting for Specific Time. End of Media initially uses Video's effective
    playback range. Offer an option to start the next Cell one effective transition duration
    before the Video ends so the outgoing Video keeps playing during the transition. Keep the
    end signal extensible to other finite media and audio-only Cells. Verify disabled, timed,
    Video-end and overlapping advances, including cropped Video and a transition longer than
    the remaining playback time.
11. **Previous/Next wait modes.** Apply typed **Direct**, **First Media End** and **All Media End**
    wherever Previous/Next triggers are exposed. Direct launches immediately. First Media End
    waits for the earliest active Video boundary, and All Media End waits for each eligible
    Video in the trigger's scope to reach one boundary. A non-looping Video reaches its
    directional endpoint. A looping Video reaches its next wrap. Each crossing counts once
    per request. With no eligible Video duration, navigation is immediate. Verify mixed
    durations, looping, repeated requests, Stop and interrupted transitions.
12. **Matrix transition indicators.** Use the green bottom lines on Cells and Columns to
    show live transition weight. Grow incoming lines from zero to full width with the left
    edge anchored. Shrink outgoing lines from full width to zero with the right edge anchored.
    Verify interrupted and immediate transitions and correct progress when rows enter the
    virtualized viewport.
13. **Empty Cell hover continuity.** Keep the hover outline visible as the pointer crosses
    between an empty Cell and its `+` button. Verify the outline clears on exit and that
    the button still opens Add Media by mouse and keyboard.
14. **Compact Matrix opacity and blend controls.** Move Opacity to a vertical slider along
    the right edge of the Matrix editor. Use a smaller Blend Mode control with an accessible
    name and no visible label. Bind both to the existing typed Parameters. Verify Inspector
    synchronization, Undo/Redo, keyboard access and layout in a narrow dock.

MapGyver owns Matrix trigger scope, playback and transition decisions. The editor dispatches
the same typed intents as scripts, transport and headless callers and uses bounded runtime
feedback for transition indicators. Gates 7–9 update the existing M07/M12 navigation layout
and retain the applicable M13/M14 trigger, stop and enablement contracts.

#### Earlier SGF feedback (M16/1–6)

Use the public `golden_audio` API for device I/O, routing and audio playback in gates 1–3.
MapGyver owns authored routes and Video and Matrix transport policy, and maps its typed
settings into the audio runtime configuration. [Golden Audio crate documentation](../../crates/golden_audio/README.md)

The M16/1 functional checkpoint unlocks implementation of gates 2–3. Keep OS-level signal
capture and physical endpoint disconnect/reconnect in the qualification return queue. Full
M16/1 acceptance remains required before M09.

1. **Audio output and routing.** Add audio-device settings in Preferences with virtual
   channels mapped to physical output channels. Video media route audio to virtual channels
   with per-channel volume. Keep virtual channel identities and authored routes stable when a
   device is missing, changes layout or reconnects. Run device I/O and mixing at the runtime
   boundary. Verify actual output, gain and channel mapping, device loss and recovery, and
   route persistence across restart and project reload.
2. **Video intro and loop.** Support an authored loop at the end of a single Video file or a
   separate loop file after the intro. Allow a separate audio source for the loop. Integrate
   both modes with Video's shared transport and document their interaction with Auto-Smooth
   Loop. Verify intro-to-loop handoff, repeated loop seams, audio synchronization, seek and
   stop behavior, missing-file recovery, and typed edits through history, copy and reload.
3. **Matrix Cell audio.** Let each Cell use its media's audio, override it with its own audio,
   or play audio without visual media. Use the same virtual output routing and existing Matrix
   transport and transition policies. Verify each mode with native audio output, including
   activation, transition, stop and project reload.
4. **OSCQuery for all Golden apps.** Implement the app-agnostic service in the `golden_core`
   transport layer and expose it through the shared Golden runtime. Provide a disableable
   Preferences folder and a configurable port. Apps can supply their own defaults, including
   **42000** for Chataigne and **43000** for MapGyver. Advertise enabled services through
   Zeroconf and expose the full app node hierarchy for discovery and control. Review
   `juce_organicui` for extension behavior. Route undoable
   actions, manager item add/remove/reorder and all editable node metadata, including name
   and color, through the same backend intents as local callers. Add **Copy OSC Address** to
   node context menus and **Show OSC Address in Help** under **Preferences → Interface** for
   the bottom-bar node help. Implement client-specific `LISTEN`, with immediate feedback for
   isolated changes and rate-limited bundles for bursts. Verify reuse through an app-neutral
   Golden host fixture and check the configured defaults in Chataigne and MapGyver. Verify
   service discovery, enable/disable, port changes and recovery, hierarchy and mutation/history
   parity, address display, subscription isolation and feedback under burst load.
5. **Seamless Surface Editor and Media Preview switching.** Keep the last valid frame visible
   until the replacement is ready. Reuse the live source when two Surfaces resolve to the same
   media so changing selection avoids a reload. Fix verified occurrences in other panels if
   they share the same lifecycle defect. Verify repeated same-source and different-source
   switches in the native app, including rapid selection, frame continuity and source reuse.
6. **Stable linked outlines in Screen Warping.** Reproduce the supplied linked-handle issue and
   keep both Surface outlines on the current drag geometry throughout the gesture. Verify
   native drags across linked Surfaces, including rapid movement, release and Undo/Redo.

### M17 — Deliver NDI Media and Screen Output

MapGyver owns the Media and Screen target types, typed Inspector settings, source defaults and
audio route policy. NDI is available by default in the normal Media and Screen target choices.
Discovery and streams start automatically as needed for authored endpoints, without a preference
or session activation step. Golden owns reusable NDI discovery and stream lifecycles, video frame
transfer and live audio buffers, clocks and mixing through public runtime APIs. Evaluate a
maintained Rust binding against the official SDK before choosing the adapter. Keep SDK calls,
network waits, frame transfers and A/V synchronization on runtime workers with bounded queues
and completion-safe buffer ownership. A missing or incompatible runtime reports an actionable
status on the affected NDI endpoints and the rest of the app stays usable.
[NDI SDK receive](https://docs.ndi.video/all/developing-with-ndi/sdk/ndi-recv) ·
[NDI SDK send](https://docs.ndi.video/all/developing-with-ndi/sdk/ndi-send)

1. **NDI Media (receive).** Add NDI as a Media type with typed per-channel sender selection and
   audio routing in the generic Inspector. Follow Shared Texture for Manual/Auto discovery,
   internal sender labeling, stable selection through loss and automatic reconnect. A new
   source selects the first external sender or None. Use the normal Resolution/Fit settings,
   Media Preview and shared source demand across Surfaces and References. Route incoming audio
   channels to named virtual outputs through Golden Audio, with gain and mute. Silence a missing
   or disconnected audio stream and resume with fresh synchronized samples on recovery. Verify
   with an independent LAN sender using moving picture and audio pulses aligned to visible markers,
   with distinct signals per channel. Cover source changes, shared receiver demand, video-only
   sources, sender and network loss/recovery, final-use retirement, and typed history and reload.
2. **NDI Screen Output (send).** Add NDI as a Screen target alongside Shared Texture. Reuse
   the rendered Screen image, resolution, frame rate and optional sender-name convention
   (**MapGyver - <Screen name>** when unset). Add typed routes from named virtual audio outputs
   to the NDI stream's audio channels, independent of local audio device selection. Unassigned
   routes are silent. Publish synchronized picture and audio while the editor is closed or
   minimized. Verify A/V offset, drift and channel mapping at an independent LAN receiver using
   visible markers and matching audio pulses. Cover concurrent Screen targets, route and format
   changes, unavailable local audio output, receiver and network reconnect, typed history and
   reload, and disable/delete/project replacement with sender withdrawal and final-use retirement.

Check Windows x64 runtime loading and redistribution terms for the selected SDK version. Verify
the supported installer path on a clean Windows system and first use on launch.
Include NDI receive and send alongside the existing source and Screen target regressions in the
bundled app. [NDI SDK distribution](https://docs.ndi.video/all/developing-with-ndi/sdk/software-distribution)

### M18 — Deliver feedback after NDI

Deliver in order, with one acceptance gate and one reviewable task per feedback item.

1. **OS file drag-and-drop.** Golden owns file-drop input and reusable drop feedback. MapGyver
   owns the target-specific import and assignment intents. Dropping on the Medias panel
   background creates and configures a compatible Media. Dropping on an existing compatible
   Media updates its file. A drop in Screen Warping creates a Media and assigns it to the
   targeted Surface or to a new Surface on a background drop. Composition adds a child
   Media alongside existing children. A Matrix Cell adds or replaces its Media, and a
   drop in the open area to the right of the Cells adds a Column. Dropping an `.exe` on a
   Column, Group or Cell configures App Control for that object. Show the intended action and
   target through clear icons and smoothly animated highlights, with reduced-motion support.
   Verify each target with real OS drops, including unsupported files, Undo and project reload.
2. **Project Settings.** Golden provides project-scoped global settings as typed Parameters
   saved in the project file. Save Preferences at app scope. Open Project Settings from the
   File menu and provide separate top-bar icons for Preferences and Project Settings. Verify
   both entry points and confirm that opening another project restores its own settings while
   retaining app-wide Preferences.
3. **File Explorer panel.** Golden owns the reusable workbench panel. Default the project
   folder to the directory containing the project file. Allow an override through Project
   Settings and a folder-picker icon in the panel. Provide Tree View and Thumbnail View.
   Drag files from the panel into MapGyver using the same import intents as OS drops. Verify
   both views, the default and custom folder after project reload, and a drag into a Media target.
4. **Shader Media.** MapGyver owns the Media type, typed settings, Shader Editor and Shader
   Library panels. Golden owns reusable GPU shader execution. Support GLSL, ShaderToy and ISF,
   including multipass shaders. Allow embedded code edited in the Shader Editor or an external
   file that reloads automatically when changed. Detect shader parameters and expose them as
   typed Media Parameters. For GLSL and ShaderToy, list declared uniforms with individual Add
   and Add All actions and honor FragCoord.XYZ metadata such as
   `uniform float u_value1; //0.5 [0, 1]`. Allow other Medias as input textures. The Shader
   Library supports browsing and drag-and-drop from ShaderToy, ISF, FragCoord.XYZ and a local
   library. Verify rendered output for each format, multipass execution, parameter editing,
   Media inputs, file reload and import from each library source.
5. **Inspector selection after creation.** Use Golden's shared selection and Inspector with
   MapGyver choosing the appropriate creation target. Keep the parent Media inspected when
   adding a Channel, and inspect the Matrix Cell when adding a Media to it. Review related
   creation flows for other cases where the new child steals useful selection. Verify the
   resulting selection after creation and that explicitly selecting the child still works.
6. **Browser video previews.** Golden owns the reusable browser preview transport and quality
   control. MapGyver supplies its editor textures through the shared preview interface. Add
   browser top-bar choices for **Disabled**, **Auto** (default), **Forced Low Quality** and
   **Forced High Quality**. Auto adjusts quality to preserve smooth interaction within
   available bandwidth. Bound preview work to visible consumers. Verify moving textures in a
   standalone browser, switching all four modes, adaptation under constrained bandwidth and
   stream retirement when the preview is disabled or closed.

### M09 — Qualify the actual app and Windows artifact

**Gate 1:** after all M10–M18 feature/feedback gates, qualify the complete revised bundled
Windows x64 product and all retained M03–M18 requirements. Run exact binary/assets/dependency
identity and license checks, Rust/UI/codegen/public-boundary and affected Chataigne regressions,
Q01/Q02 prescribed release series, revised Q03 soak, Q04 authored/active scale and full Q05
recovery/retirement. Include all media types, Media Preview, revised editors, all Screen output
modes, optional app control and SGF audio, OSCQuery, editor and Matrix transport workflows.
Include NDI receive/send with routed audio and verify the selected Windows runtime delivery path
and behavior with the runtime absent. Include M18's file import, project settings, File Explorer,
Shader Media, creation selection and remote browser preview workflows.
The footer reports actual
video FPS, active GPU, frame time and resource usage through a public Golden contribution hook.
Publish bounded worker observations.

The [qualification contract](../progress/mapgyver-qualification-contract.md) explicitly replaces
the obsolete two-channel Composition fixture with single-output Composition and Reference use,
adds Video and separately measured software-decode uploads. Preserve GPU-sharing requirements,
all existing numeric budgets, historical failures and unavailable configurations. No unapproved
GPU-work/window-timing proposal becomes operative through this merge.

A reviewable packaged artifact, onboarding and reproducible app-specific run/package commands
are required. Release/signing/publication need a separate instruction.

## Verification and status maintenance

Use the targeted validation cadence above for each coherent feedback milestone. Select checks
for the changed behavior and contracts; batch full cross-app, Intel/GPU and edge-case matrices
in the deferred verification lane. Use meaningful tests beside their owner under `tests/`.
Run formatting at the root and Golden Core manifests. Native/hardware validation follows
inspected terminal success of its compile/software prerequisites.

Keep [mapgyver-status.md](../progress/mapgyver-status.md) the user's **only progress checklist**:
finished capabilities, next features in execution order, built-but-unqualified work with reasons,
and exact manual needs. Use short list items; no historical logs, dense phase tables or long prose.
Keep the formal phase/subphase dashboard, verified counts, one active gate, return triggers,
current batch and evidence links in [mapgyver-ledger.md](../progress/mapgyver-ledger.md).
Update both in place at each milestone. Progress counts only complete verified gates;
partial/checkpointed/unavailable work stays open. Document denominator changes in the ledger
and retain prior evidence without treating it as proof of changed code.
