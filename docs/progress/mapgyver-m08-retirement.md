# M08 native retirement barrier

Updated: 2026-11-05. **M08/4 remains Active and unaccepted; M08 is 3/4.**
Golden IO now captures ordinary debug-chain process births before user execution. The retained
x64 ordinary-app singleton case reaches Idle with its external singleton preserved. This is a
delivered ownership correction, not complete Windows retirement qualification.

Creation failures now retain a returned target through `CreateError::into_parts`: its optional
`OwnedProcess` permits retirement, never resume. MapGyver publishes that PID and keeps the process
and worker lifetime through Stopping/RetirementUnverified, preventing retry or replacement before
proof. Pre-creation errors keep their OS code and normal backoff. Ownership mapping/drop tests and
three fake-time creation/replacement cases pass, as does a native forced post-create verification
rejection: error 5 retains the real owner, retirement completes and target code never executes.
This is forced verification rejection, not a claim of a genuine post-create OS access denial.
The [creation-owner receipt](evidence/mapgyver-m08-create-owner-2026-11-05.json) pins the new
source/tests: 72 IO software tests, 441 app tests, strict all-target/all-feature lint, 13 IO native
checks (WOW64 explicitly ignored), all 14 native app runtime tests, native pre-resume cancellation
and the forced post-create rejection. Root assignment remains atomic in this milestone.

## Ownership boundary and correction

`golden_io::owned_process` owns explicit suspended creation, atomic job-list assignment, native
resources and completion observation. MapGyver retains demand/readiness/restart/project policy.
The creating native thread owns `DEBUG_PROCESS`, event wait and continuation. It verifies job
membership and duplicates each descendant's creation-event process handle before continuing.
Exit events are continued before actual duplicated-handle completion is checked. The observer
must stop successfully; job active/total accounting remains an independent barrier. Drop explicitly
terminates the job without joining the observer. No pre-existing process is attached or adopted.

Windows owns debug-event process/thread handles; the observer closes image/DLL file handles.
Its native/WOW64 loader handshakes are one-time and primary-thread/first-chance guarded.
Application exceptions pass through. Observation errors retain truthful failure and terminate the
owned job. Runtime messages now also expose `observer_complete`, bounded to 512 characters.

This is visible debugging and can affect target behavior/performance or conflict with another
debugger. Protected/anti-debug targets and new descendant debugging chains have no fallback.
The [public primitive documentation](../../crates/golden_core/runtime/io/src/owned_process/README.md)
records these limits and Microsoft's process-event/thread-affinity/initial-breakpoint contracts.
Ordinary completion-port notifications remain rejected as best-effort; no active-zero, PID/name,
longer-timeout or fabricated Idle shortcut has replaced a process-completion barrier.

## Verified capability

The x64 primitive cohort requires actual retirement, removing earlier allowances for missed
transient lifetimes. It covers a child that exits before caller observation, sixteen sequential
transient children plus root exit, root-before-descendant exit, pinned root/branch/leaf completion,
drop before/after resume, graceful hidden-window close, cross-thread public callers, preserved
application exception exit codes and an untouched same-name external process.

The ordinary isolated MapGyver host passes the previously failing singleton (exit 17, Idle),
launcher-child retirement, breakaway denial (5), missing image (2), Execute denial (5), invalid
image (216) and elevation-required denial (740, no target main). Save/New/Load retires known
trees, rejects stale callbacks, recovers admission and shuts down normally while preserving
external processes. Bundled script editing executes all eight named methods, shares a Column/Cell
PID, reports ordered Starting/Ready/Stopping/Idle callbacks and executes typed/named unload.
The complete 14-test native runtime cohort also passes real Spout sender/first-image readiness,
crash/backoff, sharing, final-render retention and project cleanup. Native Agent pre-resume
cancellation passes without executing target code. The [current receipt](evidence/mapgyver-m08-observer-2026-11-05.json)
pins builds and counts: 69 Golden IO software tests, 438 app tests, strict lint, 13 native IO tests
(one explicit WOW64 ignore), 14 native runtime tests and one native cancellation check. The two
other app-library ignores retain separate scale/display-catalog scope; native cancellation ran separately.

## Remaining requirements and bounded return

**WOW64 remains unverified.** A quiet 32-bit Windows WHERE check first exhausted retirement's
three-second wait. A corrective loader-handshake implementation then returned:

```text
root=Exited(2), active=0, pending=0, unobserved=1, observer_complete=true
```

That second run also used a mixed-separator WHERE pattern containing a forward slash. The fixture
now uses the [documented plain filename search](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/where)
(`/q cmd.exe`) with the working directory explicitly set to Windows System32. It has not run;
the cause of the earlier exit 2 is unproven. Neither run
establishes ordinary 32-bit execution or complete retirement. The uncaptured accounting lifetime
is real; its origin is unknown. Do not infer that loader handling alone fixes it or that an
application exit, empty job or stopped debugger proves every job lifetime completed.

Both runs exhaust this question's baseline/corrective allowance. The explicitly ignored native
test preserves the corrected fixture and its open requirement. No third unchanged probe, another
counter/notification variant or silent budget reset is the next step. Inspect the native
creation/accounting boundary and establish an owning correction or documented evidence that
changes the mechanism decision before replaying that fixture under the original budgets.

The accounting contract explicitly includes failed associations in `TotalProcesses`; it is not
an executed-child count. That does not identify the missing lifetime here or authorize subtracting
it. A candidate owning correction is assignment of the returned root while both primary-thread
suspension and the uncontinued creation debug event fence user execution, before any resume or
event continuation. This preserves job-before-execution without enrolling creation-time candidates.
The typed creation-failure prerequisite is implemented and verified. The current atomic job-list
launch remains unchanged. Next implement the fenced assignment itself, including a typed rejected
pre-execution root whose exact handle remains owned even if job assignment fails. Its root-only
retirement must not pretend that an unassigned root is a job member or resume rejected code.
Then replay the corrected WOW64 case after software prerequisites; this requires the actual owning
boundary correction, not a third unchanged probe or a claim about the previous failure's cause.
See [job accounting](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information),
[assignment](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject)
and [creation event fencing](https://learn.microsoft.com/en-us/windows/win32/debug/debugging-events).

**Fresh elevated execution is pending.** The current development caller is not elevated and
truthfully verifies 740 without requesting elevation. The user's prepared Administrator
PowerShell check is pending. The [historical elevated result](evidence/mapgyver-m08-elevated-target-2026-11-01.json)
used the old primitive; its former source-equivalence reuse no longer qualifies the new observer.
An already-elevated invocation must verify the owned target's elevated token, Ready/Idle,
retirement and external preservation. No security settings or privilege fallback is introduced.

The [previous failure receipt](evidence/mapgyver-m08-retirement-2026-11-04.json) retains the original
x64 singleton gap and access evidence. Current results supersede its unchanged-native-code claim.
M11 remains complete. M10/M09 remain gated on complete M08/4 acceptance; the M03 32 MiB growth
failure stays Pending with its original requirement. Overall remains **57/67 (85%), 7/12 phases**.
