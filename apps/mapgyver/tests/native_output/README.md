# Native Screen acceptance

This fixture uses the ordinary MapGyver app and public media/Screen operations. Golden owns
the native windows, display catalog and Spout probe; MapGyver owns calibration and editor policy.
Finite probe readbacks run in a separate test process, never in the production frame path.

For stable output bounds (M05/9), use the ordinary isolated review binary:

```powershell
cargo rustc --locked -p MapGyver --bin MapGyver -- -C extra-filename=-codex-review
$env:MAPGYVER_TEST_BINARY=(Resolve-Path target/debug/deps/MapGyver-codex-review.exe).Path
node apps/mapgyver/tests/native_output/bounds.mjs target/mapgyver-output-bounds-RESULT
```

This finite fixture uses public authoring and Undo, plus read-only observations of its own
PID's native output HWND, physical client bounds, visibility and composited red center pixels.
It verifies position-only session preservation, resize retirement, both local display targets,
return to rectangle and disabled-window retirement. One creation across all updates rules out
window replacement. Checkpoints do not establish continuous scanout, DPI-matrix or latency gates.
The Python observer never changes focus, geometry or another application's windows.

Build from the repository root:

```text
cargo build --locked -p MapGyver --features native-qualification --bin MapGyver --bin native_qualification --bin native_output_launcher
node apps/mapgyver/tests/native_output/check.mjs --acceptance
```

The flow reserves its localhost port, creates private app data and prints `launch_required`.
Use the supported computer-use API to launch the printed `native_output_launcher.exe`, then
select its returned **child MapGyver window**. The windowless launcher may itself have no
targetable window. It starts the child at the desktop helper's integrity level, retains its
process handle, publishes status atomically and enforces a watchdog. It never invokes a shell.
App-data and local-data paths remain under the fixture's target directory.

Each `inspection_ready` checkpoint writes `acceptance-state.json`. Observe the actual editor
and both physical outputs before creating the corresponding `continue-PHASE` file:

- `initial`: open a real Screen editor and observe the concurrent bar outputs.
- `editor-open`: minimize the app after observing that editor.
- `minimized`: observe changed grid outputs, restore the app and close the editor tab.
- `editor-closed`: observe source output after editor closure.

The flow independently receives Spout frames and writes one bounded `pixels.png` per phase.
These PNGs still require reference comparison. Physical screenshots and actual UI observations
are separate evidence; the script never claims them from API success or continuation files.
Do not advance a checkpoint as if input succeeded when activation/input failed. Follow the
computer-use recovery limits and retain the failure. Normal app exit is checked separately
from complete app/driver resource qualification.

The legacy `--rectangle` and `--fullscreen --catalog FILE` flows remain API/delivery checks.
Neither mode qualifies physical pixels, DPI, input latency, cadence or complete M05 acceptance.
