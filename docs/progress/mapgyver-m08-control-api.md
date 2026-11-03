# M08 app-control command and script milestone

Updated: 2026-11-04. **M08/4 remains Active; no acceptance gate closes in this milestone.**

Columns and Cells now support seven typed **App control** commands: Set App File, Launch Matrix
Column, Launch Matrix Cell, Unload Matrix and the three settings/runtime/callback inspections.
Eight named script methods expose those operations plus complete configuration. The installed
app embeds request examples and an event-driven lifecycle callback template.

MapGyver's applications layer owns file-only configuration and script intent projection;
the Medias command/script registration consumes that boundary. Golden remains app-agnostic.
File changes preserve every other typed setting, clear explicitly and use existing atomic
validation/history. Commands use real File/NodeReference/enum Parameters and the generic
Inspector. Script configuration JSON is a bounded transient argument, never authored storage.
Runtime launches and unloads remain ephemeral; authored Undo never replays OS side effects.

## Verification

- **396 library tests passed, 3 ignored, 0 failures** in 13.92 seconds. Coverage includes all
  40 command kinds, typed settings preservation, atomic invalid-file rejection, exact history
  and project reload, actual named-script/command/UI-intent/headless parity, malformed arguments,
  launch anchors and ephemeral unload. Existing lifecycle/template tests remain passing.
- Rust protocol generation completed; the generated application intent, command kinds and
  visible command fields match Rust. Strict all-target Clippy passed with
  `native-qualification,application-process-fixture` and `-D warnings`.
- Svelte check: **0 errors, 0 warnings**. Both workspace formatters pass.
- The freshly bundled ordinary app passes the new command-menu/Inspector workflow: all seven
  menu entries, typed owner and File controls, passive editing, Execute and exact toolbar
  Undo/Redo. The fixture executable is selected by text in this check; it is never launched.
  Existing two-source Resolution commands, rejection monitoring and headless source/history/
  Save/New/Load regression also pass; the combined host receipt reports 4757 ms and normal exit.

Binary: `target/debug/deps/MapGyver-codex-m08-control.exe`, SHA-256
`15E200A47A359834DADFC2BABE73D2B5437D8AD6622F32510E05756786D6FF52`.
Local logs: `target/m08-control-api-{codegen,tests,clippy,svelte,app-build,browser}.log`.
The [compact receipt](evidence/mapgyver-m08-control-api-2026-11-04.json) records the scope.

## Exact remaining acceptance

The actual native App file picker, complete UI/script/callback lifecycle integration,
singleton/external/elevated/launcher/breakaway outcomes, access/retirement failures and
project/shutdown/recovery cleanup remain M08/4. The older target-outcome fixture still uses
removed standalone Application/association APIs; reconcile it to intrinsic Column/Cell owners
before native execution. Reuse [M08/3 runtime acceptance](mapgyver-m08-runtime.md) where unchanged.
This API/UI milestone does not establish full native lifecycle or production qualification.
Counts remain **47/67 gates (70%), 6/12 phases (50%), M08 3/4**.
