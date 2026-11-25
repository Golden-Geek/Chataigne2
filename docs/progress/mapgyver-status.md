# MapGyver status

**Updated 25 November 2026 · `█████████░` 86% accepted (130/151 checks, 9/20 phases complete)**

**Latest delivered:** Windows 4× anti-aliasing saves 126.6 MiB per tested 1080p or 4K Screen
generation. Pixel, allocation and retirement checks passed on Intel and NVIDIA.
[Checks](mapgyver-m14-antialiasing.md#direct-source-over-compositing--2026-11-25)

**Now:** NVIDIA 1080p still meets the timing budget. NVIDIA 4K and both Intel sizes improved
and still exceed it. Further rendering optimization and physical recovery verification remain.
[Timing](mapgyver-m14-antialiasing.md#sustained-comparison--2026-11-25)

**Manual action needed now:** None.

[Implementation plan](../plan/MapGyver_Implementation_Plan.md) · [Detailed progress and evidence](mapgyver-ledger.md)

## Cross-platform progress

Both Mac architectures passed relocated bundles and actual Rectangle and Fullscreen
pixel checks. Physical recovery and cadence still need a Mac with an external monitor.
Ubuntu 26.04 amd64 packages passed clean installs, X11 workbench readiness, H.264 playback
and Screen Warping. The Linux development build passed X11 output checks. Weston verified
Wayland Fullscreen live color and repeated retargeting at matching and mixed 1×/2× scales.
Xorg and Sway verified automatic output recovery for the open project and for a project
reopened with the selected connector offline. The Ubuntu 24.04 and 26.04 release
packages passed clean installation and all local desktop, media and virtual display
recovery checks. The updated hosted Ubuntu 24.04 job also passed the installed desktop
suite in clean Ubuntu.
Physical Linux display recovery and cadence need an external monitor. The native
Debian 13 ARM64 package also passed clean install, authoring, workbench, media and
virtual display recovery. On-device build and Screen output still need a Raspberry Pi.
The armhf package built and passed clean install and project authoring in emulated
Debian 13. Hosted WebKit did not load the workbench, and local emulated Solid Color
preview aborted. Desktop, media and output qualification requires a 32-bit Pi.
The Windows ARM64 app and FFmpeg SDK built natively. The installer passed silent
install, embedded-workbench startup in headless mode and silent uninstall. Installed
headless authoring, history, Save/New/Load and release WebView2 startup passed.
The app loaded all five FFmpeg DLLs from its installed directory. The hosted Screen
opened at the authored bounds, and a Windows system window covered the sample point
even after moving the workbench. Hosted CI verified installed H.264 Screen output through
an independent native Spout receiver. Physical output and recovery require an
unoccluded ARM64 desktop. Interactive authoring, Media Preview and native NDI
remain open.
[macOS pixel checks](https://github.com/Golden-Geek/Chataigne2/actions/runs/45019118144) ·
[Ubuntu 24.04 package CI](https://github.com/Golden-Geek/Chataigne2/actions/runs/45035873220/job/137249983856) ·
[Debian 13 ARM64 package CI](https://github.com/Golden-Geek/Chataigne2/actions/runs/45041349161/job/137266298921) ·
[Debian 13 armhf package CI](https://github.com/Golden-Geek/Chataigne2/actions/runs/45041349161/job/137266299198) ·
[Windows ARM64 installer and Spout CI](https://github.com/Golden-Geek/Chataigne2/actions/runs/45692693008/job/139484347947)

## Done in normal use

- **Editor recovery.** A Source drag is one Undo step after reconnect. New protects the
  replacement project from old edits. Screen, Source and Matrix recovery passed on Intel
  and NVIDIA. [Checks](mapgyver-m14-editor-recovery.md)
- **Live editor feedback.** Screen Warping, Surface Source, Composition and Matrix draw smooth
  hover and selection feedback. Keyboard focus is clear, reduced motion is respected, and
  switching Matrix Cells passed on Intel and NVIDIA. [Checks](mapgyver-m15-editor-feedback.md)
- **Smooth Frame/Home.** Screen Warping, Surface Source, Composition and both Matrix layout
  views kept the rendered image aligned with its controls on Intel and NVIDIA. Includes
  resizing during animation and navigating to a selection beyond the first Matrix page.
  [Checks](mapgyver-m14-navigation.md#second-pass-verification--2026-11-20)
- **Video time crop.** Media Preview and Spout showed the same complete forward and reverse frame sequences on Intel and NVIDIA. Short-channel hold, live replacement, and crop recovery passed. [Checks](evidence/mapgyver-m10-time-crop-2026-11-18.json)
- **Auto-Smooth Loop.** Forward and reverse blends, mixed channel lengths, live file recovery, and output cleanup on New passed on Intel and NVIDIA. [Checks](evidence/mapgyver-m10-fade-loop-2026-11-18.json)
- **Per-channel subtitles.** Independent captions stay aligned during playback, reverse, loop wraps, and live file replacement. Media Preview and output passed on Intel and NVIDIA. [Checks](evidence/mapgyver-m10-channel-subtitles-2026-11-18.json)
- **Fading Layer Stop.** Fade timing, override precedence and process cleanup passed on Intel and NVIDIA. Other Layers keep playing. [Checks](mapgyver-m14-layer-stop.md#second-pass-verification-2026-11-20)
- **Matrix enablement.** Layer and Cell visibility, inactive editing and exact reload passed on Intel and NVIDIA. [Checks](mapgyver-m14-matrix-enabled.md#second-pass-verification-2026-11-20)

- **Browser Media Preview.** Standalone Chromium showed moving textures and stopped the renderer when the panel was hidden. [Checks](evidence/mapgyver-m18-browser-media-preview-2026-11-20.json)
- **Browser preview quality.** Disabled, Auto, Forced Low Quality and Forced High Quality passed in Media Preview. Auto switched down on a constrained connection and switched back when bandwidth returned. [Checks](evidence/mapgyver-m18-browser-quality-2026-11-20.json)
- **Browser Screen editors.** Screen Warping shows live warped imagery and respects the calibration crop. Surface Source shows the full image with aligned crop controls. [Checks](evidence/mapgyver-m18-browser-screen-editors-2026-11-20.json)
- **Browser Composition and Matrix.** Composition follows a held drag. Matrix loads moving thumbnails for visible Cells and shows the selected Cell in its editor. Closing the tab releases the renderer. [Checks](evidence/mapgyver-m18-browser-spatial-2026-11-20.json)
- **Startup display.** Fresh projects prefer the lowest available external Windows display
  number. The current build selected RTK FHD HDR with the Optoma projector also active.
  The installed Start menu beta `1.0.0b7` predates this fix. Saved projects retain their
  Screen targets. Automated browser tests leave physical outputs off by default.
  [Checks](mapgyver-m14-screen-defaults.md#numbered-display-feedback)
- **Physical RTK tests.** Startup and File → New displayed the expected image on RTK on Intel
  and NVIDIA. The restricted test mode opened zero Optoma windows.
  [Checks](mapgyver-m14-screen-defaults.md#physical-rtk-startup-and-new)
- **Clear a Fullscreen display.** Choose **No display selected** in the Screen Inspector.
  The Screen stays unconfigured after reopening. Undo and Redo passed with both displays connected.
  [Checks](mapgyver-m14-screen-defaults.md#clearing-a-fullscreen-target)
- **Shader Editor.** Edit embedded GLSL beside its preview or select a file that reloads on change.
  Windows output passed for applied edits, file replacement, recovery and saved-project reload.
  [Checks](evidence/mapgyver-m18-embedded-glsl-2026-11-19.json)
- **Shader formats.** The Format selector plays procedural ShaderToy Image shaders and
  single-pass ISF generators. ISF uses metadata defaults. Saving in File mode preserves
  embedded code. [Checks](evidence/mapgyver-m18-shader-formats-2026-11-19.json)
- **Shader texture inputs.** Pick up to four Media or Channels in the Shader Editor.
  Live changes, removal, Undo and project reload passed. [Checks](evidence/mapgyver-m18-shader-textures-2026-11-19.json)
- **ISF multipass.** Ordered passes can sample earlier named temporary textures at different sizes
  and float precision. Live Media input, file replacement and project reload passed.
  [Checks](evidence/mapgyver-m18-isf-multipass-2026-11-19.json)
- **ISF previous-frame buffers.** Persistent targets accumulate across frames and survive
  compatible input edits. History resets on a buffer resize, source change or project reopen.
  [Checks](evidence/mapgyver-m18-isf-history-2026-11-19.json)
- **ShaderToy Buffer passes.** Saved JSON effects run Common, Buffer and Image code with live
  Media inputs and previous-frame feedback. Project reopen and file reload passed.
  [Checks](evidence/mapgyver-m18-shadertoy-buffers-2026-11-19.json)
- **Embedded ISF uniforms.** Add typed inputs from the Shader Editor. Live edits, Undo/Redo
  and project reopen passed. [Checks](evidence/mapgyver-m18-isf-uniforms-2026-11-19.json)
- **GLSL and ShaderToy uniforms.** Add individual declarations or choose Add All. FragCoord
  defaults and ranges, live edits, shared ShaderToy Common values and project reopen passed.
  [Checks](evidence/mapgyver-m18-glsl-uniforms-2026-11-20.json)
- **File-mode uniforms.** Replacing an external shader refreshes the controls in place.
  Matching values survive the change and project reopen.
  [Checks](evidence/mapgyver-m18-file-uniforms-2026-11-20.json)
- **Local Shader Library.** Browse the project folder and drag GLSL, ISF or ShaderToy files
  into Medias. Windows verified all three formats, Undo/Redo and project reopen.
  [Checks](evidence/mapgyver-m18-local-shader-library-2026-11-20.json)
- **Online ISF Library.** Choose ISF (VIDVOX), search, and drag into Medias or a folder.
  The shader is embedded. Windows verified Undo/Redo, rendered output and reopen.
  [Checks](evidence/mapgyver-m18-online-isf-library-2026-11-20.json)
- **FragCoord.XYZ.** Browse and search the gallery, then drag a licensed GLSL shader into
  Medias. Main and Common source, attribution and license are embedded. Windows verified
  Undo/Redo, rendered output and reopen. [Checks](evidence/mapgyver-m18-fragcoord-library-2026-11-20.json)
- **Inspector selection.** Adding a Channel keeps the parent Media inspected. Adding or
  dropping Media into a Matrix Cell inspects the Cell. You can still inspect either child
  from its context menu. [Checks](evidence/mapgyver-m18-creation-selection-2026-11-20.json)
- **File Explorer.** Browse the project folder or a saved override in Tree View or
  Thumbnail View. A Windows app test dragged a local picture into Medias and undid the import.
  [Checks](evidence/mapgyver-m18-file-explorer-2026-11-19.json)
- **Project Settings.** Open from File or the top bar. The Project Folder is saved
  with each project. The separate Preferences button opens app-wide settings. Switching
  projects and restarting MapGyver passed. [Checks](evidence/mapgyver-m18-project-settings-2026-11-19.json)
- **File drops.** Import into Medias, Screen Warping, Composition and Matrix. Drop an
  `.exe` onto a Matrix Column, Column Group or Cell to set its App Control file. Windows WebView2
  checks covered these targets. [Medias](evidence/mapgyver-m18-medias-file-drop-2026-11-19.json) ·
  [Editors](evidence/mapgyver-m18-editor-file-drop-2026-11-19.json) ·
  [Matrix App Control](evidence/mapgyver-m18-matrix-app-drop-2026-11-19.json)
- **Workbench and projects.** Save/load, Undo/Redo, copies, managers and folders, shared
  selection and Inspector, editor targeting/locking, safe parameter controls, custom colors,
  and reusable Gradients with Color animation.
- **Screens, Surfaces and Composition.** Automatic startup Screen and Surface, native output,
  calibration, warping and crop, related Surface views, linked handles, alignment guides,
  Composition media, transforms and stacking, and display reconnect and rearrangement in
  the tested setups.
- **Matrix.** Layers, Columns, Cells and Groups, media drops and References, crossfades and mix
  indicators, navigation and Stop, Auto Next and wait modes, presets, compact controls, and
  nested Group and selection styling.
- **Media.** Picture (including WebP), Solid Color, Shared Texture with reconnect and seamless
  preview switching, and Video playback, seeking, cropping and smooth or separate-file loops.
  Local Video checks include common H.264, HEVC, ProRes and HAP, plus tested DXV and MKV
  variants, per-channel track selection, subtitles and file replacement.
- **Audio, App Control and OSCQuery.** Virtual audio outputs, Video and Matrix Cell routing,
  ordinary 64-bit App Control, and OSCQuery browsing, edits, LAN discovery, feedback and
  address copying.
- **NDI with audio.** Available from first launch in the Windows x64 package, with the NDI
  runtime included. If the bundled DLL fails, MapGyver tries a compatible configured or installed
  runtime automatically. Media picture, gain and reconnect passed using the fallback. Saved
  Screen outputs resumed after restart and project reload. [Startup checks](evidence/mapgyver-m17-ndi-startup-2026-11-19.json)
- **NDI runtime guidance.** An affected Media or Screen shows how to restore NDI if no compatible
  runtime can load. Ordinary Shared Texture output kept running in both tests.
  [Checks](evidence/mapgyver-m17-ndi-startup-2026-11-19.json)
- **Concurrent NDI Screens.** Both delivered picture and stereo audio while minimized. Audio route
  changes kept the renderer running. A live format change rebuilt it and the peer receiver recovered.
  [Checks](evidence/mapgyver-m17-ndi-screen-audio-2026-11-19.json)
- **NDI device and project recovery.** Routed audio kept sending with the selected local audio
  device missing. New withdrew both senders. Opening the saved project restored them in the same app.
  [Checks](evidence/mapgyver-m17-ndi-screen-audio-2026-11-19.json)
- **4× anti-aliasing memory.** Two Surfaces fit the 4K cap. Intel and NVIDIA passed pixel,
  memory pressure and cleanup checks. [Checks](mapgyver-m14-antialiasing.md#4k-scratch-allocation--2026-11-20)

## Next, in order

1. **4× anti-aliasing.** Reduce GPU render cost for NVIDIA 4K and Intel 1080p/4K (M14/8).
2. **Display qualification.** Verify simultaneous multi-display output, hot-plug and native Ctrl+N (M14/6).
3. **Retained qualification.** Complete the open checks below before the final Windows app and installer validation.

## Qualify later

- **ShaderToy Library.** App-key entry, search, import, Undo/Redo and reopened output passed
  with a local API fixture. Live verification needs a reachable ShaderToy API and a valid app key.
  [Checks](evidence/mapgyver-m18-shadertoy-library-2026-11-20.json)

### Needs a physical setup or direct input

- **Editor pointer input.** OS-level input and same-frame hit verification need a desktop session with Computer Use or hands-on input. [Checks](mapgyver-m14-editor-recovery.md)
- **Auto-Smooth Loop device loss.** An elevated Windows test session is needed to verify GPU recovery during an active blend. Wider shared-consumer and storage checks are still pending. [Checks](evidence/mapgyver-m10-fade-loop-2026-11-18.json)
- **File drops.** Direct Windows File Explorer drags need hands-on input or a desktop session
  that can drag between apps. Verify the supported targets, rejection, Undo and reload.
  [Cases](evidence/mapgyver-m18-matrix-app-drop-2026-11-19.json)
- **Displays.** Verify output defaults with one and two external monitors, occupied destinations
  and the native New shortcut. Finish mixed-display timing and high-refresh interaction checks.
  [Display checks](mapgyver-m14-screen-defaults.md)
- **Video audio.** Capture the routed physical output and disconnect and reconnect that endpoint
  during playback. [Audio checks](evidence/mapgyver-m16-video-audio-2026-11-18.json)
- **NDI.** Use an independent LAN sender and receiver for A/V sync, drift and recovery. Verify a
  clean Windows installation and the remaining lifecycle cases on that setup.
  [NDI gates](mapgyver-ledger.md#execution-board-and-return-queue)
- **Shared Texture.** Finish NVIDIA/D3D9, adapter loss and remaining format/allocation cases
  when matching sender and hardware are available. [Compatibility checks](evidence/mapgyver-m13-app-catalog-2026-11-07.json)

### Engineering and release checks

- **Editor and Matrix.** Finish wider shared-consumer retirement and Matrix stress.
  [Recovery cases](evidence/mapgyver-request-lifetime-deferred-2026-11-17.json)
- **Video, GPU and long-run resources.** Finish device recovery, sustained codec and
  shared-consumer timing, 4× anti-aliasing cost and recovery, and peak-memory growth. **Extended formats are deferred
  until requested.** DXV and MKV still have open 4K timing, profile and recovery checks.
  [Qualification queue](mapgyver-ledger.md#execution-board-and-return-queue)
- **32-bit App Control.** Fix and verify the remaining WOW64 retirement case.
  [Retirement checks](mapgyver-m08-retirement.md)
- **Final Windows app and installer.** After the retained checks, fix the known parallel Video
  and older Matrix/tool-fixture failures, then run the full app and installer regression.
  [Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)
