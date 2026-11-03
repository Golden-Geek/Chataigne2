# Reusable app-control configuration

**M08/2 remains Active.** The first configuration milestone is implemented and functionally
verified: equivalent launch definitions preserve the existing owned process rather than
triggering a stop/restart. The typed settings milestone below is also verified. Neither
milestone alone accepts the full configuration/demand gate.

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
standalone-workflow removal, Column/Cell embedding and shared-owner aggregate demand remain open.
The prior custom-form browser fixture has been replaced by the typed-widget fixture; final
M08/3 acceptance still owes complete contextual CRUD/picker/native interaction coverage.

## Exact remaining M08/2 requirement

Embed the verified typed subtree into Column/Cell App file and conditional controls, and remove
the standalone Applications/catalog association workflow.
Aggregate identical normalized keys across consumers, Cells, Columns and Matrices without
transient process restarts. Implement clamped non-wrapping Cell pre/post distances relative
to Column position, direct activation and outgoing/final-use retention through jumps,
reorder/delete and project/history operations. Complete fake-time/process, UI/public caller
and project verification before accepting M08/2. M08/3–4 retain their complete actual-process,
picker/API/script/callback/snippet, target-outcome and retirement/access acceptance.
