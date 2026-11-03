# Owned application runtime acceptance

**M08/3 Accepted — 2026-11-04. M08 is 3/4.** MapGyver owns normalized application demand,
readiness policy, project fencing and process/render leases under `apps/mapgyver/src/applications`.
Golden IO owns suspended creation, Job Object membership, resume and truthful root/tree retirement.
Settings remain real Column/Cell Parameters; [configuration acceptance](mapgyver-m08-configuration.md)
and [parameter ownership](../architecture/mapgyver-parameters.md) describe that boundary.

## Original requirement audit

| Requirement                    | Inspected current proof                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Actual owned-process readiness | `application_runtime` launches the owned executable, observes WaitingForSender through its startup gap, then Ready with unchanged PID/launch identity. Both `first_frame=false` and `true` run against an actual D3D11/Spout sender without a renderer, preview, injected evidence or Video clock. Exact metadata and completed-image probes verify the native path and bounded receiving storage.         |
| Timeout and crash/backoff      | An absent exact sender times out and retires its process without restart. Real exit code 23 drives three distinct launches and configured 150/300 ms capped backoff; no restart occurs before its deadline. Intentional unload cancels further retries. Fake-time tests cover resume failure and unverified retirement without premature replacement.                                                      |
| Shared-demand deduplication    | Column/Cell consumers in two Matrices share one PID/launch/runtime. Equivalent executable/environment spelling, neighbor→exact activation, consumer handoff and disabled destinations preserve identity. Effective arguments split only the affected consumer; rejoining retires the extra owner and preserves the original. Software grouping also covers 4,096 equal consumers and simultaneous handoff. |
| Launch cancellation            | Native Agent creates and Job-owns the actual target suspended. Admission changes before resume; the primary thread never executes its report-writing code. Suspended ownership remains until observed retirement, then process identity clears. No process-name adoption or unrelated process action.                                                                                                      |
| Final-render retention         | Actual owned processes survive Column/Cell removal, unload and project close while an explicit controlled renderer completion remains pending, then stop after completion. This is the production ChannelRenderer with its null backend as the deterministic completion oracle; it does not claim a delayed hardware fence. M07 separately verifies actual native render retirement.                       |
| Stale-project rejection        | Same authored IDs loaded into a fresh engine cannot reconcile the old runtime. Previous-launch and foreign-project readiness is rejected. Dropping the original engine cancels ownership and fences later evidence. Current callback/observation tests also reject retired project scopes and stale receipts.                                                                                              |
| Complete owner cleanup         | Pinned OS handles for root, branch and leaf all signal after forced stop and after original-project retirement, across two actual process trees. Every runtime retirement handle reaches idle within its unchanged deadline. Authored history/project JSON remains separate from OS side effects.                                                                                                          |

## Verification

[Receipt](evidence/mapgyver-m08-runtime-acceptance-2026-11-04.json): **393 library tests pass**,
zero failures, in **13.93 seconds**, using `native-qualification,application-process-fixture`.
Three explicit exclusions remain in that software invocation; native pre-resume cancellation is
then run separately and passes in **0.05 seconds**. The other two retain their existing owners.
Generated-contract equality passes; no DTO or production runtime changed in this milestone.

After terminal software/build success, the complete Windows lifecycle harness passes **14 tests,
zero failures/exclusions in 6.08 seconds**, including both actual readiness modes and the formerly
excluded pinned-descendant case. The initial baseline also passed all fourteen in 5.25 seconds;
the only test change makes sender-only and first-image runtime modes explicit before final closure.
Strict all-target native/process Clippy, scoped diff checks and both Cargo formatters pass.

Readiness uses the current default hardware-adapter route. GPU preference and other changed
feedback contracts remain M11. Process retention uses an explicit completion oracle; this does
not qualify global GPU/CPU/cadence, process memory, physical latency or artifact readiness.
M08/4 still requires complete picker/UI/script/headless commands/callbacks/snippets, target
outcomes, recovery/shutdown and retained access-denial/unverified-retirement cases.

## Reproduction

Build owned fixtures and inspect software success before native execution:

```text
cargo build --locked -p golden_io --features process-fixture --bin golden_io_process_fixture
cargo test --locked -p MapGyver --features native-qualification,application-process-fixture --lib
cargo test --locked -p MapGyver --features native-qualification,application-process-fixture --test application_runtime --no-run
cargo clippy --locked -p MapGyver --all-targets --features native-qualification,application-process-fixture -- -D warnings
```

Run the compiled library test executable with the exact filter
`applications::runtime::agent::tests::native_pre_resume_cancellation_keeps_suspended_ownership_and_never_executes_the_target`
and `--ignored --nocapture --test-threads=1`. Then run the compiled `application_runtime` executable
with `--include-ignored --nocapture --test-threads=1`. The receipt pins paths, hashes and logs.
