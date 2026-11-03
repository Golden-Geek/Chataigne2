# Reusable app-control configuration

**M08/2 remains Active.** The first configuration milestone is implemented and functionally
verified: equivalent launch definitions preserve the existing owned process rather than
triggering a stop/restart. This does not accept the full configuration/demand gate.

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

## Exact remaining M08/2 requirement

Replace the standalone Applications/catalog association workflow with reusable **typed**
Column/Cell App file and conditional controls, including real argument/environment child nodes.
Aggregate identical normalized keys across consumers, Cells, Columns and Matrices without
transient process restarts. Implement clamped non-wrapping Cell pre/post distances relative
to Column position, direct activation and outgoing/final-use retention through jumps,
reorder/delete and project/history operations. Complete fake-time/process, UI/public caller
and project verification before accepting M08/2. M08/3–4 retain their complete actual-process,
picker/API/script/callback/snippet, target-outcome and retirement/access acceptance.
