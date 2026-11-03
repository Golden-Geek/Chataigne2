# Reusable app-control configuration

**M08/2 remains Active.** The first configuration milestone is implemented and functionally
verified: equivalent launch definitions preserve the existing owned process rather than
triggering a stop/restart. The typed settings milestone below is also verified. Neither
milestone alone accepts the full configuration/demand gate. Cell ownership is verified below;
Column ownership/standalone removal is the current milestone. The complete aggregate-demand
contract remains required.

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
the existing stop/retirement/readiness/backoff rules. The runtime remains keyed by existing
consumer identities at this milestone; different consumers do not yet share one owner merely
because their normalized keys match.

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

## Exact remaining M08/2 requirement

Aggregate identical normalized keys across consumers, Cells, Columns and Matrices without
transient process restarts. Preserve the verified clamped/non-wrapping Cell ranges while adding
direct activation and complete outgoing/final-use retention through jumps,
reorder/delete and project/history operations. Complete fake-time/process, UI/public caller
and project verification before accepting M08/2. M08/3–4 retain their complete actual-process,
picker/API/script/callback/snippet, target-outcome and retirement/access acceptance.
