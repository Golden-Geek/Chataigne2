# M08 native retirement barrier and target-access evidence

Updated: 2026-11-05. **M08/4 is unaccepted and selected Active to fix complete transient-lifetime observation.**
M11 is now accepted; this reproduced ownership barrier is the critical prerequisite before M10.

MapGyver's existing runtime message now explains why retirement is unverified: root state,
active job count, pending captured handles and unobserved lifetimes, or the actual observation
error. Messages stay bounded to 512 characters. Ownership, its global slot and the prohibition
on replacement/restart remain intact until the original complete-retirement barrier passes.
This corrects an observation gap in the product: previously the earlier root-exit reason hid
which completion requirement was missing. Golden IO's native ownership code is unchanged.

## Actual result and remaining requirement

The freshly built ordinary app reports the singleton duplicate as:

```text
Owned tree retirement unverified: root=Exited(17), active=0, pending=0, unobserved=1
```

That is a real uncaptured lifetime, not a snapshot error or a still-running captured process.
The external singleton survives and is never adopted. Root PID absence, job drain and normal
host exit do **not** close complete retirement. The first baseline reached this same evidence
but its access-fixture setup/cleanup failed on numeric SID syntax. The corrected final run
passes all target/access assertions and preserves the combined **partial** result. No third
native experiment or unchanged rerun is authorized by this batch.

The private access-denied fixture produces actual **CreateProcess error 5**, no PID, one stable
failure and no target-main report. Only Execute permission on that exact temporary copy is
denied; the added ACE is removed before deleting the fixture. The baseline's private directory
was also restored and removed. Missing/invalid images remain errors 2/216; breakaway denial,
launcher-child Idle cleanup and non-elevated requireAdministrator error 740 all pass.

Historical [actual elevated-target evidence](evidence/mapgyver-m08-elevated-target-2026-11-01.json)
verified the owned target's elevated token, Ready/Idle, cleanup and external preservation under
an already-elevated caller, without requesting elevation. Source comparison against its base
`194d888a` shows **no Golden IO owned-process changes**, and the Native backend methods are
unchanged. Reuse that platform/primitive evidence with current binding/runtime and denial checks;
it is not a claim that this current executable ran elevated. The remaining blocker is complete
singleton/transient retirement, not missing computer access or a missing elevation prompt.

## Rejected shortcut and bounded return action

A private completion port associated before launch was considered. Microsoft documents that
ordinary job-message delivery is best-effort and PID-bearing messages do not preserve identity
without an open process handle. It does not establish the required complete handle coverage.
See [the completion-port contract](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_associate_completion_port).
No notification-count, zero-active, longer-timeout, process-name or fabricated Idle workaround
is introduced. Actual [numeric-SID syntax](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/icacls)
was used to repair the private fixture, with no global security change.

Establish a documented identity-preserving completion mechanism addressing the uncaptured
lifetime before any new native experiment, or obtain an explicit user revision of the retirement
contract. Then run the retained exact fixture with the original budgets. Do not repeat unchanged
historical probes or present the current conservative state as complete cleanup. No ownership
requirement is optional or waived.

**Execution ordering:** M08's configuration/runtime, picker, commands, scripts/callbacks and
project/recovery/shutdown paths are functionally verified. M11/1–10 are accepted. M08/4 is now
the single Active gate under the plan's requirement to fix the reproduced ownership blocker
before Video. M03's failed private-growth requirement remains in the explicit return queue;
its budget is unchanged. Selecting this correction accepts no gate and weakens no ownership
or retirement requirement.

## Checks

398 Rust library tests pass in 12.66 s, 2 separate-scope tests are ignored and native pre-resume
cancellation is filtered (its prior native acceptance is unchanged). New tests cover each
missing barrier and bounded access-error reporting without replacement. Strict all-target
Clippy with native/process features, protocol equality, JS syntax/Prettier, both Cargo formatters
and the fresh app build pass before native execution. Unchanged Svelte/UI checks are reused.
The [receipt](evidence/mapgyver-m08-retirement-2026-11-04.json) pins binaries and the final results.
Counts stay **47/67 gates (70%), 6/12 phases (50%), M08 3/4**.
