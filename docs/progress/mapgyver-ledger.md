# MapGyver agent ledger

**Updated: 2026-11-22 · Overall: `█████████░` 86% · 9/20 phases complete**

**Accepted: 130/151 gates. Phase completion: 9/20 (45%). M19/1 is Accepted. M19/2–4 need hardware checks. M19/5 Raspberry Pi armhf is Active.**

Internal execution/evidence ledger. The user-facing checklist is `mapgyver-status.md`.

**M19/1 Accepted.** Apple Silicon built the default app with native FFmpeg, verified authoring,
Undo/Redo and Save/New/Load for three sources and six channels, and passed strict desktop
startup, workbench connection and shutdown. The initial HTML now declares the existing PNG icon.
[Apple Silicon qualification](https://github.com/Golden-Geek/Chataigne2/actions/runs/44933901297)

**M19/2 Media Preview verified.** Apple Silicon Metal and local DX12 passed Solid Color live
edits, Picture Fit, Fill, Stretch and relink, and H.264 playback, paused seek and relink. The
fixtures checked rendered color and alpha and observed renderer retirement. Apple Silicon also
passed the default build, project fixture and strict desktop readiness again. All 179 Golden
Video unit tests, strict Clippy, the default Windows compile check, both formatters, Svelte,
71 preview UI tests, Prettier and Actions lint passed across the Media Preview batches.
[Latest Apple Silicon Media Preview run](https://github.com/Golden-Geek/Chataigne2/actions/runs/44940636041)

**M19/2 Screen Warping verified.** Apple Silicon Metal and local DX12 rendered the expected
pixels inside and outside the projective overlap of two authored Surfaces. A live geometry
and opacity edit updated the same Screen. Both Rust formatters, the default Windows compile
check, Prettier and Actions lint passed.
[Apple Silicon Screen Warping run](https://github.com/Golden-Geek/Chataigne2/actions/runs/44942165608)

**M19/2 Rectangle lifecycle verified.** Apple Silicon passed the strict desktop smoke from
the completed build. WindowServer observed the authored 4:3 Rectangle in the same window
across a live edit, its withdrawal on disable and its return on enable. The fixture exited
normally with that window open. The Windows x64 DX12 portable build also verified actual
red and green output pixels and exact client bounds. All 179 Golden Video tests, 43 desktop
host tests, strict Golden Video Clippy, both formatters and the default Windows compile check
passed. The earlier underlay classification and redundant rebuild failures are resolved.
[Apple Silicon lifecycle](https://github.com/Golden-Geek/Chataigne2/actions/runs/44947707188)
[Underlay failure](https://github.com/Golden-Geek/Chataigne2/actions/runs/44944343028)
[Startup timeout](https://github.com/Golden-Geek/Chataigne2/actions/runs/44945975635)

**M19/2 Fullscreen routing verified.** On Apple Silicon the Screen Inspector received named
displays keyed by ColorSync UUID. WindowServer measured the Fullscreen window at the selected
display's bounds. The window survived a live color edit, retired when the selection was
cleared, reopened when restored and shut down normally. All 11 local catalog tests passed,
including exact recovery when the intended UUID returns. An identical replacement stayed
unassigned. The default Windows build, local DX12 Rectangle fixture and both formatters passed.
Native macOS output pixels, physical hotplug and cadence remain open.
[Apple Silicon Fullscreen](https://github.com/Golden-Geek/Chataigne2/actions/runs/44950133263)

**M19/2 macOS bundles verified.** The [packaging command](../../tools/mapgyver/ffmpeg/README.md)
built two ad hoc signed debug bundles with the pinned FFmpeg libraries. Both macOS 15 jobs
verified architecture, signatures and bundle-local Mach-O links after relocation. The
relocated apps passed desktop readiness, project authoring and persistence, H.264 playback,
seek and relink, Screen Warping, and native Rectangle and Fullscreen window lifecycle.
The Windows compile, workbench production build, both Rust formatters, shell syntax, Prettier
and Actions lint passed. [Apple Silicon and Intel CI](https://github.com/Golden-Geek/Chataigne2/actions/runs/44951750513)

**M19/2 native Mac pixels.** The fixture captures the owned WindowServer output and samples its
center in sRGB. Apple Silicon and Intel both measured red and green Rectangle pixels and green
and blue Fullscreen pixels at the selected display bounds. They verified live edits, retirement
and reopening. Local DX12 Rectangle, JavaScript syntax, Prettier and Actions lint passed.
Physical display cadence and hotplug require a Mac with a connected monitor. Verify that
output stops when the display is absent and returns to the same saved UUID.
[macOS pixel checks](https://github.com/Golden-Geek/Chataigne2/actions/runs/45019118144)

**M19/3 Linux desktop batch (22 November, 13:41 CET cutoff).** Enable the shared portable GPU
Media Preview in the default Linux MapGyver desktop build. MapGyver owns feature selection
and CI. Golden owns the desktop runtime and wgpu renderer. Validate the default binary in
an X11 desktop session with strict workbench readiness, project history and Save/New/Load,
plus local Picture and Video pixels through the browser preview. Linux packaging, native
Screen output and display recovery remain in this gate.
The first Ubuntu 26.04 compile exposed a direct reference to FFmpeg 9's animated WebP enum.
Golden Video now reads the descriptor captured at stream open. The default Ubuntu 26.04 build
passed. The strict workbench probe connected over WebSocket and exited normally on both Xvfb
X11 and WSLg Wayland. Headless authoring and Save/New/Load, Solid Color and Picture preview,
H.264 playback, seek, relink and Screen Warping passed with the portable renderer. The Linux
desktop host supports absolute placement on X11. Golden realizes the hidden GTK window on
the UI thread before GPU surface attachment. The Xvfb fixture captured red and green at the
authored 160,120 origin and 320×240 size. It verified same-window edits, retirement and
reopening. All 23 targeted Golden Video decoder tests passed with FFmpeg 8. The shared DX12
fixture, default Windows compile with FFmpeg 9, Rust formatting, Prettier and Actions lint
passed. Ubuntu 24.04 CI, Linux installation, Fullscreen display routing, Wayland output
and physical reconnect remain open.
The [Ubuntu 24.04 CI run](https://github.com/Golden-Geek/Chataigne2/actions/runs/45023316606)
built the default application and passed authoring and strict UI readiness. Its PowerShell
wrapper timed out waiting for desktop shutdown. The replacement uses a Node-owned desktop
process and requires exit code zero. It passed against the Ubuntu 26.04 release build with
the workbench ready, its WebSocket exchanging frames and a normal exit.

**M19/3 Linux package batch (22 November, 14:26 CET cutoff).** Produce a native Debian
package for the build distribution. MapGyver's packaging tools own the package and use
Debian's library metadata to declare the runtime dependencies of the linked executable.
Verify the package installs under apt in a fresh Ubuntu environment, then start the
installed workbench and decode local media. Linux Fullscreen and physical recovery remain
in the return queue for this gate.
The Ubuntu 26.04 amd64 debug and release packages installed with only declared dependencies
in fresh containers. Both passed the desktop launcher, license, ELF linkage, installed
project authoring and normal exit checks. Executables extracted from both packages passed
X11 readiness, H.264 playback, paused seek and live relink, Screen Warping and renderer
retirement. The debug package also passed X11 Rectangle pixels and bounds.
The [Ubuntu 24.04 package run](https://github.com/Golden-Geek/Chataigne2/actions/runs/45025967029)
built the Debian package and passed the clean apt install, ELF linkage, installed
project authoring and strict X11 workbench readiness. The Node fixture timed out
waiting for shutdown. It now observes the owned child's process exit independently of
WebKit's output pipes, requires exit code zero and includes native logs on timeout.
The updated fixture passed against the Ubuntu 26.04 release binary. Ubuntu 24.04
shutdown and the downstream packaged media/output checks still need verification.

**M19/3 X11 Fullscreen batch (22 November, 14:37 CET cutoff).** Select an active XRandR
output in the Screen Inspector and present Fullscreen at that output's bounds. Golden owns
enumeration and placement. MapGyver owns published choices and exact saved identities.
The default Linux build and 11 Screen catalog tests passed. The Xvfb fixture verified exact
Rectangle and Fullscreen bounds, live color edits on the same windows, retirement and
reopening. An independent Xlib read found green and blue on the 1280×1024 Fullscreen
window where FFmpeg's X11 capture returned black. The fixture now checks Xlib pixels on
both targets. Golden desktop Clippy lints passed. Treating all warnings as errors is
blocked by three existing unused WebView2 control items on Linux. The next deliverable
is native Wayland Fullscreen on a selected output. Physical monitor hotplug and scanout
cadence require a physical Linux display session.

**M19/3 Wayland Fullscreen batch (22 November, 15:05 CET cutoff).** Golden owns Wayland
output discovery and compositor-directed placement. MapGyver publishes addressable
outputs and routes an authored Fullscreen Screen to its exact selected identity.
After the Linux build passes, use an owned compositor session to verify output selection,
visible color updates and window retirement. Retain physical Linux recovery and cadence.
The default Linux build and existing X11 fixture passed. Weston 14 provided two native
Wayland outputs backed by Xvfb host windows. The owned app passed independent pixel
checks for choosing the second output, red-to-green edits, retargeting to the first,
clearing and reopening. An intermittent 2× disconnect during retargeting led to the
follow-up below.

**M19/3 Wayland retarget follow-up (22 November, 15:05 CET cutoff).** Keep an authored
Fullscreen Screen visible through repeated output changes under 2× scaling. Golden's
Wayland window host owns configure and map ordering. Weston reproduced an oversized
logical surface when the GPU attached its 1280×960 buffer before GTK committed scale 2
for a 640×480 logical output. The host now waits for a real GTK draw at the selected size
and scale, then hands the surface to the renderer after GDK's commit handler. Four runs
each at 1× and 2× verified pixels on both outputs through eight retargets, clearing and
reopening per run. The default app build, desktop Clippy lints and X11 pixel regression
passed.

**M19/3 mixed-scale Wayland batch (22 November, 15:25 CET cutoff).** Keep the existing
Fullscreen route stable while moving one Screen between a 1× and a 2× compositor output.
Golden owns per-output buffer extent and GTK handoff. Weston reported 640×480 and
1280×960 device-pixel extents for equal logical outputs at different scales. The fixture
passed at 1×/1×, 2×/2×, 1×/2× and 2×/1×. Each run verified exact output extents, colors
through eight retargets, clearing, reopening and normal process exit. CI runs all four
configurations. Ubuntu 24.04 desktop exit and packaged media/output remain open.
Return to physical Linux recovery and cadence on a Linux desktop with external monitors.

**M19/3 Wayland display recovery batch (22 November, 15:30 CET cutoff).** Preserve an
authored Fullscreen Screen and its selected output through a compositor output
disable/re-enable. Golden owns display discovery and window ownership. MapGyver owns
the saved target and renderer binding. Sway removed and re-advertised the connector
with a new Wayland object ID. Before the fix, presentation stalled when FIFO waited
for a frame callback on the disabled output, leaving the native window or shutdown
waiting. Golden Video now prefers mailbox and uses immediate mode when that is the
available alternative to FIFO on Wayland. Three Sway runs verified output retirement,
an unchanged authored target, resumed live green pixels, and the saved red pixels after
reloading while offline and re-enabling the connector. All runs exited normally. Weston
retargeting also passed all four 1×/2× configurations. This establishes compositor
output recovery. Physical reconnect and cadence still need external monitors.

**M19/3 X11 display recovery batch (22 November, 15:40 CET cutoff).** Preserve an
authored Fullscreen destination across an XRandR output becoming inactive and returning.
Golden owns display discovery and window lifetime. A private Xorg server with the dummy
driver verified DUMMY1 dropped from the active topology and the owned window retired.
Re-enabling it restored the same authored target, exact 640×480 client bounds at (640, 0),
and live green pixels. Save/New/Load while DUMMY1 was inactive restored the original red
pixels when it returned. The X11 recovery fixture passed twice, and the shared Sway recovery
scenario, four Weston scale configurations, and standalone X11 pixel checks passed. The
CI workflow runs the fixture against the installed package. Physical X11 hotplug and
cadence still need external monitors.

**M19/3 Linux release package batch (22 November, 15:45 CET cutoff).** Rebuild the
Ubuntu 26.04 amd64 release package with the current X11 and Wayland output recovery
code. MapGyver's packaging tools own the Debian runtime dependencies. Verify clean
apt installation, desktop launch and exit, packaged media, and both recovery fixtures
using the resulting binary. Ubuntu 24.04 remains under its own CI check.
The Weston CI command contained a stray `+` before `bash`. After correction, the exact
workflow shell step passed locally for all four scale configurations. The refreshed
Ubuntu 26.04 release package passed clean apt install, launcher and ELF checks,
Save/New/Load, owned native X11 window readiness and clean exit, H.264 playback and seek,
Picture and Screen Warping, and X11 and Wayland output and recovery. The package SHA-256 is
`a347fb1303a63622c165a39a1a2e067364e2fd2dbb20a3d5058efa3554a018e9`.

**M19/3 Ubuntu 24.04 installed desktop batch (22 November, 15:45 CET cutoff).** Qualify
the packaged Golden desktop host, MapGyver media, and Screen output on Ubuntu 24.04 using
native startup, project authoring, playback and X11/Wayland recovery fixtures. CI passed
clean package install and the external workbench probe, then the app timed out. Logs
showed an AT-SPI bus warning and no acknowledgement of the shutdown sentinel.
[Ubuntu 24.04 job](https://github.com/Golden-Geek/Chataigne2/actions/runs/45029020238/job/137229834543)
The fixture now requires the owned X11 window to map and its WebKit view to request the
workbench snapshot before the external Chromium probe.
CI retains the install-checked package before desktop validation for exact reproduction.
The locally built debug package passed clean apt installation and ELF linkage with GTK
3.24.41, WebKitGTK 2.52.6 and FFmpeg 6.1.1. As an ordinary user under Xvfb, its owned X11
window mapped, its native WebKit view requested the 63-node workbench snapshot, and the
app acknowledged the sentinel and returned zero. Package SHA-256
`813e73bdf46834813dad07e940058973a9169c33f5f654503fe75b2a33a30ee0`.
The installed package also passed project authoring and Save/New/Load, strict Chromium
readiness, H.264 playback and seek, Picture and Screen Warping, X11 Rectangle and
Fullscreen pixel checks, four Weston scale pairs, and Xorg and Sway output recovery
with the project open and after reopening it. The hosted-runner retest remains open.

**M19/3 Ubuntu 24.04 release package batch (22 November, 15:55 CET cutoff).** Build
the MapGyver production Debian package from the pushed source, then install it on clean
Ubuntu 24.04. Use the installed binary for project authoring, native WebKit startup and
exit, media playback and authored X11/Wayland Screen output and recovery. Monitor the
[hosted package retest](https://github.com/Golden-Geek/Chataigne2/actions/runs/45032174624).
The optimized amd64 package from `567481d1b` passed clean apt installation without
recommended packages, launcher and ELF checks, project authoring and Save/New/Load,
owned X11 WebKit startup and exit, media and Screen Warping, and all four Weston
scale pairs and Sway recovery. In a native Wayland session with XWayland disabled,
Sway reported the app's owned, visible `xdg_shell` workbench. WebKit loaded the
63-node project snapshot and the app exited successfully after emitting the same
AT-SPI warning as the hosted runner. While retiring a Screen, X11 `xwininfo` raced a destroyed
child and returned an incomplete tree. The observer now retries that error without
treating partial results as evidence of retirement. The same release package passed
the Rectangle and Fullscreen pixel and bounds retest with an injected partial query and
passed the Xorg recovery retest. Package SHA-256
`0ef1b3189fbdf141a12b57daad2721b454b0bd187645e95697da7230171a46a5`.

The [hosted Ubuntu 24.04 job](https://github.com/Golden-Geek/Chataigne2/actions/runs/45032174624/job/137241457758)
failed before mapping the native X11 window. Its retained debug package
(`e1924e97a6ee7963bae09c0b170c6d92783b08b171bfec27bcbba8a0e788b4c7`) passed the same
X11 workbench and shutdown check, media, X11 pixels and recovery, and all Wayland
fixtures in clean Ubuntu 24.04. CI now runs those desktop checks against the installed
package as an ordinary user in a clean Ubuntu image. The exact workflow script passed
locally against the retained artifact, including the pinned Chromium setup. The
[clean Ubuntu hosted job](https://github.com/Golden-Geek/Chataigne2/actions/runs/45035873220/job/137249983856)
passed the install, native workbench, media, X11 and Wayland output suites. The stock
runner's earlier launch failure is recorded above. Return to Linux qualification on a
desktop with a physical external output on X11 and Wayland to verify reconnect and
scanout cadence against the saved Screen.

**M19/4 aarch64 package batch (22 November, 16:40 CET cutoff).** MapGyver
packaging owns the native ARM64 Debian 13 build, install and workbench checks. Produce
an `arm64` package with the existing native FFmpeg and shared GPU backend, and check the
installed artifact on a native ARM64 runner in clean Debian 13. The actual Raspberry Pi
OS desktop, its native GPU and physical Screen output remain required on a Pi.
The ARM64 workflow and Bash entry points passed syntax and architecture checks.
As a Debian library and packaging check, the same builder passed on amd64 with FFmpeg
7.1.5 and WebKitGTK 2.54.3. The optimized package passed clean apt installation,
authoring and Save/New/Load, X11 workbench startup and exit, media and Screen Warping,
X11 output and XRandR recovery, four Weston scale pairs, and Sway output recovery.
[The first native ARM64 build](https://github.com/Golden-Geek/Chataigne2/actions/runs/45035872813)
produced an optimized `arm64` package and passed minimal Debian 13 installation, ELF
linkage, authoring and Save/New/Load. The artifact contains an ELF64 AArch64 binary.
The [Raspberry Pi ARM64 package index](https://archive.raspberrypi.com/debian/dists/trixie/main/binary-arm64/Packages.gz)
meets its declared minimum versions for the FFmpeg, ALSA, GTK, Cairo and libc packages
provided by the Pi archive. Package SHA-256
`870bec975d0b6c1e71fdcfb9bf7f868b0f83faff31e1c0e568d9c5efd4cd183a`.
Desktop test setup failed before launch because `npm ci` invoked Golden Audio codegen on
the runner without the ALSA SDK. The test now installs JavaScript dependencies with
`--ignore-scripts` and uses the installed package's workbench. A clean Ubuntu check
without development packages loaded Playwright with that flag. The
[ARM64 rerun](https://github.com/Golden-Geek/Chataigne2/actions/runs/45037577689)
passed build, install, project authoring and Save/New/Load, native X11 workbench and
normal exit, H.264 playback and seek, Screen Warping, X11 Rectangle and Fullscreen
pixels, all four Weston scale pairs, and Xorg and Sway recovery across reconnect and
project reopen. The downloaded package is ELF64 AArch64, the overlapping Pi archive
versions satisfy its declared minimums, and its SHA-256 is
`d10b3361b5943f8b2248112ef6ae89361c7867fa20dd327b91dae087d9c7d108`.
Return on a Pi with current 64-bit Raspberry Pi OS and an HDMI display to verify an
on-device build, installation, native workbench, media playback, and Screen output
and reconnect using the Pi GPU. The next software package is armhf.
The Windows 4× AA work is preserved at `89a68a941224c94277ac361b345e8a02a066076f`; return
when Windows qualification resumes and run its pixel and allocation checks before timing.

**Displays:** Latest native Surface-link fixture used the laptop at2560x1600/125% and an owned main-window Screen preview on NVIDIA. [Earlier physical hotplug, arrangement and restoration](evidence/mapgyver-m05-hotplug-2026-11-06.json) remain accepted for their tested two-display/both-GPU scope. Current control checks do not requalify external topology.

**Latest verified:** 4K 4× AA with two Surfaces now fits the 1 GiB Screen cap. Both GPUs passed image, allocation, rejection and retirement checks. [Checks](mapgyver-m14-antialiasing.md#4k-scratch-allocation--2026-11-20)
**Feedback verified:** [Per-channel subtitles are Accepted](evidence/mapgyver-m10-channel-subtitles-2026-11-18.json). During moving playback, both GPUs sampled the correct cues at hard and smooth wraps, in reverse, and after live relink. Native Media Preview showed the cue and the following gap.
**Latest feedback:** The shared Windows browser launcher suppresses physical Screen outputs by default. The owned NVIDIA WebView2 selected RTK FHD HDR (2), opened zero physical output windows and published exact Spout frames before and after reopening. M14/6 physical checks remain pending. [Checks](mapgyver-m14-screen-defaults.md#automated-test-isolation)
**Current handoff:** M14/8 returns to the supersampled composite render cost, with pixel parity required before comparing a changed renderer. M17 awaits independent LAN timing, reconnect and clean Windows installation. M14/6 awaits the remaining controlled physical checks. [Display checks](mapgyver-m14-screen-defaults.md#physical-rtk-startup-and-new)
**Latest batch:** All four sustained 4× Shared Texture cohorts completed. NVIDIA 1080p passed. NVIDIA 4K met every submission deadline and exceeded GPU p95. Intel exceeded GPU and submission limits at both sizes. All cohorts stayed under the memory caps and retired to zero. [Checks](mapgyver-m14-antialiasing.md#sustained-shared-texture-timing--2026-11-20)
**Launch display feedback:** With the laptop, RTK and Optoma connected, the 18:06 development build selected RTK on startup and on three New operations. All 16 checks passed with zero physical windows opened. The Start menu shortcut still opens installed beta `1.0.0b7`, which predates the fix. M14/6 physical-output checks remain pending. [Rebuild and checks](mapgyver-m14-screen-defaults.md#development-build-refresh)
**Overlay prerequisite cleared:** On2026-11-16 the user checked native Settings and confirmed Off, then instructed standing Off state for now (recorded16:23UTC). Use it until the user reports or authoritative evidence shows a change; do not ask before each run. All original GPU/correctness/resource budgets remain. Desktop-helper repairs are not required for this finite SDK pixel fixture.
**Retained blockers:** M10/4 retains unfinished age/full-GPU-path, shared-consumer, profile/repeated/performance-tier/resource/recovery qualification. [NVIDIA H.264](evidence/mapgyver-m10-h264-nvidia-2026-11-14.json) meets original cadence/CPU envelopes for one cohort and has complete connected direct-File pixel/PTS evidence. Its4K30 age upper59.3588/58.7843ms does not establish50ms; shared Reference is held-only and whole GPU path remains unproved. Earlier [two4K60 HAP](evidence/mapgyver-m10-two-4k60-hap-2026-11-13.json), [ProRes](evidence/mapgyver-m10-prores-alpha-sustained-2026-11-13.json), [resident Intel](evidence/mapgyver-m10-resident-sustained-2026-11-13.json) and [HAP variants](evidence/mapgyver-m10-hap-variants-sustained-2026-11-13.json) retain exact failures/scopes. The2026-11-16 native Settings confirmation and standing Off instruction govern subsequent cohorts. The M14 raster run establishes no M10 timing/performance claim. M08/4 WOW64 and M03/9 timing/growth debt remain; user-accepted smoothness/AA stay accepted.
**Current native limit:** [Corrected M03 lifecycle](evidence/mapgyver-m03-late-drain-2026-11-05.json) confirms 53.602 MiB peak growth against 32 MiB across all 100 cycles, with matching environment identities and fresh paired overlay-Off observations. Handles/live allocations return and shutdown takes 116 ms; peak growth still fails. The untracked private-memory cause is unproven; earlier failures remain preserved.
**Execution:** M19/1 Apple Silicon workbench is Accepted. M19/2 and M19/3 await physical desktop reconnect and cadence tests. M19/4 awaits a 64-bit Raspberry Pi OS desktop with HDMI. M19/5 Raspberry Pi armhf is Active. M14/8 returns when Windows qualification resumes. M17/1–2 require independent LAN and clean Windows qualification. M14/6 needs a physical session using both externals and native input. M14/1 is Blocked on physical pointer verification. M15/5 shared editor feedback and M14/2 smooth navigation are Accepted. M10/9 needs an elevated Windows session for genuine device loss during overlap. M18/1 needs direct cross-window Explorer input. M18/4 needs a valid key and live ShaderToy reachability. M10/3 extended formats stay user-deferred. M16/1 retains qualification. M10/8 and M10/10 are Accepted. M14/9–10 are Accepted. M09 waits on the retained gates.

**Recovery checkpoint:** [Earlier failures and parked candidate](evidence/mapgyver-request-lifetime-deferred-2026-11-17.json) remain historical. The integrated fix was reconciled with the current transport and adds runtime project-generation admission. [Verified scope](mapgyver-m14-editor-recovery.md)

Retained physical/timing gates stay in the explicit return queue.
**Latest feedback priority:** [M15](../plan/MapGyver_Implementation_Plan.md#m15--apply-the-2026-11-17-latest-feedback-before-remaining-video)
incorporates [Latest Feedback.md](../../Latest%20Feedback.md) and the2026-11-18 color/Group amendment as fifteen gates (thirteen Accepted, two remaining): safe
parameter controls; Matrix selection/colors, presentation and configuration presets; shared
editor transitions/handles; startup targets; related Surfaces/test patterns; linked Screen
handles; geometry alignment; Golden Gradient; external-only automatic sender defaults;
custom colors on Screens/Surfaces/Medias/Folders; Matrix Set Color / Reset Color menus;
single-line Layer Group headers; colored left-side wrapping rails for readable nested Groups.
M15/11–15 and geometry alignment are complete. The reported control-mode crash is a
correctness fix, with earlier priority if it blocks the active feature.
M15/1–4,6–9,11–15 are Accepted with their owning software/native proofs. No budget change
or automatic restoration of deferred work is authorized.
**Priority update:** [M14 feedback](../plan/MapGyver_Implementation_Plan.md#m14--apply-the-2026-11-08-feedback-after-first-pass-video)
adds ten gates covering shared 2D selection/hover/navigation, Composition actions/order/zero scale/
enabled metadata, external-display defaults, optional sender names, 4x AA and Matrix enabled/Stop
semantics. **The2026-11-14 user revision selects M14 now after accepted M10/1,2,5,6,7, retaining full M10/4 qualification as unfinished and release-blocking.**
This supersedes the earlier first-pass-acceptance-before-M14 execution order only.
Unrelated qualification remains queued and still gates M09; reproduced Video-path correctness/
ownership defects remain prerequisites. M13 is13/14 and M12 is11/11 after the explicit user
acceptance below. Planning alone accepts no gate.
**2026-11-09 Video feedback:** [M10/5–8](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)
added four gates: typed read-only/trigger transport and ranged Current Position (5 now Accepted); smooth
25–30 fps paused/playing seek with the reported `ball_turn.mp4` D3D invalid-call regression;
Matrix activate/deactivate transport enums; and low-priority disableable subtitles. Preserve
the selected gate and its exact remaining scope. M10/4 returns in retained first-pass
qualification. Video time cropping follows sender defaults and precedes subtitles, then SGF Feedback.
M10/5,6,7 are now accepted; previous Video evidence retains only its tested scope.
**Remaining requirement:** M14/M15/M16 and unfinished M13 format handling, complete app-control transient retirement, Video and retained
physical/performance/final qualification. Retained requirements and timing/retirement budgets
remain intact; M11/2 explicitly revises the fixed allocation-capacity policy.
**Latest acceptance / last delivered capability:** [M15/7 Surface display](evidence/mapgyver-m15-surface-display-2026-11-17.json) closes97→98 gates. Backend chooses full channel/calibration/no image; immutable matching patterns share a native procedural pass with stable reserved texture identities.16 render/editor/generated checks,19 UI tests, UI0/0, strict lint and bundled build pass. Native PID27252 exits0 after seven complete checks; Windows.Graphics.Capture observes source-free ColorBars/Grid, independent Grid/red-channel panes and64 related contours. Toolbar pages cover all70 peers with unchanged history and one preview; reload passes. The earlier related-view proofs retain cross-Screen/Reference/selection/Inspector/source recovery scope. M15/5 intermediate paint/cadence/Composition/Matrix and final GPU qualification stay open.
**Newly usable:** [Observed x64 process retirement](mapgyver-m08-retirement.md): short-lived children are pinned before execution, and the ordinary MapGyver singleton duplicate reaches Idle without adopting/killing the external singleton. Launcher/breakaway/access outcomes, project/shutdown recovery, actual script callbacks and fresh elevated execution pass. Visible debugging and retained WOW64 limits remain explicit; M08/4 stays open.
**Earlier accepted delivery:** [Official calibration pattern](evidence/mapgyver-m11-calibration-pattern-2026-11-04.json): square grey cells, palette rows, circle, diagonals, shared-clock rainbow boundaries, centered supplied logo, subtle cell labels, local millisecond clock and resolution/measured renderer FPS. One bounded draw works without a source. Eleven actual-app/native captures cover all required aspects through 4K, resize/warp, live timing, routing restoration and output retirement; software/strict and both-GPU kernel/resource checks pass.
**Foundation:** [Native repeated-image placement](evidence/mapgyver-m11-native-tiles-2026-11-04.json) passes both GPUs with shared upload, per-tile crops and withdrawal.
**Current limit correction:** Golden defaults to 64 registrations, but MapGyver explicitly
negotiates eight. Earlier notes conflated these limits. The app cap is unchanged; one logical
grid lease carries source groups through generated preview protocol v7 (v6 added per-group presentation Fit; v7 adds the window canvas colour). Logical channel capture has a separate bounded
128-definition policy, while physical producer/pass/dependency/memory budgets stay unchanged.
**Previously delivered:** [Typed Matrix output exposure](../architecture/mapgyver-matrix-output-exposure.md): Layer/Group opt-in channels preserve internal Final rendering; discovery, References, demand and retired identities honor exposure.
The generic Channels Inspector now follows the same bounded backend admission. Actual native
Layer/Group exposure, Blend Mode, Reference discovery and history pass; ordinary live channel
editing, CRUD/history and 37-channel paging remain verified.
**Previously delivered:** [Composition-owned creation](../architecture/mapgyver-owned-media.md): right-click the canvas for all supported media types; real typed children, placement, selection and single-step history need no source object in the catalog.
**Previously delivered:** [Shared canvas controls](../architecture/mapgyver-canvas-controls.md): centered navigation and contextual actions, bounded offscreen Matrix Frame, selected-only Surface paint and history-safe geometry input. M12/5 supersedes Matrix's Frame/Home toolbar with a scrolling grid and trailing creation headers.
**Previously delivered:** [Live Shared Texture controls](../architecture/mapgyver-sender-selection.md): inline Manual Detect, Auto sender choices and exact recovery, including after project reload.
**Superseded M11 behavior:** [Forwarded channel File controls](mapgyver-m11-inspector.md#direct-channel-file-controls) previously edited a catalog Parameter. Accepted M13/9 replaces that authority and removes its secondary asset picker; its complete audit above owns the current contract.
[Expanded typed channel controls](mapgyver-m11-inspector.md#expanded-typed-channel-controls) remain delivered.
Color edits need no nested Parameters folder; kind/sampling controls are gone and sampling is
fixed to Linear. Native editing/CRUD/history/paging and both-GPU Fit/Composition pixels pass.
Fit Mode, unit-preserving clocks and typed GPU Preferences remain delivered.
M11/4 is Accepted with complete native/browser/caller/history coverage; see the [acceptance audit](mapgyver-m11-inspector.md#m114-acceptance).
**Usable now:** typed media/References and independent previews; Screen/Surface mapping and
native outputs; child-media Composition/editors; Matrix model/playback/grid and typed owned
app control. The phase reports below own their detailed acceptance and remaining qualification.
**Last fully accepted phase:** M12 completes through the user's existing AA acceptance. M11's official calibration, identity/workbench and user-confirmed taskbar icon remain accepted; its [calibration audit](evidence/mapgyver-m11-calibration-pattern-2026-11-04.json) retains the earlier both-GPU kernel/resource and native clock/routing proof.
The [Shared Texture cadence hotfix](mapgyver-shared-texture-cadence.md) remains verified and pushed.
M11/10 and M11 remain accepted for their tested contract. M12 is 11/11 and M13 is 13/14.
M14 is 4/10 and M15 is 13/15 (87%), including geometry alignment and external-only sender defaults.
M08 remains 3/4 and Blocked on the retained WOW64 qualification/scope question.
Retained physical/timing requirements remain open; M10/5–10 own Video feedback, and M15 owns the latest changed feedback with unchanged M14 requirements retained.

| Phase                                                                                                                                                              | State                               | Progress          | Gates | Subphase progression                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------- | ----------------- | ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [M00 Baseline](mapgyver-m00-baseline.md)                                                                                                                           | Accepted                            | `██████████` 100% | 1/1   | Baseline 1/1                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| <a id="m01-acceptance-items"></a>[M01 Native route](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)                                      | Accepted                            | `██████████` 100% | 4/4   | Host/GPU/lifetime/decision 4/4                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| <a id="m02-acceptance-items"></a>[M02 Shared shell](mapgyver-m02-shell.md)                                                                                         | Accepted                            | `██████████` 100% | 7/7   | Build/identity/discovery/tools/host/isolation 7/7                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| <a id="m03-acceptance-items"></a>[M03 Rendering](mapgyver-m03-render-foundations.md)                                                                               | Blocked production qualification | `█████████░` 89%  | 8/9   | Foundations 7/7; recovery 1/1; qualification 0/1                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| <a id="m04-acceptance-items"></a>[M04 Media](mapgyver-m04-source-parity.md#m04-phase-audit)                                                                        | Accepted                            | `██████████` 100% | 8/8   | Sources 6/6; Reference 1/1; Preview 1/1                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| <a id="m05-acceptance-items"></a>[M05 Screens/editors](../plan/MapGyver_Implementation_Plan.md#m05--deliver-screens-surfaces-and-shared-navigation)                | Pending physical timing checks | `█████████░` 90%  | 9/10  | Authoring/warp/calibration 3/3; editors 2/2; physical 2/3; bounds/workbench stats 2/2                                                                                                                                                                                                                                                                                                                                                                                                               |
| <a id="m06-acceptance-items"></a>[M06 Composition](../plan/MapGyver_Implementation_Plan.md#m06--deliver-child-media-composition-and-editor)                        | Accepted                            | `██████████` 100% | 5/5   | [Child model 1/1](mapgyver-m06-model.md); [dependencies 1/1](mapgyver-m06-dependencies.md); [native output 1/1](mapgyver-m06-native.md); [editor geometry 1/1](mapgyver-m06-editor.md); [panels 1/1](mapgyver-m06-panels.md)                                                                                                                                                                                                                                                                        |
| <a id="m07-acceptance-items"></a>[M07 Matrix](../plan/MapGyver_Implementation_Plan.md#m07--deliver-child-media-matrix-playback-and-grid)                           | Accepted                            | `██████████` 100% | 4/4   | [Model 1/1](mapgyver-m07-model.md#m071-acceptance--2026-11-03); [playback 1/1](mapgyver-m07-playback.md); [grid 1/1](mapgyver-m07-grid.md); [native/scale 1/1](mapgyver-m07-native.md)                                                                                                                                                                                                                                                                                                              |
| <a id="m08-acceptance-items"></a>[M08 App control](../plan/MapGyver_Implementation_Plan.md#m08--deliver-reusable-optional-app-control)                             | Blocked retained qualification      | `████████░░` 75%  | 3/4   | Golden process 1/1; [configuration/demand 1/1](mapgyver-m08-configuration.md#configuration-and-demand-acceptance--2026-11-03); [runtime 1/1](mapgyver-m08-runtime.md); [remaining acceptance 0/1](mapgyver-m08-retirement.md)                                                                                                                                                                                                                                                                       |
| <a id="m11-acceptance-items"></a>[M11 Feedback](../plan/MapGyver_Implementation_Plan.md#m11--resolve-the-2026-11-03-feedback-before-video-and-final-qualification) | Accepted                            | `██████████` 100% | 10/10 | [Stability 1/1](mapgyver-m11-stability.md); [capacity 1/1](mapgyver-m11-capacity.md); [GPU 1/1](mapgyver-m11-gpu.md#native-preferences-acceptance); [media 1/1](mapgyver-m11-inspector.md#m114-acceptance); [discovery 1/1](../architecture/mapgyver-sender-selection.md); [canvas 1/1](../architecture/mapgyver-canvas-controls.md); [Composition 1/1](../architecture/mapgyver-owned-media.md); Matrix 1/1; [identity 1/1](evidence/mapgyver-m11-workbench-identity-2026-11-04.json); pattern 1/1 |
| <a id="m12-acceptance-items"></a>[M12 Feedback](../plan/MapGyver_Implementation_Plan.md#m12--resolve-the-2026-11-05-feedback-before-video) | Accepted (user revision) | `██████████` 100% | 11/11 | [Stability 1/1](evidence/mapgyver-m12-handoff-acceptance-2026-11-05.json); [Inspectors/enabled 1/1](evidence/mapgyver-m12-inspectors-2026-11-05.json); [re-entry/default windows 1/1](evidence/mapgyver-m12-reentry-2026-11-05.json); [Column Groups 1/1](evidence/mapgyver-m12-group-acceptance-2026-11-05.json); [grid 1/1](evidence/mapgyver-m12-grid-acceptance-2026-11-05.json); [locks/follow 1/1](evidence/mapgyver-m12-spatial-bindings-2026-11-05.json); [spatial editor 1/1](evidence/mapgyver-m12-spatial-acceptance-2026-11-05.json); [thumbnail Fit 1/1](evidence/mapgyver-m12-thumbnail-fit-2026-11-06.json); [typography 1/1](evidence/mapgyver-m12-typography-2026-11-06.json); [menu icons 1/1](evidence/mapgyver-m12-menu-icons-2026-11-06.json); [Screen AA qualification 1/1 (user acceptance)](evidence/mapgyver-user-acceptance-2026-11-08.json) |
| <a id="m13-acceptance-items"></a>[M13 Feedback](../plan/MapGyver_Implementation_Plan.md#m13--resolve-the-2026-11-06-feedback-before-video) | Blocked: format prerequisites | `█████████░` 93% | 13/14 | [Source defaults 1/2](evidence/mapgyver-m13-sender-defaults-2026-11-06.json); [Matrix layout/controls/drops/transitions 4/4](evidence/mapgyver-m13-transition-metadata-2026-11-07.json); [smoothness 1/1 (user acceptance)](evidence/mapgyver-user-acceptance-2026-11-08.json); [default Screen 1/1](evidence/mapgyver-m13-initial-screen-2026-11-07.json); [shared manager/time UX 2/2](evidence/mapgyver-m13-time-drag-2026-11-07.json); [Picture 1/1](evidence/mapgyver-m13-picture-acceptance-2026-11-07.json); [App Control Inspector 1/1](evidence/mapgyver-m13-app-control-inspector-2026-11-07.json); [canvas background 1/1](evidence/mapgyver-m13-surround-acceptance-2026-11-08.json); [targeting 1/1](evidence/mapgyver-m13-targeting-acceptance-2026-11-08.json) |
| [M10 Video](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers) | Pending qualification | `███████░░░` 70% | 7/10 | [Common codecs1/1](evidence/mapgyver-m10-common-acceptance-2026-11-09.json); [HAP1/1](evidence/mapgyver-m10-hap-acceptance-2026-11-09.json); [typed transport1/1](evidence/mapgyver-m10-transport-acceptance-2026-11-10.json); [seek1/1](evidence/mapgyver-m10-warm-seek-2026-11-11.json); [Matrix1/1](evidence/mapgyver-m10-matrix-video-policy-2026-11-12.json); [fade-loop Blocked0/1](evidence/mapgyver-m10-fade-loop-2026-11-18.json), [time cropping1/1](evidence/mapgyver-m10-time-crop-2026-11-18.json), [subtitles1/1](evidence/mapgyver-m10-channel-subtitles-2026-11-18.json), extended formats 0/1 deferred; integration0/1 retained qualification |
| <a id="m14-acceptance-items"></a>[M14 Editor/Matrix feedback](../plan/MapGyver_Implementation_Plan.md#m14--apply-the-2026-11-08-feedback-after-first-pass-video) | Pending: 4× AA render cost | `███████░░░` 70% | 7/10 | [Selection/hover Blocked0/1](mapgyver-m14-selection.md); [smooth navigation Accepted1/1](mapgyver-m14-navigation.md); [actions/order1/1](mapgyver-m14-composition-actions.md); [zero scale1/1](mapgyver-m14-zero-scale.md); [media enabled1/1](mapgyver-m14-enabled.md); [display defaults Blocked0/1](mapgyver-m14-screen-defaults.md); [sender name1/1](mapgyver-m14-sender-names.md); [4× AA Pending0/1](mapgyver-m14-antialiasing.md); [Matrix enabled1/1](mapgyver-m14-matrix-enabled.md); [Layer Stop1/1](mapgyver-m14-layer-stop.md#second-pass-verification-2026-11-20) |
| <a id="m15-acceptance-items"></a>[M15 Latest feedback](../plan/MapGyver_Implementation_Plan.md#m15--apply-the-2026-11-17-latest-feedback-before-remaining-video) | Accepted | `██████████` 100% | 15/15 | [Controls1/1](mapgyver-m15-parameter-control.md); [selection/colors1/1](evidence/mapgyver-m15-matrix-selection-2026-11-17.json); [appearance1/1](evidence/mapgyver-m15-matrix-appearance-2026-11-17.json); [presets1/1](evidence/mapgyver-m15-presets-acceptance-2026-11-17.json); [shared feedback1/1](mapgyver-m15-editor-feedback.md); [startup1/1](evidence/mapgyver-m15-startup-targets-2026-11-17.json); [Surface display1/1](evidence/mapgyver-m15-surface-display-2026-11-17.json); [Surface links1/1](evidence/mapgyver-m15-surface-links-2026-11-18.json); [alignment1/1](evidence/mapgyver-m15-geometry-alignment-2026-11-18.json); [Gradient1/1](evidence/mapgyver-m15-gradient-2026-11-19.json); [sender defaults1/1](evidence/mapgyver-m15-external-sender-defaults-2026-11-18.json); [custom colors1/1](evidence/mapgyver-m15-object-colors-2026-11-18.json); [color menus1/1](evidence/mapgyver-m15-matrix-color-menus-2026-11-18.json); [compact Groups1/1](evidence/mapgyver-m15-compact-groups-2026-11-18.json); [nested rails1/1](evidence/mapgyver-m15-group-rails-2026-11-18.json) |
| [M16 SGF Feedback](../plan/MapGyver_Implementation_Plan.md#m16--deliver-sgf-feedback-after-subtitles) | Pending: Video audio qualification | `█████████░` 93% | 13/14 | [Matrix transport 1/1](evidence/mapgyver-m16-matrix-transport-2026-11-18.json), [Layer transport 1/1](evidence/mapgyver-m16-layer-transport-2026-11-18.json), [Column Group transport 1/1](evidence/mapgyver-m16-column-group-transport-2026-11-18.json), [Auto Next 1/1](evidence/mapgyver-m16-cell-auto-next-2026-11-18.json), [wait modes 1/1](evidence/mapgyver-m16-navigation-waits-2026-11-18.json), [transition indicators 1/1](evidence/mapgyver-m16-transition-lines-2026-11-18.json), [empty Cell hover 1/1](evidence/mapgyver-m16-empty-cell-hover-2026-11-18.json), [compact opacity/blend controls 1/1](evidence/mapgyver-m16-compact-controls-2026-11-18.json), [Video audio functional 0/1](evidence/mapgyver-m16-video-audio-2026-11-18.json), [Video intro/loop 1/1](evidence/mapgyver-m16-separate-loop-2026-11-18.json), [Matrix Cell audio 1/1](evidence/mapgyver-m16-matrix-cell-audio-2026-11-18.json), [OSCQuery 1/1](evidence/mapgyver-m16-oscquery-addresses-2026-11-19.json), [seamless previews 1/1](evidence/mapgyver-m16-seamless-previews-2026-11-19.json), [linked outlines 1/1](evidence/mapgyver-m16-linked-outlines-2026-11-19.json) |
| [M17 NDI Media and Screen Output](../plan/MapGyver_Implementation_Plan.md#m17--deliver-ndi-media-and-screen-output) | Blocked | `░░░░░░░░░░` 0% | 0/2 | NDI Media receive 0/1, NDI Screen Output send 0/1. Independent LAN and clean installation remain. [Local output checks](evidence/mapgyver-m17-ndi-screen-audio-2026-11-19.json) |
| [M18 Feedback after NDI](../plan/MapGyver_Implementation_Plan.md#m18--deliver-feedback-after-ndi) | Blocked | `███████░░░` 67% | 4/6 | Project Settings, [File Explorer](evidence/mapgyver-m18-file-explorer-2026-11-19.json), [Inspector selection](evidence/mapgyver-m18-creation-selection-2026-11-20.json) and [browser previews](evidence/mapgyver-m18-browser-spatial-2026-11-20.json) Accepted. Direct OS file drops and live ShaderToy verification are Blocked. |
| [M09 Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)                                                | Waiting                             | `░░░░░░░░░░` 0%   | 0/1   | Revised actual-app qualification 0/1                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| [M19 Cross-platform](../plan/MapGyver_Implementation_Plan.md#m19--cross-platform-delivery) | Active | `██░░░░░░░░` 17% | 1/6 | [Apple Silicon workbench accepted](https://github.com/Golden-Geek/Chataigne2/actions/runs/44933901297). [Remaining gates](#execution-board-and-return-queue). |

**Counting:** overall = accepted fixed gates / 151, rounded to the nearest percent. M19 adds six
platform gates (145 → 151) and one phase (19 → 20). M18 adds six
pending feedback gates (139 → 145) and one phase (18 → 19). M17 added two pending NDI gates
(137 → 139) and one phase (17 → 18). M16/6 adds one acceptance (117 → 118 overall, 12 → 13 within M16). There are eight complete phases. The SGF Matrix feedback added eight gates (M16 6 → 14, overall 129 → 137). M10/10 added the time-cropping gate (128 → 129 overall, 9 → 10 within M10). The original M16 feedback added six gates and one phase (122 → 128 gates, 16 → 17 phases). Two gates were closed by explicit user acceptance.
The 2026-11-18 Video amendment adds M10/9 (121 → 122 gates). At the geometry checkpoint, 104 accepted gates count as 85% across 122 gates, with eight of sixteen phases complete. This fade-loop functional checkpoint awards no acceptance.
The2026-11-18 amendment adds M15/12–15:117→121 gates,98 accepted unchanged,84%→81%; M15 becomes6/15 (40%). No phase is added and no acceptance is awarded.
M15/8 acceptance then raises98→99/121 (82%) and M15 6→7/15 (47%); phase completion remains8/16 (50%).
M15/12 object-color acceptance adds one verified gate:100/122 (82%), M15 8/15 (53%); no phase completes.
Phase/subphase progress uses its fixed gate count. Bars round to tenths. The 2026-11-17 feedback
added M15's eleven fixed gates and one phase:
**106 → 117 gates**, **15 → 16 phases**, accepted numerator **92 unchanged**. Overall **87% → 79%**
and phase completion **53% → 50%** reflect added scope only; no older gate is waived or reopened
by counting alone. M15 supersedes changed behavior and requires its own acceptance.
The 2026-11-09 Video feedback added four fixed gates within M10:
**102 → 106 gates**, with **85 accepted** unchanged, so overall83% →80% and M10 2/4 →2/8
(50% →25%). No phase was added and no older acceptance was removed; changed behavior requires
the four new gates. Before the latest user acceptance, the 2026-11-08 feedback added M14 ten fixed gates and one phase:
denominators **92 → 102 gates** and **14 → 15 phases**. At that scope change, the accepted numerator stayed **81**;
88% → 79% reflects new scope only. M10 originally had four gates; gate 4 still covers first-pass
bundled integration, now including high-priority gates 5–7, and gate 3 includes the same
integration requirements for the extended formats. MKV track selection is part of this same
gate. Gate 8 adds low-priority subtitles before M09.
The earlier 2026-11-06 feedback added M13's fourteen fixed gates and one phase:
denominators **78 → 92 gates** and **13 → 14 phases**. The accepted numerator stays **69**;
the change from 88% to 75% reflects added scope, with no acceptance awarded or removed.
M11 stays 10/10, M08 stays 3/4 and M07 stays 4/4 for their original tested contracts.
Before the latest user acceptance, M12 remained10/11; M13 owns latest changed-contract verification. Prior evidence is reused only
within its tested scope; M13 supersedes the older Picture asset and all-channel Reference contracts.
Partial work, planning and unavailable checks earn no gate.

**2026-11-02 scope revision:** the prior plan was 5/10 phases (50%), with 40/48 gates accepted.
The updated requirements reopen affected M04–M08 acceptance and add M10. M04 changes 6/6→1/8
(reopen 1,2,4,5,6; add Reference/Preview); M05 5/8→3/10 (reopen 6,7; add bounds/supplements);
M06 4/4→0/5 (reopen 1–4; add independent panels); M07 3/4→0/4 (reopen 1,2,4);
M08 3/4→1/4 (reopen 2,3); M10 adds 0/4. Other gates are unchanged.
This adds nine gates, reopens sixteen and changes the phase denominator from 10 to 11.
Earlier evidence is retained for reuse, with its original schema/build limits; no new feature
or gate was accepted by merging the plan.

**2026-11-08 user acceptance:** existing smoothness and AA qualification close two gates by explicit user decision:81 to83/102. The denominator stays102; M12 completes and phase completion becomes8/15. Retained numerical failures remain historical evidence. New4x AA implementation remains required.

## Execution board and return queue

**Current delivered feature:** Apple Silicon can start the MapGyver desktop and connect the
bundled workbench. Authoring, history and Save/New/Load pass through the same runtime.
[Apple Silicon checks](https://github.com/Golden-Geek/Chataigne2/actions/runs/44933901297)

M10/3 extended formats is Pending until the user resumes it. M16/1 retains native qualification Pending. M10/10 time crop is Accepted. M10/9 device loss is Blocked on an elevated session and M10/8 subtitles is Accepted. M15/1–15 are Accepted. Selected M14 features retain qualification
Pending. M14/1 is Blocked on physical pointer verification. M14/6 RTK routing and display clearing are verified. Remaining physical checks require a controlled display session.
M14/2–5,7,9–10 are Accepted; M10/4 stays unfinished/release-blocking. Pending rows are the
return queue, not concurrent work. Intel's
failed readback stress returns with M10/4, without changed budgets.

| State    | Gate(s)  | Missing requirement / concrete next or return action                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| -------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Accepted | M19/1 | [Apple Silicon CI](https://github.com/Golden-Geek/Chataigne2/actions/runs/44933901297) passed the native build, authoring and persistence fixture, and strict desktop smoke. |
| Blocked | M19/2 | Both Mac architectures passed relocated bundles and captured Rectangle and Fullscreen pixels. Return on a physical Mac with a connected external display to verify scanout cadence and unplug/reconnect against the saved UUID. |
| Blocked | M19/3 | Ubuntu 24.04 and 26.04 release packages and the clean Ubuntu hosted job passed desktop, media and virtual display recovery checks. Return on a physical Linux desktop with an external monitor to verify reconnect and cadence for X11 and Wayland. |
| Blocked | M19/4 | The installed Debian 13 ARM64 package passed the native desktop, media and virtual output suites. Return on a Pi with the current 64-bit Raspberry Pi OS and HDMI to verify the on-device build, desktop, media and real Screen output using the Pi GPU. |
| Active | M19/5 | Produce the armhf package with the Raspberry Pi OS 32-bit userland ABI. Verify install, desktop and media software checks before Pi GPU and HDMI qualification. |
| Pending | M19/6 | Build and verify an ARM64 Windows SDK, a native install, desktop media/output and recovery. |
| Accepted | M13/1 | [Complete creation audit](evidence/mapgyver-m13-sender-defaults-2026-11-06.json): fresh Auto/first sender/None through every creation factory, script/command/public callers; zero/one/multiple sender cases, real SDK discovery order/loss/recovery, typed generic controls, creation/mode history, copy and sparse Manual/None/Auto reload pass. Ordinary hostPID42156 and both owned producers exit0; no desktop input or new format/timing claim. |
| Blocked | M13/2 | [Complete available app catalog](evidence/mapgyver-m13-app-catalog-2026-11-07.json) passes70/68 native layouts; earlier live/source/Intel device/D3D9 proof retained. Return on exposed NVIDIA D3D9 source, supported same-LUID NVIDIA loss/revised identity contract, actual backend/new path for named rejected allocations, authoritative metadata/revised contract for unadvertised interpretation. Every exact prerequisite/action remains in the receipt; no full gate credit or unchanged rerun. |
| Accepted | M13/3 | [Complete Matrix acceptance](evidence/mapgyver-m13-matrix-acceptance-2026-11-07.json): canonical inline controls/top-left labels/whole headers, root/nested/reparent/history/reload, front-first rendering and SVG hit order,72 threshold/font-scale cases, actual minimum dock resize, complete both-GPU native grid/Group/offscreen/sparse lifetime flows and22 independent output images pass. No timing or generic GPU-owner retirement claim. |
| Accepted | M13/4 | [Explicit user acceptance for now](evidence/mapgyver-user-acceptance-2026-11-08.json). Existing behavior is accepted; retained36/39 updates/s means,12 ambiguous frames and21.56px separation are historical failed measurements. No new smoothness qualification is queued absent feedback or a reproduced dependency defect. |
| Accepted | M13/5 | [Complete acceptance](evidence/mapgyver-m13-initial-screen-2026-11-07.json): atomic typed Screen/Surface, zero bootstrap history, desktop/headless startup and repeated New, real Windows/menu commands, exact zero/multiple load/reload, ordinary source/output edits and both-GPU output withdrawal/recovery pass. Detached-candidate handles bind before publication without app inbox callbacks. |
| Accepted | M13/6 | [Complete scoped-control audit](evidence/mapgyver-m13-control-acceptance-2026-11-07.json): every typed owner/caller, empty/disabled/rapid/history/reload, outgoing/anchor/unrelated scopes, actual grid/generic Inspector controls, six held-completion process cases and12 both-GPU pixel/retirement cases pass. Column Group's real Stop folder is visible after fixing its app-owned Inspector filter. |
| Accepted | M13/7 | [Complete Cell drop audit](evidence/mapgyver-m13-cell-drop-acceptance-2026-11-07.json):24 bundled UI cases, all four callers,16 sparse structural cases and216 exact both-GPU images pass. Running scopes/relaunch, Copy/Reference source/process ownership, exact history/reload and16 zero-resource retirements are verified. The32-Cell chain boundary rejects atomically. No desktop-input or production-cadence acceptance is inferred. |
| Accepted | M13/8 | [Complete manager audit](evidence/mapgyver-m13-manager-acceptance-2026-11-07.json): exact creation receipts survive transport, pending selection waits for graph projection and all menu creation selects. Bundled15 cases, four Chataigne consumers, filtered/virtualized disclosure, real hierarchy, history/reload and owning software/strict checks pass. No desktop-input/native cadence acceptance inferred. |
| Accepted | M13/9 | [Complete Picture audit](evidence/mapgyver-m13-picture-acceptance-2026-11-07.json): real per-channel File, no implicit Asset/forwarding or extra selection, independent copies, all caller/history/reload and generic controls pass. Shared-image identity/admission, event-driven same-path Reload, missing/replaced recovery,78 both-GPU images and bounded retirement pass. Three Picture-specific native tests are clean; the separate historical Matrix interruption failure remains M09 debt. |
| Accepted | M13/10 | [Complete clean-Inspector audit](evidence/mapgyver-m13-app-control-inspector-2026-11-07.json): authored settings/conditional folders only for Column, Cell, Column Group and direct generic child inspection. Mixed/offscreen/disabled/project states, zero per-Inspector queries, shared root reader, actual Group and shared Column/Cell lifecycle plus named script/typed-command callbacks pass. No new native cadence/production acceptance is inferred. |
| Accepted | M13/11 | [Complete transition audit](evidence/mapgyver-m13-transition-metadata-2026-11-07.json): real optional Time metadata, no separate boolean, preserved dormant values and cached Cell → Column → Layer precedence,250ms base and Smooth default. All owners/callers, zero/nonzero/Linear/Smooth, history/copy/reload, fresh generic UI controls and both-GPU interrupted native fade/demand checks pass. Retained broad Matrix fixture failures remain M09 debt. |
| Accepted | M13/12 | [Complete shared time audit](evidence/mapgyver-m13-time-drag-2026-11-07.json):319 unit tests, three clean UI type checks,13 shared browser groups and11 actual bundled app groups. Real drag/slide, fractional stepped/unstepped units, bounds, keyboard/accessibility, pointer cancellation/loss, mixed selection safety, one backend history item and exact Undo/Redo pass. Dashboard shares the control; backend/protocol/native behavior is unchanged. |
| Accepted | M13/13 | [Complete actual desktop acceptance](evidence/mapgyver-m13-surround-acceptance-2026-11-08.json):32 frames and12 independent output images across both GPUs establish grey surround/content-black backing, native image/physical controls/hits, missing-source DOM fallback, pan/zoom/clipping, theme/rem, window/dock resizing and project transitions. Both hosts exit0; original budgets and failed attempts preserved. |
| Accepted | M13/14 | [Complete targeting audit](evidence/mapgyver-m13-targeting-acceptance-2026-11-08.json): shared catalog-only Media/folder menu and dependent Channel dropdown, typed backend eligibility/defaults, four-caller parity, nested current bindings, missing/recovery, copy/history/reload and large catalogs pass.579 app/320 Golden tests,145 contracts, ten bundled browser groups and both-GPU42 routing/162 mixed images pass. Reconciled Reference, unavailable-drop and Matrix-exposure scopes preserve independent output identity. Earlier failed measurements and every unrelated qualification gate remain preserved. |
| Blocked  | M03/9    | [Proved elapsed GPU-path failures](evidence/mapgyver-m03-gpu-path-lower-2026-11-05.json), original sync-0 deadline misses and lifecycle peak growth remain. GPU-scope/deadline/memory proposals await explicit decisions; unchanged targets still bind. Return after a decision or a justified owning implementation addressing an original failure. No new presentation-policy/clock probe.                                                                                                                                                                                              |
| Accepted | M12/1 | [Complete functional stability acceptance](evidence/mapgyver-m12-handoff-acceptance-2026-11-05.json): corrected sizing, shared masks, staged native layout and themed retirement; both-GPU recorded input/resize, draft/scroll, reconnect/output and project transitions pass. Capture scope and retained production budgets remain explicit. |
| Accepted | M12/2 | [Standard Inspectors and one Surface enabled state](evidence/mapgyver-m12-inspectors-2026-11-05.json): current-object editing, mixed/offscreen selection, history/copy/reload, scripts/headless and both-GPU output checks pass. Column Groups introduced in M12/4 inherit this contract and need their actual-instance checks there. |
| Accepted | M12/3 | [Current app re-entry and default windows](evidence/mapgyver-m12-reentry-2026-11-05.json): actual visible Normal/explicit Hidden, 20 native rendered and headless zero-pre/post cycles each plus five stopping re-entries each, crash/endpoint recovery, ownership/history/project/shutdown pass. Original M08/4 remains unaccepted. |
| Accepted | M12/4 | [Complete audit](evidence/mapgyver-m12-group-acceptance-2026-11-05.json): persisted nested topology, standard Inspector, flat demand/independent Cell semantics, outgoing retention, shared process continuity, 20 cycles/five stopping re-entries, history/projects, actual Normal/Hidden windows, native SDK source/output and requested/submitted/stopping markers pass. Both GPUs pass registration/first-frame readiness with actual completion and retirement. |
| Accepted | M12/5 | [Complete adaptive grid acceptance](evidence/mapgyver-m12-grid-acceptance-2026-11-05.json): settled zero-axis creation, small/overflow/resize, keyboard/history, trusted drag/reparent, groups, offscreen Inspector and 1000×1000 sparse lifetime pass. Native PID 10676 exits normally; unchanged binary hash and software prerequisites match the pushed source milestone. |
| Accepted | M12/6 | [Complete shared binding acceptance](evidence/mapgyver-m12-spatial-bindings-2026-11-05.json), alongside independent grid, Screen/Surface, Inspector and Media Preview proofs. Matrix/Composition target resolution, locks/rename, unrelated retention, off-page selection, missing/Undo, layout/reconnect/New/Load and document fences pass. Full manipulation/Cell placement stays in M12/7. |
| Accepted | M12/7 | [Full requirement audit](evidence/mapgyver-m12-spatial-acceptance-2026-11-05.json): shared Composition/Matrix viewport and geometry, typed Size/Fit, active/inactive editing, bounded independent selection, all pointer/keyboard handles and history, selection/locks/reconnect/project fences, 1000×1000 sparse editing and 222 qualified both-GPU images pass. Eight owned UI hosts retire normally. Original production/physical budgets and return gates remain unchanged. |
| Accepted | M12/8 | [Explicit user acceptance for now](evidence/mapgyver-user-acceptance-2026-11-08.json). Existing Raw/Smooth behavior and AA qualification are accepted. Earlier Intel4K cost/cadence failures remain historical limits; no further optimization/qualification is queued absent new feedback or a reproduced dependency defect. M14/8 still implements new4x AA with functional checks. |
| Accepted | M12/9 | [Complete thumbnail Fit](evidence/mapgyver-m12-thumbnail-fit-2026-11-06.json): current native dimensions fit before clipping; both GPUs pass aspects, live resize, alpha/margins, grouped labels, scroll/reuse, viewport resize, bounded shared tiles and independent output pixels. Both owned app processes exit normally. |
| Accepted | M12/10 | [Complete typography acceptance](evidence/mapgyver-m12-typography-2026-11-06.json): embedded licensed font, larger destination-density readouts and subordinate labels, existing shared clock/FPS, unchanged branded geometry/palette, bounded worker preparation/one admitted atlas, both GPUs/quality modes/aspects/warp/live/routing and normal owned retirement. M12/8 timing remains separate. |
| Accepted | M12/11 | [Complete menu icon acceptance](evidence/mapgyver-m12-menu-icons-2026-11-06.json): all current panel/tab and catalog/outliner/Composition identities, generic panels, Folder submenu, keyboard labels and six CSS-token/relative-scale variants pass in browser and actual app against the supplied screenshots. No physical DPI or timing acceptance inferred. |
| Pending | M05/8 | [Common-QPC software checkpoint](evidence/mapgyver-m05-native-qpc-2026-11-06.json) is pushed in a2034676. Supported computer-use capture/activation now succeeds for four owned M13/2 app observations. No M05 native timing samples exist; preparedPID2924/HWND67965064 expired, fixture exit1. Return after M13/2: one fresh owned30-action baseline plus independent raw-pixel/clock/terminal replay, at most one justified comparison. Original p95<=50ms, same-frame moving correspondence and complete physical scope remain open. Earlier73.2444/75.8513ms bounds and cumulative limits remain preserved. |
| Accepted | M11/4    | [Complete typed Inspector acceptance](mapgyver-m11-inspector.md#m114-acceptance): compact File rows, real shared Parameters, stable File/Color drafts, full generic/source browser flows and native catalog/channel history/project/CRUD/paging checks. Fit pixels, clocks, commands and four-caller parity remain verified.                                                                                                                                                                                                                                                              |
| Accepted | M11/3    | [Typed GPU Preferences and native acceptance](mapgyver-m11-gpu.md#native-preferences-acceptance): five cold starts, both explicit GPUs, separate persisted choice, truthful restart requirement, pinned renderer recreation, missing choice without fallback and recovery. Eight full pixel checks and 447 independent copies pass.                                                                                                                                                                                                                                                       |
| Accepted | M11/2    | [Complete capacity acceptance](mapgyver-m11-capacity.md#m112-acceptance--quiescence-releases-context-bindings): both GPUs pass eight 4K producers/two aliases, 56 complete images, bounded native ownership, reuse and real pressure/recovery. NVIDIA/Intel retire in 61.721/260.242 ms; pressure cases 49.990/93.013 ms, within the original two-second bound.                                                                                                                                                                                                                           |
| Accepted | M11/1    | [Native stability acceptance](mapgyver-m11-stability.md#m111-acceptance--native-input-and-project-transitions): recorded selection/reconnect fixes, actual Inspector/Matrix input, independent outputs, guarded desktop pixels, three close/reopen cycles and New/Load with old imagery retired. No prior brief-unavailable waiver used.                                                                                                                                                                                                                                                  |
| Blocked  | M08/4    | [Named x64 matrix and retained WOW64 case](mapgyver-m08-retirement.md): singleton/launcher/breakaway/access/project/script and fresh elevated Ready/Idle/token/cleanup now pass. Candidate fenced assignment still returns WOW64 Exited(0), active/pending zero and one unmatched accounting entry; candidate removed and stashed. Return on the explicit pending x64-versus-positive-WOW64 scope answer, or new authoritative barrier/owning correction. No entry waived or assumed to be a distinct child; M09 remains gated. First-pass Video proceeds under the explicit 2026-11-08 priority.                                                 |
| Accepted | M07/4    | [Full native audit](mapgyver-m07-native.md): both GPUs pass Layer/Group/Final/preview blends/alpha/order, shared source, deletion/Undo, 1000×1000/six-Cell bursts/reload/interruption and twelve completion-safe retirements. Bulk typed creation is corrected; M07 is complete at 4/4.                                                                                                                                                                                                                                                                                                   |
| Accepted | M08/3    | [Actual runtime audit](mapgyver-m08-runtime.md): both Spout readiness modes, process sharing/handoff, timeout/crash/backoff, native pre-resume cancellation, final-use retention, stale-project rejection and pinned root/branch/leaf retirement pass. M08 is 3/4; complete UI/target/access scope remains M08/4.                                                                                                                                                                                                                                                                         |
| Accepted | M11/5    | [Live sender control acceptance](../architecture/mapgyver-sender-selection.md): Manual String + inline Detect, Auto Enum, live metadata, exact recovery, independent GPU outputs, adapter mismatch, history and post-load mode/absence/recovery.                                                                                                                                                                                                                                                                                                                                          |
| Accepted | M11/6    | [Shared canvas controls](../architecture/mapgyver-canvas-controls.md): public toolbar/shortcuts, contextual actions, bounded Matrix Frame, selected-only paint, native navigation/Composition/keyboard/history/source and narrow-peer checks pass.                                                                                                                                                                                                                                                                                                                                        |
| Accepted | M11/7    | [Owned creation](evidence/mapgyver-m11-composition-creation-2026-11-04.json): complete backend/catalog/command/script/template contract, actual native/headless menus and history, toolbar copy/delete, reorder, project identity and independent GPU pixels before/after reload.                                                                                                                                                                                                                                                                                                         |
| Accepted | M11/8    | [Full audit](evidence/mapgyver-m11-cell-tiles-2026-11-04.json): both pinned GPUs pass nine-source/Final and live-edit/loss/Undo/Fit/scroll pixels; sparse 1,000×1,000 UI lifetime and readable labels pass. Full native suite: 240 images, zero envelope error, 16 balanced owner retirements. Typed Inspector/discovery, nested Group blending, References/copy/projects and backend exposure pass.                                                                                                                                                                                      |
| Accepted | M11/9    | [Full audit](evidence/mapgyver-m11-workbench-identity-2026-11-04.json): supplied media/tab/header/window/executable/NSIS icons, Medias/Screens names, independent locked titles/rename/reconnect/New/Load, Screen/Surface lifetime, real human-readable connected display, missing-state regression, real runtime/narrow/unavailable/recovery footer checks and user-confirmed taskbar identity.                                                                                                                                                                                          |
| Accepted | M11/10   | [Full official pattern audit](evidence/mapgyver-m11-calibration-pattern-2026-11-04.json): complete one-draw branded geometry/image/text, both-GPU kernels/resources, all required actual-app native aspects through 4K, resize/perspective warp, decoded local milliseconds/resolution/measured FPS, source-free routing, Off restoration and owned output retirement.                                                                                                                                                                                                                    |
| Accepted | M10/1 | [Complete common-codec audit](evidence/mapgyver-m10-common-acceptance-2026-11-09.json): seven profiles on both GPUs,130 physical pixels and2,980 clean UI receipts; chromatic/10-bit resident H.264/HEVC, ProRes4444 alpha, controls, independent copy, shared Matrix/Reference, mixed-rate hold and project reload.611 app/264 Golden software tests, strict and generated contracts pass. Existing tiny-file failures stay explicit fallbacks; sustained/bundled/device qualification stays M10/4. |
| Accepted | M10/2 | [Complete HAP audit](evidence/mapgyver-m10-hap-acceptance-2026-11-09.json): all three formats retain compressed GPU blocks; both-GPU external4K/known-color/alpha/YCoCg-scale pixels, three concurrent4K files/six consumers/three decoder-render owners/stable storage/zero retirement and actual HAP copy/Matrix/Reference/reload pass.611 app/270 Golden tests, strict/UI and178 physical checkpoints/4,130 clean receipts pass. Initial weak copy-pixel baseline is explicitly unaccepted; corrected bright/scaled fixtures establish acceptance. |
| Pending | M10/4 | User revisions retain every requirement unfinished/release-blocking while M14/M15 proceed. Return at a scheduled qualification boundary after M15 or for a reproduced active-product dependency defect. Recheck [coherent binaries/source inputs](evidence/mapgyver-m10-producer-build-2026-11-14.json), original native OverlayOff/environment prerequisites, then run existing three-receiver H264 controller and inspect path/age/lineage/cadence/resources. Fresh user-native Off09:32 UTC is recorded, no cohort claimed. Prior cadence/CPU and21608 direct-File matches retain one-cohort scope;2/3 Reference coverage,4K30 age upper59.3588ms and complete path/profile/series/performance-tier/resource/recovery/render-thread-origin debt remain. |
| Accepted | M10/5 | [Complete acceptance](evidence/mapgyver-m10-transport-acceptance-2026-11-10.json):640 app/694 engine tests, live bounds/feedback, all callers/history/copy/reload and current generic Inspector. Both GPUs pass ten profiles/234 physical checkpoints/3,782 clean receipts, relink, copies, Matrix/Reference and reload. Current held-drag/resume and load policy pass. |
| Accepted | M10/6 | [Complete revised-scope audit](evidence/mapgyver-m10-warm-seek-2026-11-11.json): NVIDIA35.65/33.35 physical paused/playing updates/s, p9562.76/59.89ms conservative copy bounds, zero unknown pixels. Both-GPU exact seeks, native Inspector/image review and genuine recovery pass. Intel normal-input progress is verified under the user's allowance; its0.235/0.275/s continuous-readback stress failure remains visible M10/4 debt, not a healthy-rate claim. |
| Accepted | M10/7 | [Complete original audit](evidence/mapgyver-m10-matrix-video-policy-2026-11-12.json):678 app/696 engine tests, strict/generated/UI/default runtime builds and both-GPU34-image native runs pass. Nine policies, moving outgoing completion, manual/reactivation/deletion/project fences, shared References/two-Layer Stop, Matrix enable, caller/history/copy/reload and zero retirement are verified. NVIDIA1240/Intel1245 clean UI receipts; both normal exit0. CPU-upload evidence stays mandatory for existing sustained/playback qualification, while policy accepts actual resident frames. M14/10 fading Stop and all M10/4 budgets remain required. |
| Blocked | M14/1 | [Verified editor recovery](mapgyver-m14-editor-recovery.md) closes the exact Source Undo and old-project mutation failures on both GPUs. Return with the Computer Use `node_repl` tool or a hands-on session to verify physical pointer input and same-frame image/control/hit agreement. Retain any uncovered Matrix active/outgoing selection cases in that pass. |
| Accepted | M14/4 | [Zero-scale Composition](mapgyver-m14-zero-scale.md): typed/Inspector/public-caller, finite geometry, selection/history/copy/reload, healthy siblings, zero crossings and source recovery pass. Independent two-output/preview pixels and retirement pass on both GPUs in the unchanged19-state fixture. |
| Accepted | M14/5 | [Media metadata enablement](mapgyver-m14-enabled.md): Inspector/menu/public callers, disabled visibility/editing, demand/final-use, history/copy/reload and no duplicate Enabled pass. Full browser history contract and both-GPU disabled/restored native pixels pass; no event or numerical budgets changed. |
| Accepted | M14/3 | [Complete actions/order audit](mapgyver-m14-composition-actions.md): toolbar/menu, command/scripts, history/copy/reload, bounded paint/hits, mixed-ratio transforms and both-GPU27-state/162-image pixels/retirement pass. Corrected Video status retirement and original seven-type creation/editor contract pass without rejected requests. Default parallel Video failures remain M09 debt below. |
| Blocked | M14/6 | [Startup and New at RTK](mapgyver-m14-screen-defaults.md#physical-rtk-startup-and-new) passed with real pixels on both GPUs and two connected externals. [Display clearing](mapgyver-m14-screen-defaults.md#clearing-a-fullscreen-target) is verified. Return for a controlled physical session covering one-external defaults, simultaneous and atomic output on both externals, hot-plug and native Ctrl+N. |
| Accepted | M14/7 | [Complete optional sender audit](mapgyver-m14-sender-names.md): fresh disabled String metadata, automatic/custom naming, dormant text, all callers/history/copy/reload, real Inspector, both-GPU32 checks/20 exact images, genuine SDK collision HRESULT and automatic retry, target/project withdrawal and completed old native-owner retirement pass. No timing/peak-growth or retained gate claim. |
| Accepted | M14/9 | [Matrix enablement](mapgyver-m14-matrix-enabled.md#second-pass-verification-2026-11-20) passed Inspector, active/outgoing playback, exact reload and independent Intel/NVIDIA output and editor pixels. |
| Accepted | M14/10 | [Layer Stop qualification](mapgyver-m14-layer-stop.md#second-pass-verification-2026-11-20). Typed callers and history, native timing and overrides, interruption, unchanged peers, shared process lifetime and completion-safe retirement passed. Intel and NVIDIA matched 240 output and preview images within the oracle. |
| Pending | M14/8 | [4× AA](mapgyver-m14-antialiasing.md) is functionally delivered. The Inspector, history, both-GPU pixels, scratch admission and overload checks passed. [Sustained timing](mapgyver-m14-antialiasing.md#sustained-shared-texture-timing--2026-11-20) passed NVIDIA 1080p. When Windows qualification resumes, return with a lower-cost render path for NVIDIA 4K and both Intel sizes, then finish supported-hardware rejection and physical recovery. |
| Accepted | M14/2 | [Smooth navigation](mapgyver-m14-navigation.md#second-pass-verification--2026-11-20) passed native same-frame image/control pixel checks, Frame/Home keyboard and toolbar actions, resize interruption and sparse Matrix selection on Intel and NVIDIA. Separate M05/8 latency and M13/4 synchronization remain open. |
| Accepted | M15/1 | [Complete control-safety acceptance](mapgyver-m15-parameter-control.md): shared numeric admission, all required control paths/types, changed bounds, invalid-input/sibling safety, grouped history, canonical/explicit metadata and actual bundled three-control mode/history/normal/omitted-metadata reload pass. No performance/codec qualification is inferred. |
| Accepted | M15/2 | [Selection/color contract](evidence/mapgyver-m15-matrix-selection-2026-11-17.json): pointer/keyboard/modifier/shared selection, nested Groups, disabled/offscreen selection, all five kinds' Inspector colors/history/copy/reload, representative dark/light contrast and independent panels pass. Wider stress/interruption returns with the final app regression batch before M09. |
| Accepted | M15/3 | [Presentation contract](evidence/mapgyver-m15-matrix-appearance-2026-11-17.json): quiet themed contours/rounded cards; per-Layer submitted state, shadow transitions/reduced motion, selection/custom colors, dark/light/font/narrow presentation, sparse offscreen grid, interruption and stable hit/preview layout pass. Actual app selected/unselected/idle screenshots reviewed; final GPU/stress gates unchanged. |
| Accepted | M15/4 | [Complete preset acceptance](evidence/mapgyver-m15-presets-acceptance-2026-11-17.json), [core workflow](evidence/mapgyver-m15-presets-2026-11-17.json) and [Column muting](evidence/mapgyver-m15-column-enabled-2026-11-17.json): stable typed snapshots, all five contextual/command/script operations and callbacks, atomic history/rejection, copy/reload, disabled/outgoing playback, million-address sparse axes and bounded member/offscreen Inspector editing. Native menu/recall/paging/real Parameters/save/reload pass. No retained production gate is closed. |
| Accepted | M15/5 | [Presented editor feedback](mapgyver-m15-editor-feedback.md) passed on Intel and NVIDIA. Owned captures verify intermediate hover and selection, retargeting, reduced motion and matching native installation. Actual Matrix Cell selection, keyboard focus and held gestures pass. |
| Accepted | M15/6 | [Startup/New target acceptance](evidence/mapgyver-m15-startup-targets-2026-11-17.json): desktop/headless panels prove fresh identities, zero bootstrap history, hidden/late/reopened panels, saved targets/locks/reload and empty projects. Backend tests additionally prove deletion/Undo and foreign-loaded-project fencing. |
| Accepted | M15/7 | [Surface-display acceptance](evidence/mapgyver-m15-surface-display-2026-11-17.json) and [crop correction](evidence/mapgyver-m15-calibration-crop-2026-11-17.json). Native source-free patterns/contours, mixed independent panes,70-peer paging/reload and the revised cropped ColorBars/Grid mapping pass. All original resource/profile/final gates remain. |
| Accepted | M15/8 | [Complete acceptance](evidence/mapgyver-m15-surface-links-2026-11-18.json): same-Screen snapping, font/zoom/thresholds/off, group history/copy/reload/missing targets, live held-pointer peer/output images, five native Undo cycles and three-member merge/unlink pass. User-activated foreground capture verifies32 edge probes (2.3125px maximum color-boundary offset within3px contour/rounding allowance) and all five native corner/unlink hits. No subpixel, high-refresh or Intel qualification claim. |
| Accepted | M15/12 | [Object-color acceptance](evidence/mapgyver-m15-object-colors-2026-11-18.json): Screens, Surfaces, Medias/References and nested Folders share typed metadata and generic Inspector set/reset. Script/transport/history/copy/reload, default reset, bounded editor cues and light/dark WebView appearance pass. Retained native presentation/Intel/performance qualification is unchanged. |
| Accepted | M15/13 | [Matrix color-menu acceptance](evidence/mapgyver-m15-matrix-color-menus-2026-11-18.json): all five context kinds, shared picker, default/custom/reset, context-only multiselection convention, grouped Undo/Redo, atomic sparse Cell color, offscreen Inspector and reload pass; existing metadata copy proof retained. |
| Accepted | M15/14 | [Compact Group acceptance](evidence/mapgyver-m15-compact-groups-2026-11-18.json): single-line new/existing/nested/empty Groups, generic Inspector Opacity/Blend Mode, collapse/hidden selection, context actions, grouped history/reload, shared DOM/native tile geometry and narrow/font/device-scaled hit regions pass. Retained GPU/physical qualification is unchanged. |
| Accepted | M15/15 | [Group-rail acceptance](evidence/mapgyver-m15-group-rails-2026-11-18.json): bounded offscreen ancestry/ranges, distinct nested custom/reset colors, theme/selection/hover/disabled states, clear control gutters, clipping/scroll/collapse/reparent/reorder and pointer transparency pass. Bundled-app DOM captures award no retained GPU/physical qualification. |
| Accepted | M15/9 | [Verified geometry alignment](evidence/mapgyver-m15-geometry-alignment-2026-11-18.json). Shared move/resize guides across four editors, native same-frame Composition pixels within 1 physical pixel, sparse Matrix, zoom/pan, Escape and exclusions pass. |
| Blocked | M10/9 | [Playback and relink verified on both GPUs](evidence/mapgyver-m10-fade-loop-2026-11-18.json). Return in an elevated Windows test session for genuine device loss during overlap, fresh post-recovery direct/Reference pixels and the remaining wider shared-consumer/storage cases. |
| Accepted | M15/11 | [External sender defaults](evidence/mapgyver-m15-external-sender-defaults-2026-11-18.json). Native and headless SDK discovery, exact keys and labels, same-name takeover/recovery, multiple outputs, renaming, caller parity, history, copy and reload pass. |
| Accepted | M10/10 | [Frame-accurate Video time crop](evidence/mapgyver-m10-time-crop-2026-11-18.json). Both GPUs passed held and moving native preview/output, exact forward and reverse hard seams, smooth overlap, mixed-duration hold and live replacement. Typed callers and project operations also passed. |
| Accepted | M10/8 | [Per-channel subtitles](evidence/mapgyver-m10-channel-subtitles-2026-11-18.json). Typed ownership, callers, history/copy/reload, held and moving cues, reverse, full and cropped hard/smooth wraps, live relink, and native preview/output passed. Both GPUs verified moving pixels. |
| Accepted | M16/7 | [Matrix toolbar and caller parity](evidence/mapgyver-m16-matrix-transport-2026-11-18.json). Bundled keyboard and pointer checks invoke exact Parameters and cover nested Column traversal, top-level Groups and Stop. |
| Accepted | M16/8 | [Layer navigation and caller parity](evidence/mapgyver-m16-layer-transport-2026-11-18.json). Sparse, empty and disabled Cells, Layer Stop and restart, peer playback, preserved Column anchor and exact bundled row-header and Inspector pulses passed. |
| Accepted | M16/9 | [Column Group controls and caller parity](evidence/mapgyver-m16-column-group-transport-2026-11-18.json). First enabled Column, direct mixed children, nesting, empty Groups, reorder, bundled header/Inspector and script/transport passed. |
| Accepted | M16/10 | [Cell Auto Next verification](evidence/mapgyver-m16-cell-auto-next-2026-11-18.json). Specific Time, None, cropped End of Media, Stop, and a transition longer than the remaining active Video passed. |
| Accepted | M16/11 | [Previous/Next wait verification](evidence/mapgyver-m16-navigation-waits-2026-11-18.json). Direct, mixed loop boundaries and finite endpoints, scope, replacement and Stop cancellation pass. The bundled toolbar applies the real Inspector setting. |
| Accepted | M16/12 | [Transition line verification](evidence/mapgyver-m16-transition-lines-2026-11-18.json). Incoming/outgoing anchors, interruption, immediate Cut, natural completion, virtualized row entry, and editor-only and submitted Spout paths passed. |
| Accepted | M16/13 | [Empty Cell hover verification](evidence/mapgyver-m16-empty-cell-hover-2026-11-18.json). The contour survives crossing onto and off the Add Media button within the Cell, clears on exit, and permits mouse and keyboard activation. |
| Accepted | M16/14 | [Compact controls verification](evidence/mapgyver-m16-compact-controls-2026-11-18.json). Right-edge vertical Opacity and compact Blend Mode use the Inspector's typed Parameters. Keyboard access, Undo/Redo, narrow-dock layout, adaptive preview geometry and sparse reload pass. |
| Pending | M16/1 | [Video audio](evidence/mapgyver-m16-video-audio-2026-11-18.json) passed functional checks on a live WASAPI stream. Return in the hardware qualification batch with a controllable audio endpoint. Measure the routed output through OS-level capture, disconnect that exact device during playback and verify automatic restoration at the current transport time. |
| Accepted | M16/2 | [Separate loop Video and audio](evidence/mapgyver-m16-separate-loop-2026-11-18.json) passed native handoff, repeated seams, seek, Stop and automatic file recovery. Typed history, duplicate and reload pass. Larger-frame and sustained timing remain in the Video qualification lane. |
| Accepted | M16/3 | [Matrix Cell audio](evidence/mapgyver-m16-matrix-cell-audio-2026-11-18.json) passes inheritance, own-file replacement, audio-only Cells, native crossfade, virtual routing, Stop and project reload. The pre-existing relocation fixture remains in M09. |
| Accepted | M16/4 | [HTTP hierarchy and app defaults](evidence/mapgyver-m16-oscquery-hierarchy-2026-11-18.json), [UDP parameter edits](evidence/mapgyver-m16-oscquery-input-2026-11-18.json), [manager and metadata commands](evidence/mapgyver-m16-oscquery-commands-2026-11-18.json), [Zeroconf and rebind](evidence/mapgyver-m16-oscquery-lan-2026-11-18.json), [per-client feedback](evidence/mapgyver-m16-oscquery-feedback-2026-11-19.json) and [Copy OSC Address and Help](evidence/mapgyver-m16-oscquery-addresses-2026-11-19.json) verified. |
| Accepted | M16/5 | Surface Editor and Media Preview retain their previous image through a delayed Spout start. Same-channel and Reference Surface selections reused one live receiver. [Checks](evidence/mapgyver-m16-seamless-previews-2026-11-19.json) |
| Accepted | M16/6 | Linked Screen Warping outlines move together before the engine responds and through release and Undo/Redo. Native hits and independent Spout pixels passed. [Checks](evidence/mapgyver-m16-linked-outlines-2026-11-19.json) |
| Pending | M10/3 | User-deferred until requested. [WebP Picture](evidence/mapgyver-m10-webp-picture-2026-11-19.json) is verified. [MKV H.264/PCM/SubRip](evidence/mapgyver-m10-mkv-tracks-2026-11-19.json), [MKV HEVC/Opus/SubRip](evidence/mapgyver-m10-mkv-loop-2026-11-19.json), [legacy and DXV3 including translucent High Quality](evidence/mapgyver-m10-dxv-2026-11-19.json), and [active file replacement](evidence/mapgyver-m10-file-replacement-2026-11-19.json) passed functional checks. Sustained DXV3 Normal passed NVIDIA connected identity, visible deadlines, CPU, connected age and tracked retirement. Intel passed visible deadlines, connected age and CPU after sharing and conversion. The [direct-upload audit](evidence/mapgyver-m10-direct-upload-gpu-2026-11-19.json) bounds all tested DXV batches. Intel exceeds GPU budget and NVIDIA timing remains inconclusive. [Sustained MKV](evidence/mapgyver-m10-mkv-sustained-2026-11-19.json) passed NVIDIA connected identity, visible deadlines and tracked retirement. Minimized deadlines, missing witnesses, MKV 4K age, further profiles, repeats, recovery and license requirements remain open. |
| Accepted | M15/10 | [Reusable Gradient and animated Color](evidence/mapgyver-m15-gradient-2026-11-19.json). Typed stop order, duplicate, history, sparse reload and runtime constraints passed. The bundled Inspector edited stops and timing, and independent Spout verified RGBA, reload and Manual restoration. |
| Accepted | M03/4 | [Complete original recovery audit](evidence/mapgyver-m03-blocked-retirement-2026-11-06.json): actual SDK contention, real queued GPU waits, initial/endpoint recovery, healthy isolation, source/device recreation with old uses held, fresh pixels and zero-resource retirement. Cancellation never fabricates completion; M03/9 production budgets remain separate and required. |
| Accepted | M05/5 | [Actual physical recovery](evidence/mapgyver-m05-hotplug-2026-11-06.json): both GPUs, eleven physical red-pixel/native-window observations across hotplug and live negative-origin/restored arrangements. Authored targets and GPU identities unchanged; same HWND per case. Mixed-DPI proofs retained; original topology restored. |
| Blocked | M17/1 | [Bundled and fallback runtime startup](evidence/mapgyver-m17-ndi-startup-2026-11-19.json) passed live gain, loss, reconnect and missing or incompatible runtime guidance. Return with an independent LAN sender and clean Windows installation for A/V offset and drift, network recovery, final-use retirement and distribution. |
| Blocked | M17/2 | [Local SDK receipts](evidence/mapgyver-m17-ndi-screen-audio-2026-11-19.json) verify concurrent and minimized publication, live changes, a missing audio device, sender withdrawal and renderer retirement on New, and same-process reload. Return with an independent LAN receiver to measure offset, drift and recovery, and with a clean Windows installation to verify distribution. |
| Blocked | M18/1 | All [Medias](evidence/mapgyver-m18-medias-file-drop-2026-11-19.json), [editor](evidence/mapgyver-m18-editor-file-drop-2026-11-19.json) and [Matrix/App Control](evidence/mapgyver-m18-matrix-app-drop-2026-11-19.json) targets passed with disk-backed Windows WebView2 input. This session has no Computer Use JavaScript tool. Return with direct Explorer-to-MapGyver input to verify actual drags, rejection, Undo and reload. |
| Accepted | M18/2 | [Windows Project Settings checks](evidence/mapgyver-m18-project-settings-2026-11-19.json) verified the File menu, both toolbar icons, typed Project Folder values in two files and app-wide Preferences across project changes and restart. |
| Accepted | M18/3 | [Windows File Explorer check](evidence/mapgyver-m18-file-explorer-2026-11-19.json) verified Tree and Thumbnail Views, the default and override roots after reload, and a drop into Medias through the shared import intent. |
| Blocked | M18/4 | The [Shader Editor and file reload](evidence/mapgyver-m18-embedded-glsl-2026-11-19.json), [live Media textures](evidence/mapgyver-m18-shader-textures-2026-11-19.json), [ISF persistent passes](evidence/mapgyver-m18-isf-history-2026-11-19.json), [ShaderToy Buffer passes](evidence/mapgyver-m18-shadertoy-buffers-2026-11-19.json), [embedded ISF inputs](evidence/mapgyver-m18-isf-uniforms-2026-11-19.json), [embedded GLSL/ShaderToy uniforms and FragCoord metadata](evidence/mapgyver-m18-glsl-uniforms-2026-11-20.json), [File-mode uniforms](evidence/mapgyver-m18-file-uniforms-2026-11-20.json), [local Shader Library](evidence/mapgyver-m18-local-shader-library-2026-11-20.json), [online ISF catalog](evidence/mapgyver-m18-online-isf-library-2026-11-20.json) and [FragCoord.XYZ gallery](evidence/mapgyver-m18-fragcoord-library-2026-11-20.json) are verified. [ShaderToy API fixture](evidence/mapgyver-m18-shadertoy-library-2026-11-20.json) passed. Return when the API is reachable with a valid app key to browse, drop and render a supported live shader and reopen it. |
| Accepted | M18/5 | [Windows creation selection](evidence/mapgyver-m18-creation-selection-2026-11-20.json) verified both Channel add routes, explicit Channel selection, Matrix Add Media and Media drop selecting the Cell, and explicit nested Media inspection. |
| Accepted | M18/6 | [Standalone Chromium](evidence/mapgyver-m18-browser-spatial-2026-11-20.json) verified all four quality modes, throttled Auto and recovery, moving Media and Screen editor previews, held Composition edits, visible Matrix thumbnails, selected Cell content and retirement on disable, hide and close. |
| Waiting  | M09/1    | After all feature and retained qualification gates pass, including M17 and M18, complete Q01–Q06, actual-app statistics and Windows artifact checks, including high-refresh interaction. [Retained native Matrix case](evidence/mapgyver-m13-picture-acceptance-2026-11-07.json): after explicit Layer exposure, the timed three-source interruption assertion still fails; two experiments exhausted, cause unproven. Return at final qualification to this exact case after software prerequisites and reconcile actual captured sources against the original retention contract. No full five-test native target pass is claimed. Reconcile remaining older source-drop/Reference fixtures before final regression. | [Supplementary legacy Matrix UI failures](evidence/mapgyver-m13-transition-metadata-2026-11-07.json): the parameter fixture stops at Blend Mode after opacity; the viewport fixture’s center header clicks leave the Matrix inspected and its Time-control wait expires. Both hosts exit0, pages/rejections are clean, causes unproven. Return at final regression to reconcile current hit regions and selection/history settling, then complete both original fixture scopes; no full UI-suite pass is claimed. [Global product-tool contract debt](evidence/mapgyver-m10-bundled-runtime-2026-11-09.json): reconcile legacy app-local UI-watch and desktop-shutdown assertions with their current shared owners, then rerun the full tool contracts before final qualification.

**M09 parallel software return:** the default parallel Video suite fails the existing typed-policy
generic QuickJS callback assertion in `target/m14-video-status-tests-final-2026-11-16.log` and,
in another run, the timed-column preload `!idle.is_current()` assertion in
`target/m14-video-status-tests-complete-2026-11-16.log`. All110 Video tests pass sequentially,
with six existing hardware tests ignored. Cause is unproven; return before final qualification
with source-supported corrections and the original parallel command. No full parallel pass.

**M09 test-feature return:** [The new optional-boundary check](evidence/mapgyver-m10-source-allocation-2026-11-13.json)
found four unchanged native-test calls to qualification-only `capture_pixels` under bare `d3d11`.
Before final qualification, gate those fixtures with their required feature and rerun the original
all-target command. Without FFmpeg, the production d3d11 library and complete qualification
all-target strict checks already pass; no bare-d3d11 all-target pass is claimed.

## Evidence and handoff

- [Historical acceptance and batch evidence](mapgyver-evidence-through-2026-11-02.md) preserves
  the complete previous ledger, including the unfinished Matrix Parameter batch and rejected
  timing/memory proposals. Its old execution boards are non-operative.
- [Qualification contract](mapgyver-qualification-contract.md) owns retained budgets, revised
  Composition/Reference and Video workloads, and separately accounted software decode uploads.
- Native user observations: [Screen/Surface](evidence/mapgyver-m05-manual-editor-acceptance-2026-11-02.json),
  [Composition](evidence/mapgyver-m06-manual-editor-acceptance-2026-11-02.json),
  [Matrix playback](evidence/mapgyver-m07-manual-playback-acceptance-2026-11-02.json).
  Brief “preview unavailable” on restore was previously accepted for that scope; M11/1 now requires
  explicit flash/transition acceptance. Other timing/alignment budgets remain.
- Prior Matrix Parameter and icon/presentation work is preserved, unaccepted, in named stash
  `mapgyver-pre-revision-matrix-parameters-and-icons-2026-11-02`, object
  `f550ec0c03e907e9ebab44891538f1030358c30a`. Do not restore it automatically. Its typed-row
  approach is reconciled in the current conversion; its older model/host/presentation
  changes remain un-restored and unaccepted.
- [Shared Texture cadence repair](mapgyver-shared-texture-cadence.md) owns the resolved urgent
  investigation and its limits; fix `2d29e381` is pushed. Do not reopen unchanged source probes.
  Git delivery uses existing HTTPS credentials per command; the saved SSH remote is unchanged.
  Preserve concurrent user work and the running user's locked executable.
- **Current handoff — Matrix loop waits (2026-11-19):** [Checks](evidence/mapgyver-m16-navigation-waits-2026-11-18.json) confirm explicit Previous/Next Wait waits through the next loop boundary and remembers that crossing while slower peers finish. Extended Video remains deferred. No new whole gate was added or accepted.

## Feedback delivery cadence

- Public `mapgyver-status.md` is the user's only progress view: short, self-contained, ordered lists.
- Keep this ledger for exact gates/evidence. Synchronize both at each milestone.
- Per small feedback: finish the owning-layer change, format, run the smallest relevant behavioral/type/build checks once, and do a short app smoke when needed.
- Use `cargo test --profile dev` for focused software behavior without timing assertions, keeping the same suites/assertions and reusing development dependencies. Native/performance qualification retains its specified optimized build; a fast software run earns no performance credit.
- Reuse unchanged passing evidence. Do not repeat full Engine/app/cross-product builds by habit.
- Batch full regressions, Intel/GPU/profile/performance/recovery/edge matrices after remaining Video and M15–M18 feature delivery, before M09/release.
- Functional delivery may advance to the next feature with explicit qualification Pending. Never count unfinished deep checks as Accepted.
- Reproduced ordinary-use crashes, data loss or broken required behavior still interrupt feature delivery.

## Maintenance rule

Replace current state and the single current-batch/handoff entry in place. Keep this page
focused on progress, missing acceptance and next action. Link necessary detailed results in
owning reports/evidence; do not append daily logs, transcripts or duplicate dashboards.
At each implementation batch start, replace the handoff entry with deliverable, owner, proving
check and stopping time; at its end record only the current result and exact remaining scope.
