# M08 ordinary-app lifecycle and project integration

Updated: 2026-11-04. **M08/4 stays Active for retained target/access/retirement acceptance.**

The ordinary host now verifies the current app-control contract through real script editing,
native process ownership, project operations and normal shutdown. These tests consume the
public app/project/script/Parameter APIs; they do not restore standalone Applications or
associations. Production code and process/retirement budgets are unchanged.

## Verified behavior

- The bundled script editor installs the embedded template, saves changed source and explicitly
  reloads its runtime. All eight named app-control methods execute. Actual lifecycle callbacks
  arrive in sequence for the same project: **Starting → Ready → Stopping → Idle**.
- A Column and independently launched Cell share actual PID **34792** and launch identity.
  The script inspects runtime consumers/callback state, executes a real typed **Unload Matrix**
  command, calls named unload and clears App file. The owned process retires; all four callbacks
  are acknowledged, with zero pending/faults. Source-script creation/history/callback regression,
  UI error/rejection monitoring and ordinary source/project regression also pass.
- An ordinary host rejects **65 additional unique normalized keys atomically**, exposes the
  exact 64-key admission error and preserves its existing owned PID/launch. Clearing that demand
  restores Running without restarting the existing process. Missing executable fixtures ensure
  the overflow scene cannot execute new targets, even if admission were to regress.
- **Save/New/Load** produces three distinct project identities. New retires the original owner
  plus a root/branch/leaf tree; Load retires the interim process and restores stable authored
  identity/settings with a fresh definition instance. Saved settings do not automatically launch.
  Three old-project callback tickets reject; the loaded project acknowledges fresh callbacks.
- **Normal host shutdown** retires a newly launched three-member tree. The independently spawned
  same-executable process survives admission failure, New, Load and final shutdown.

The project baseline passed, then the completed recovery/project route passed in a combined
2382 ms host receipt. Two UI setup checks failed before any target launch because the fixture
looked up a typed field using an incomplete declared path. Source inspection established
`parameters/media`; the corrected full script/process workflow passed in a 2922 ms host receipt.
These setup failures do not represent process-lifecycle failures and remain in local logs.

## Reproduction and limits

Reuse the fresh build and passing 396-test/strict-Clippy/Svelte prerequisites pinned by
[the command/script milestone](mapgyver-m08-control-api.md). JS syntax/Prettier, scoped diff
checks and both Cargo formatters pass for this test-only slice. Run from the repository root:

```text
node apps/mapgyver/tests/headless/media.mjs --application-lifecycle-ui
node apps/mapgyver/tests/headless/media.mjs --application-project-replacement
```

Set `MAPGYVER_TEST_BINARY` to the pinned `MapGyver-codex-m08-control.exe` for the exact build.
[The receipt](evidence/mapgyver-m08-app-integration-2026-11-04.json) owns project/PID identities
and logs. The historical `headless/check.mjs` remains obsolete for execution; the current driver
owns these migrated routes. Existing typed configuration/demand, native runtime and picker
acceptance remain in their owning reports.

Next: resolve the reproduced singleton complete-retirement barrier and audit the exact retained
elevated/access-denial requirements against observed native outcomes. The prior singleton result
is still **RetirementUnverified**; no complete gate or release readiness is claimed.
Counts remain **47/67 gates (70%), 6/12 phases (50%), M08 3/4**.
