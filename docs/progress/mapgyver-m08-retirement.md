# M08 target and retirement acceptance

Updated: 2026-11-05. **M08/4 is Blocked on the retained WOW64 support/scope decision; M08 stays 3/4.**
All named Windows x64 target/access cases now pass, including fresh actual elevated execution.
The additional positive WOW64 retirement check remains unverified. No unknown entry is waived.

## Ownership and verified behavior

Golden IO owns explicit suspended creation, atomic creation-time private-job assignment, native
handles and completion observation. MapGyver owns demand/readiness/restart/project policy.
Creation debug events pin descendant handles before continuation. Actual handle completion,
successful observer shutdown and complete matched job accounting remain required. No process
name or PID absence authorizes adoption, killing another process or successful retirement.

The [creation-owner milestone](evidence/mapgyver-m08-create-owner-2026-11-05.json) retains a created
target on verification failure as a retirement-only owner. MapGyver publishes its PID, keeps
Stopping/RetirementUnverified and prevents retry/replacement or lifetime release before proof.
Its 72 IO software tests, 441 app tests, strict lint and 29 native checks remain valid.

[Fresh ordinary-app checks](evidence/mapgyver-m08-target-access-2026-11-05.json) pass:

- Singleton duplicate exits 17 and reaches Idle; its external singleton survives without adoption.
- Launcher root/child retire; breakaway is denied with error 5 and reaches Idle.
- Missing/Execute-denied/invalid images report 2/5/216; a standard caller reports 740 without
  executing the elevation-required target or requesting a fallback.
- Save/New/Load retires known trees, rejects stale callbacks, restores fresh identities,
  recovers admission and shuts down normally while preserving external processes.
- The bundled editor executes all eight named methods, shared Column/Cell demand, ordered
  Starting/Ready/Stopping/Idle callbacks, typed/named unload and authored history.
- The [native picker receipt](evidence/mapgyver-m08-picker-targets-2026-11-04.json) retains actual
  Column/Cell Open/Cancel, typed widgets, conditional controls and Undo/Redo. The authored UI
  and dialog behavior are unchanged by IO ownership work.

The [control API](mapgyver-m08-control-api.md), [app integration](mapgyver-m08-app-integration.md),
[configuration](mapgyver-m08-configuration.md) and [native runtime](mapgyver-m08-runtime.md)
reports own the complete command/snippet/caller/demand/readiness and final-render contracts.
Current ordinary-host checks and the complete fourteen-case native runtime cohort retain their
tested scope; they do not claim release or universal target compatibility.

## Fresh elevated execution is verified

The development token is restricted, but the account is a local administrator and the existing
Windows policy permits elevation without a prompt. A finite, hidden owned harness launched
through supported Windows RunAs, verified its own elevated token, and ran the current isolated
app under that caller. No security setting or credential/consent UI was changed or automated.

The owned target reported **PID 35464, elevated=true**, reached Ready, unloaded to Idle and
retired. The external process survived. Controller PID 31916 and the ordinary host exited zero.
MapGyver itself still uses explicit CreateProcess with no elevation fallback. The standard-token
740 case also passes. The [receipt](evidence/mapgyver-m08-target-access-2026-11-05.json) distinguishes
controller elevation from app policy and pins the executable, harness and logs. The earlier
manual elevation question is superseded; no user invocation is now needed.

## Retained WOW64 result and rejected correction

A complete corrected-WHERE cohort under a candidate suspended/debug-fenced assignment returned:

```text
root=Exited(0), active=0, pending=0, unmatched accounting=1, observer_complete=true
```

Thirteen native checks passed; this fourteenth case failed complete retirement. Assigned and
unassigned rejected-root checks and scope guards passed, but moving assignment after the returned
root did not close the accounting gap. That creation-time-only hypothesis is rejected. The
candidate was removed; production retains the verified atomic job-list implementation.

The candidate is preserved in stash `mapgyver-rejected-fenced-assignment-2026-11-05`, object
`6873844f8043fb644b253329425327ef654bf436`. Do not restore it automatically. The receipt pins
the failed cohort and candidate binaries. The original two probes and this changed-mechanism
qualification remain recorded; no unchanged replay, counter offset or renamed investigation
resets their limits.

[Microsoft's accounting contract](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information)
includes failed associations. An unmatched entry does not identify an additional PID or prove a
distinct/live child. Its origin here is unproven. The field's documentation now says so; the
strict zero-unmatched retirement predicate is unchanged. The corrected fixture remains explicitly
ignored for positive production qualification. Its exit-zero observation belongs to the rejected
candidate, not a claim of successful production WOW64 cleanup.

The plan targets Windows x64 and explicitly retains unverified-retirement/access-denial cases.
A scope question asks whether additional positive 32-bit cleanup must block M08/Video or remain
separate retained qualification while the named x64 matrix closes. No answer is inferred.
**Return trigger:** an explicit scope decision, or new authoritative identity-preserving evidence/
an owning correction that proves the missing WOW64 barrier, followed by its required acceptance.
M08 stays unaccepted meanwhile; M10/M09 remain gated. No ownership requirement is called optional.
