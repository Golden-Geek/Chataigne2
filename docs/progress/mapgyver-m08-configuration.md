# Reusable app-control configuration

**M08/2 remains Active.** The first configuration milestone is implemented and functionally
verified: equivalent launch definitions preserve the existing owned process rather than
triggering a stop/restart. The typed settings milestone below is also verified. Neither
milestone alone accepts the full configuration/demand gate. Column/Cell ownership, standalone
removal and normalized-key runtime sharing are now functionally verified below. Independent
Cell activation is now verified below; the complete revised retained-demand matrix remains required.

## Ownership and behavior

`applications/configuration.rs` owns the public `ApplicationRuntimeKey`. Its constructor
validates authored settings, normalizes absolute executable/directory spelling and DOS drive
letters, and sorts explicit environment entries. The resulting immutable, hashable key contains
every launch/control setting. Both runtime capture and the actual process Agent now require
this type; raw definitions cannot enter the Agent directly.

The normalizer uses Rust's [`std::path::absolute`](https://doc.rust-lang.org/std/path/fn.absolute.html),
which performs no filesystem access and retains verbatim Windows path semantics. It does not
resolve symlinks, inspect executables or guess directory case sensitivity. File-component/name
case remains significant; only DOS drive letters are folded. Environment name/value spelling,
argument order and inherited versus explicitly empty environments remain distinct. Settings
are still authored exactly as supplied: normalization creates a runtime projection, never a
document rewrite or history entry. Key hashes are in-memory collection keys, not persisted IDs.

Semantically equivalent path/environment-order edits now keep configuration state Current
and retain the same native PID and launch UUID. Effective argument or control changes retain
the existing stop/retirement/readiness/backoff rules. At that first milestone, runtime owners
were still keyed by consumer identity; the shared-runtime milestone below supersedes that limit.

## Verification — 2026-11-03

- `cargo test --locked -p MapGyver --lib`: **356 passed, 0 failed, 2 explicit qualification
  tests ignored**. Five new tests cover key equality/hash, all setting distinctions, malformed
  definitions, nonexistent/verbatim/case-sensitive path behavior and real Agent create/resume/
  close/kill counts under equivalent versus effective changes. Existing callback, project,
  admission, readiness, retention and generated-contract tests remain passing.
- Strict Clippy passes for the library and all tests with
  `--features native-qualification,application-process-fixture -- -D warnings`.
- Native prerequisites were inspected before execution: terminal library/Clippy success and
  `cargo test --locked -p MapGyver --features application-process-fixture --test application_runtime --no-run`.
- The resulting actual Windows suite passes **7 tests, 0 failures, 1 ignored** in **2.67 seconds**.
  Its new case edits executable slash/dot spelling and environment order, waits for the new
  definition revision, and proves identical PID/launch UUID, Current configuration, unchanged
  authored settings/history and completion-based process retirement. Existing real-process
  demand, missing-definition/final-use retention, crash/backoff, absent readiness, project
  replacement, renderer final completion and headless-driver cases also pass.
- Both Cargo formatters and diff checks pass. DTO serialization is unchanged; Hash derivations
  add no protocol fields, and checked-in generated TypeScript equality remains passing.

An initial software compilation found a missing import after the Agent type change; it was
corrected before any native execution. The native suite passed on its first run. The ignored
pinned-descendant case still requires the explicitly built Golden IO process fixture and is
not claimed as accepted here; preserve its M08/4 return requirement.

Native suite: `target/debug/deps/application_runtime-d39e2a9bf0d5ebf6.exe`.
SHA-256: `1EEEDC9DAF039DE4F4308E129A7086E9A59883D4AE0A3E304C4A78F493F03ED6`.
Receipts: `target/m08-config-{tests,clippy,native-build,native}.log`.

## Typed settings milestone — 2026-11-03

`AppControlNode` replaces the opaque Application node with `mapgyver.app_control.v1`.
Thirteen declared Parameters own App file/directory, window/readiness, deadlines, restart and
environment inheritance. Arguments and environment entries are real ordered child nodes with
their own String Parameters. Inspection and runtime capture project this tree; no persisted
definition DTO remains. Old Application records and unknown app-control schema versions fail
before Golden's recoverable decoder can silently skip their settings.

Configure preserves existing child identities by ordinal and groups retained scalar/entry
writes into one atomic proposal. Surplus children retire and missing children use one forest
insertion. Direct Parameters enforce the same executable, deadline, string and aggregate
32 KiB bounds. Entry edits invalidate the owner revision without a tree snapshot. Inherited
and explicitly empty environments remain distinct. Ordinary settings now use Golden's generic
Inspector, including File widgets; the old custom definition form is removed.

Inactive settings remain materialized and persist. DSL dependencies control materialization,
so reversible Inspector visibility uses the existing presentation filter rather than removing
write targets. The shared File constraint now permits the unconfigured empty value, preserving
extension validation for chosen files. App-owned process validation still requires an absolute
executable. [Ownership details](../architecture/mapgyver-parameters.md).

Verified results ([receipt](evidence/mapgyver-m08-typed-configuration-2026-11-03.json)):

- **363 app tests pass**, with the same two explicit qualification exclusions. Seven new cases
  cover generic/direct writes, caller parity through the existing suites, stable entry identity,
  unchanged-definition fences/history, one-step Configure, undo/redo/project replay, deadline
  proposal ordering, child limits, aggregate-size atomic replacement, generic subtree duplication
  with fresh child identities/one-step history and old-schema rejection.
- **26 shared Parameter tests pass**, one pre-existing exclusion; clearing a constrained File
  passes while wrong/missing chosen extensions still reject. Strict Clippy passes for both
  changed crates, library/tests and native/process-fixture feature coverage.
- Svelte checks report **zero errors/warnings**; touched UI and fixture formatting pass. Rust
  generated-TypeScript parity remains passing with no DTO change.
- After software and binary prerequisites succeeded, the actual Windows lifecycle suite passes
  **7 tests, zero failures, one retained pinned-descendant exclusion**, in **2.63 seconds**.
- The isolated bundled app passes the generic Inspector test: actual file/path, window/readiness,
  every stop/restart value, Argument/Environment creation and editing, conditional visibility,
  inherited environment, clearing and Undo/Redo. Public reads verify exact committed values;
  no browser errors/rejected intents or process launches occur. The first fixture run inspected
  an exiting widget before its animation completed; awaiting DOM settlement corrected the test.

Binary identities and local command receipts are in the linked receipt. Native picker interaction,
standalone-workflow removal, Column/Cell embedding and shared-owner aggregate demand were open
at that milestone; the Cell slice below advances its owning boundary.
The prior custom-form browser fixture has been replaced by the typed-widget fixture; final
M08/3 acceptance still owes complete contextual CRUD/picker/native interaction coverage.

## Cell-owned app control — 2026-11-03

Cell v2 declares the reusable typed control subtree, including integer `pre`/`post` Parameters.
`EnsureCell` materializes an empty address without choosing defaults in the UI;
`ConfigureCellApplication` creates/configures that address atomically. A compact Matrix action
selects the real Cell and exposes ordinary File/control/collection widgets. Inactive controls
and Argument/Environment children hide when App file is empty. Authoring launches no process
and adds no standalone catalog entry. Embedded settings cannot be removed or manipulated with
standalone CRUD; they copy and retire with their Cell/Column owner.

The persistent Cell UUID is the application consumer identity. Initial regression failures
showed that default declared-child UUIDs can disappear during sparse encoding; using the actual
Cell identity fixes default reload/history/copy/caller parity without persisting an extra UUID
or changing Golden's codec. Internal settings identities remain private. Cell v1 is explicitly
rejected before recoverable decoding; Rust-generated DTOs and all consumers change together.

Logical and render demand use one plain binding projection. Configured Cells contribute relative
to their Column position with clamped, non-wrapping pre/post ranges, including media-free Cells.
Direct Parameter changes invalidate the owning demand capture; unconfigured materialization and
source-only edits retain the existing no-rescan contract. Jumps, reorder, logical disable,
deletion/Undo and copied identities pass. Current clips accumulate new controls for retention;
the complete revised direct-trigger/disabled-render/outgoing/final-use matrix remains required.

[Verification receipt](evidence/mapgyver-m08-cell-control-2026-11-03.json):

- **368 library tests pass**, zero failures, two existing qualification exclusions. Five new
  cases cover sparse/default ownership, one-step history, project replay, range rejection,
  standalone-CRUD protection, bounded Column-relative demand, reorder/disable/delete/Undo,
  independent copies/schema rejection and UI/generic/hosted script parity across the four
  existing caller modes. New dedicated named methods/commands/snippets remain in M08/4.
  The caller fixture also explicitly uses an initially absent address.
- Strict library/all-test Clippy passes with native/process-fixture features. Svelte reports
  zero errors/warnings; touched UI/fixtures, both Cargo formatters and generated-contract tests pass.
- After inspected software and executable-build success, the Windows suite passes **8 tests,
  zero failures, one retained pinned-descendant exclusion**, in **2.86 seconds**. The new case
  starts an actual owned process from a media-free Cell, preserves PID/launch from Neighbor to
  exact-Column demand, then verifies completed retirement outside its range, without a renderer.
- The isolated bundled app passes actual Cell preparation and generic File/pre/post/Argument
  edits, inactive visibility, Undo/Redo, owner identity, zero standalone catalog entries and no
  authoring process side effects. Existing application-widget UI regression also passes.
  Zero browser errors/rejected intents occur. The initial Cell UI fixture used an obsolete
  channel-row helper; selecting the current Media Manager object corrected that fixture.

**No complete M08 gate closes in this slice.** Column-owned controls, standalone/association
removal, cross-consumer normalized-key sharing, independent Cell triggering and full retained
transition/final-use behavior remain in the same active gate. Native picker/complete contextual
CRUD, app-owned command/snippet coverage and retained access/retirement cases remain M08/3–4.

## Column-owned configuration and standalone removal — 2026-11-03

The app-owned `ColumnNode` declares `mapgyver.matrix.column.v2`, with a real app-control
child and typed transition override, whole-millisecond duration (0..u32::MAX) and curve.
Its public application identity is the persistent Column UUID, matching Cell ownership.
`ConfigureColumnApplication` authors settings and integer pre/post in one transaction;
`ConfigureColumn` edits only transition Parameters. Column/cell config uses one demand
projection, including empty media-free addresses and logical headless playback.

The standalone Applications panel, catalog, creation/rename/remove/duplicate/enable operations,
association picker/list and their generated DTOs are removed. Current read/configuration
operations resolve only Column/Cell owner identities. Declared settings cannot be detached,
duplicated separately or separately disabled; copying/removing the consumer owns their lifetime.
Project admission rejects old opaque Column and standalone app-control records before Golden's
recoverable codec can omit them. This deliberately breaks the retired document/API contract.

The Matrix contextual action selects the actual Column in the shared generic Inspector.
Ordinary File, integer distances and transition widgets replace the custom association form.
The four existing public caller modes retain configuration/read/rejection/history coverage.
Obsolete catalog/association-specific tests are replaced with intrinsic-owner cases, including
copy/project/history, inherited disablement, stale capture fencing and read-only absence.
[Verification receipt](evidence/mapgyver-m08-column-control-2026-11-03.json): **359 library
tests pass**, two retained qualification exclusions; strict library/all-native-test Clippy,
Rust-generated contract equality, Svelte (zero errors/warnings), 26 app/Matrix UI tests and
formatting pass. The count changes from 368 because retired catalog/association tests are
removed or consolidated into current owner cases; acceptance denominators do not change.

The isolated bundled app passes actual Column File/pre/post/transition editing, exact Undo/Redo,
no Applications panel, UI neighbor/exact launch with the same PID/launch UUID and completed
retirement outside its range. The Cell generic Inspector fixture passes on that same binary.
Both have zero browser errors/rejected intents, and ordinary source/history/Save-New-Load parity
passes. The first Column fixture incorrectly assumed demand reasons were in the runtime DTO;
using committed playback and the generated actual status contract corrected only the fixture.

The freshly linked Windows process suite passes **8 tests**, zero failures, one retained
pinned-descendant exclusion, in **1.16 seconds**, after inspected software/strict compile and
isolated app-build success. Cargo integration build commands also attempt to publish the normal
app binary, which Windows refuses while the user's app is open. Those commands are recorded as
failed publishing attempts, not successful `--no-run` checks; the linked test artifact, strict
all-native-target compilation and successful isolated app build are recorded separately.

This milestone does not implement cross-consumer normalized-key deduplication. The former
native two-Matrix/same-standalone-ID fixture becomes a Column neighbor/exact activation fixture;
runtime tests still cover multiple reasons for one consumer. Cross-consumer/cross-Matrix sharing
is explicitly unaccepted until the next implementation slice supplies it.

## Shared normalized runtime ownership — 2026-11-03

`runtime/grouping` now assigns immutable normalized keys to physical workers and reconciles
the entire committed consumer set together. Equal Column/Cell settings across Matrices use one
process. A consumer handoff preserves runtime/PID/launch identity; an effective setting change
splits only the changed lease, and matching settings rejoin the existing owner. Outgoing changed
settings retain the previous key and report Pending for that consumer without misreporting
another consumer still using Current settings. Missing retained definitions cannot authorize
new process creation. Admission after suspended creation checks for a current matching lease,
preserving project, source and capture fencing without tying a shared launch to a removed founder.

The limits now distinguish **4,096 consumer bindings** from **64 active/retiring runtime keys**.
This changes the previous incorrect one-worker-per-consumer accounting, not the physical worker
budget. The admission fixture uses 65 genuinely different keys and still rejects before any
worker/process is started. Five pure grouping cases exercise 4,096 equal consumers, independent
setting changes/handoff, invalid versus current leases, missing retained settings and per-consumer
Pending/Current projection. Existing fake-process/time and stale-scope coverage remains in place.

`ApplicationRuntimeId` is separate from authored `ApplicationId`. Process status/callbacks identify
the ephemeral worker; runtime snapshots add current consumer IDs, saved revisions/instances and
configuration states. Immutable membership is captured once per reconciliation, not rebuilt in
the process polling loop. Lifecycle events carry only `ApplicationProcessStatus`, preserving
the fixed per-launch callback reservation and preventing consumer-count event fan-out. Rust owns
and generates all changed DTOs. The Inspector shows the selected binding's state and total users.

Final-plan retention resolves consumer IDs into runtime IDs at the admitted render boundary.
Several matching consumers charge one owner/lifetime entry; subsequent consumer edits cannot
redirect an unfinished plan's lease. A physically retained owner can have zero current consumers.
Runtime identities/observations remain absent from authored project/history data.

[Verification receipt](evidence/mapgyver-m08-runtime-sharing-2026-11-03.json): **364 library
tests pass**, two retained qualification exclusions; strict library/all-native-target Clippy,
exact Rust-generated TypeScript, Svelte (zero errors/warnings), 26 app/Matrix UI tests and formatting
pass. **Nine actual Windows process tests pass** in **1.42 seconds**, one pinned-descendant case
still excluded. The new case shares one PID across a Column/Cell/two Matrices with equivalent
path/environment spelling, preserves it through neighbor/exact and cross-Matrix release, splits
one changed consumer into a second process and rejoins the original without restarting its peer.
The existing unfinished-render/project-close case now has two consumers sharing one process and
proves it survives until the renderer's explicit completion signal. This uses the existing null
renderer completion oracle; it does not claim hardware GPU completion qualification.

The isolated bundled app passes the same three-consumer/one-PID contract, live Inspector runtime
identity/count/configuration, actual grid-header launches and final range retirement. Cell generic
authoring and source/history/Save-New-Load regressions pass on the same binary, with zero browser
errors/rejected intents. The initial combined fixture expected a legacy Cut button after selecting
a materialized Cell; the generic Cell Inspector correctly replaced that form, so the fixture now
uses the existing Column header launch actions. No product change was needed for that correction.

**No complete M08 gate closes here.** The current key-sharing capability is verified, while the
independent Cell playback and complete revised demand/retirement acceptance remain required.

## Independent Cell playback — 2026-11-03

`LaunchCell` commits one Layer request while retaining the current Column anchor and other Layers.
Sparse per-Layer mixes preserve timed/interrupted weights; later Column launches start from each
Layer's actual mix. Cell transition override is a typed Parameter declaration and precedes Column
override/current Matrix fallback. The required typed Layer-duration conversion remains M07.
Disabled Cells skip direct activation and hold previous content on Column launches; enabled empty
addresses fade toward transparency. Request epochs and source-version fences remain ephemeral.

One bounded source-binding cache serves all mixes. Per-Layer/clip app-reference retention prevents
an outgoing Cell's app from disappearing when its Column is deleted, and releases it at that
Layer's own fade deadline. Logical direct demand works with no Column anchor, renderer or clock;
it does not move neighbor/preload ranges or launch the other Column's control. Immutable cached
Layer maps/stamps/app arrays avoid whole-Matrix binding copies per Layer. The separate authored
configuration cache preserves the existing logical no-render-index contract.

[Verification receipt](evidence/mapgyver-m08-independent-cell-2026-11-03.json): the full library
suite passes **368 cases**, two retained exclusions. An additional outgoing/deletion test and its
four-case Cell suite pass afterward (**369 distinct library cases exercised**, no product-code
change between those runs). Strict native-target Clippy, Media/Preview generated equality,
Svelte (0/0), 26 UI tests and formatting pass. **Ten Windows process tests pass** in **1.15 seconds**,
one retained pinned-descendant exclusion, including direct no-anchor launch and completed unload.

The bundled app passes actual double-click Cell activation, unchanged Column anchor, only the
Cell's process consumer, independent requested markers and actual Unload retirement, alongside
shared-runtime/Cell Inspector/source/history/Save-New-Load regressions. Browser errors/rejected
intents are zero. The marker check waits for the asynchronous viewport read after backend demand
settles; its initial immediate DOM read raced that update. Native Layer pixels and complete
Matrix UI/typed-model qualification are not claimed by these headless/source-weight oracles.

**M08/2 remains Active.** Complete the disabled-render/outgoing/final-use matrix through interruption,
reorder/deletion/Undo and project changes before closing it. M07 retains typed Layer duration,
complete Layer/Cell transforms, revised grid/defaults and native/scale acceptance.

## Exact remaining M08/2 requirement

Preserve verified independent activation, clamped/non-wrapping ranges and shared normalized owners;
finish complete disabled-render/outgoing/final-use retention through jumps,
reorder/delete and project/history operations. Complete fake-time/process, UI/public caller
and project verification before accepting M08/2. M08/3–4 retain their complete actual-process,
picker/API/script/callback/snippet, target-outcome and retirement/access acceptance.
