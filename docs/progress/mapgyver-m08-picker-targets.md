# M08 native App file picker and target outcomes

Updated: 2026-11-04. **M08/4 remains Active; no complete gate is accepted here.**

The actual bundled Windows app now has verified **Column and Cell App file selection** through
Golden's native Open dialog. Computer Use selected only the existing owned process fixture.
For each owner, the backend received the selected typed File setting, conditional controls
appeared, toolbar Undo/Redo restored the setting and native Cancel left the complete application
snapshot/revision unchanged. Selection launched no process. The app reported no page errors or
rejected intents and exited normally. This replaces the previous manual-path-only picker limit.

`tests/browser/native-application-picker.mjs` prepares the isolated app and checks these outcomes;
Computer Use operates each actual Windows dialog without intercepting the picker result.
The runner prints the exact next selection/cancellation and uses bounded waits. It is an
operator-assisted native test, separate from the fully automated browser File input test.

## Target outcomes through current public operations

The target/elevation fixtures now use intrinsic Column application IDs and aggregate runtime
consumer identities. The existing ordinary headless driver exposes explicit
`--application-target-outcomes` and `--application-elevation-required` flags; no old standalone
Application or association calls are used by these routes. The historical `headless/check.mjs`
and its separate project-replacement route still require reconciliation before reuse.

| Actual outcome                 | Evidence                                                                                                                                                                                                                                           |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| External singleton             | External PID 35756 survived; duplicate exit code 17 was not adopted or declared sender-ready.                                                                                                                                                      |
| Launcher root exit             | Root 21056 exited with code 0; owned child 19844 retired; runtime reached Idle.                                                                                                                                                                    |
| Breakaway                      | Owned PID 35692 received Windows error 5; no escaped fixture was created; unload reached Idle.                                                                                                                                                     |
| Missing/invalid image          | Errors 2 and 216, no process identity, no retry with restart disabled; unsupported `.cmd` configuration rejected atomically.                                                                                                                       |
| Elevation-required image       | Verified embedded `requireAdministrator`; caller token was not elevated. Error 740, one stable failed launch, no target-main execution, no elevation request/fallback; unload reached Idle. An actually running elevated target is **unverified**. |
| Singleton duplicate retirement | **RetirementUnverified** with retained identity and no scheduled retry. OS PID absence is observed, but complete owned-tree retirement is **unverified**.                                                                                          |

The first native baseline required complete Idle retirement and failed on that singleton result.
The second and final experiment recorded the original truthful unverified outcome, preserved the
open requirement and completed the remaining target checks. It did not repair or waive ownership
proof. The runner now labels this combined result **partial** when any retirement is unverified.
Production code, deadlines, process budgets and retirement semantics are unchanged.

Software prerequisites were the preceding milestone's 396 passing tests, strict all-target
Clippy, generated contracts, Svelte 0/0 and fresh bundled build. This test-only slice additionally
passes JS syntax/Prettier and both Cargo formatters; unchanged Rust is not recompiled.
[The receipt](evidence/mapgyver-m08-picker-targets-2026-11-04.json) pins the binary, fixture and
local logs. Both ordinary source regression and normal host shutdown passed.

Next: finish ordinary-app script/callback/command lifecycle and current project/shutdown/recovery
tests, then resolve the reproduced singleton retirement barrier at its owning boundary. Retain
actual running-elevated/access-denial and all other unverified acceptance explicitly.
Counts remain **47/67 gates (70%), 6/12 phases (50%), M08 3/4**.
