# Native Screen acceptance

This fixture uses the ordinary MapGyver app and public media/Screen operations. Golden owns
the native windows, display catalog and Spout probe; MapGyver owns calibration and editor policy.
Finite probe readbacks run in a separate test process, never in the production frame path.
The shared browser launcher suppresses fullscreen and rectangle outputs by default.
Fixtures that measure physical outputs explicitly opt in with `allowPhysicalOutputs`.

The portable macOS check runs with `MAPGYVER_TEST_BINARY` pointing to the bundled MapGyver
executable. Run `node apps/mapgyver/tests/browser/portable-rectangle.mjs` in a desktop session
with Screen Recording access for the test launcher. The observer captures only the test
process's output window and samples its center in sRGB. It verifies authored color updates
and output retirement and reopening for Rectangle and Fullscreen. Physical monitor hotplug
and scanout cadence require a connected display session.

The same portable check covers Rectangle and Fullscreen on Linux X11 with `x11-utils`,
Python 3 and the Xlib runtime. Run it with `xvfb-run -a -s '-screen 0 1280x1024x24'` and
`GDK_BACKEND=x11` for a single active TrueColor output. The observer reads the owned X11
window's center pixels through Xlib and checks exact bounds, live edits and retirement.
Fullscreen bounds are compared with the independently queried X11 root.

The native Wayland fixture starts its own Weston compositor and two virtual outputs. With
`weston` and `xdotool` installed, run
`xvfb-run -a -s '-screen 0 1600x1200x24' bash apps/mapgyver/tests/native_output/check-wayland-output.sh`.
MapGyver uses its Wayland backend. The fixture reads pixels from Weston's two X11 host
windows to verify the selected output, live color edits, retargeting, retirement and reopening.
For 2× scaling, use a `2560x1200x24` Xvfb display and pass `2` to the script.
Pass `1 2` or `2 1` for two different output scales. The fixture compares each output's
observed device-pixel bounds with its configured scale before checking the pixels.

With `sway` and `grim` installed, run
`bash apps/mapgyver/tests/native_output/check-wayland-recovery.sh`. The script owns an
isolated Sway session and checks the selected output's pixels and its native window.
It disables and re-enables the connector with the project open, then repeats the check
after reopening the saved project while that connector is offline.

With the Xorg dummy driver, `xrandr`, `xauth` and the X11 pixel observer installed, run
`bash apps/mapgyver/tests/native_output/check-x11-recovery.sh`. It applies the same saved
project checks to a private Xorg server, verifies the active RandR output and checks the
owned window's exact bounds and pixels.

For current unattended concurrent-output acceptance (M05/4):

```powershell
cargo build --locked -p MapGyver --features native-qualification --bin native_qualification
cargo rustc --locked -p MapGyver --bin MapGyver -- -C extra-filename=-codex-review
$env:MAPGYVER_TEST_BINARY=(Resolve-Path target/debug/deps/MapGyver-codex-review.exe).Path
node apps/mapgyver/tests/native_output/concurrent.mjs target/mapgyver-concurrent-output-RESULT
```

The fixture runs fullscreen and rectangle on the two available local displays alongside an
independent Spout SDK receiver. It changes source colors after closing all native preview panels,
minimizes through the real titlebar control, restores through the existing host window command,
and independently verifies OS minimized state, physical output pixels and complete Spout pixels.
It needs neither a manual continuation file nor a new host test backdoor. New retires the outputs;
the fixture checks normal owned app/consumer shutdown. Wider physical/timing gates remain separate.

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

The older checkpoint flow below reserves its localhost port, creates private app data and prints `launch_required`.
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
