# MapGyver — simple status

- **Updated:** 22 November 2026.
- **Overall plan acceptance:** `█████████░` **86% — 130/151 checks accepted, 9/20 phases complete**. M19 adds six platform gates.
- **Latest verified:** 4× AA fits the memory cap. Sustained timing passed at 1080p on NVIDIA. NVIDIA 4K and both Intel sizes still exceed the GPU budget. [Checks](mapgyver-m14-antialiasing.md#sustained-shared-texture-timing--2026-11-20)
- **Plan execution:** macOS first, then desktop Linux and Raspberry Pi (armhf and aarch64). Windows ARM64 has a separate gate.
- **Manual action needed from you now: none.**
- **Next:** Verify the current Ubuntu 26.04 release package and the Ubuntu 24.04 desktop exit, media and output checks (M19/3). The corrected Weston CI command passed locally at all four matching and mixed scales. Xorg and Sway recovered the saved Screen after a live disconnect and reload while offline. Physical Mac hotplug and cadence await an external monitor. [Cross-platform evidence](mapgyver-ledger.md)

## Done — code and normal-use checks passed

- [x] **Parameter controls:** out-of-range values clamp safely; Opacity/Video Speed animation works; bindings/scripts update correctly; Undo and reload work, including missing range/step metadata.
- [x] **Projects:** save/load, normal Undo/Redo and copy/duplicate workflows.
- [x] **Project Settings:** Project Folder persists in each project. Preferences are app-wide and have a separate toolbar icon. [Checks](evidence/mapgyver-m18-project-settings-2026-11-19.json)
- [x] **File Explorer:** open the project folder or its override, browse Tree and Thumbnail Views and drag files onto the import targets. [Checks](evidence/mapgyver-m18-file-explorer-2026-11-19.json)
- [x] **Workbench:** media/screen managers, folders, shared selection, generic Inspector, contextual menus and panel targeting/locking.
- [x] **Pictures and Solid Color:** real channel settings/files, independent copies, file reload and color controls.
- [x] **Shared Texture:** receiving, previews and preview cadence, sender retention across Manual/Auto, and ordinary reconnect. New sources pick the first external sender or None. **(internal)** marks Screen outputs. Native pixel checks passed for deliberate internal selection, same-name takeover and external recovery.
- [x] **Screens/Surfaces:** creation, warping, crop/source editing, opacity, enable/disable, calibration and native outputs.
- [x] **Startup display:** Fresh projects prefer the lowest available external Windows display number. The current build chose RTK FHD HDR with the Optoma also active. The installed Start menu beta `1.0.0b7` predates this fix. Saved projects retain their Screen targets. Automated browser tests leave physical outputs off by default. [Checks](mapgyver-m14-screen-defaults.md#numbered-display-feedback)
- [x] **Clear a Fullscreen display:** Choose **No display selected** in the Screen Inspector. The Screen stays unconfigured after reopening. [Checks](mapgyver-m14-screen-defaults.md#clearing-a-fullscreen-target)
- [x] **Physical RTK tests:** Startup and File → New show the expected image on RTK on both GPUs. [Checks](mapgyver-m14-screen-defaults.md#physical-rtk-startup-and-new)
- [x] **Smooth Frame/Home:** shared native image and control alignment, Matrix selection beyond page one, and dock resize during animation passed on Intel and NVIDIA. [Checks](mapgyver-m14-navigation.md#second-pass-verification--2026-11-20)
- [x] **Live editor feedback:** native paint and keyboard focus in Screen Warping, Surface Source, Composition and Matrix passed on Intel and NVIDIA. [Checks](mapgyver-m15-editor-feedback.md)
- [x] **Editor recovery:** exact Source Undo after reconnect, clean New while dragging, and restored locked editor panels. [Checks](mapgyver-m14-editor-recovery.md)
- [x] **Shader Editor:** edit embedded GLSL beside its preview or choose a file that reloads on change. Windows Screen output passed for applied edits, file replacement, recovery and saved-project reload. [Checks](evidence/mapgyver-m18-embedded-glsl-2026-11-19.json)
- [x] **Shader formats:** procedural ShaderToy Image and single-pass ISF playback. ISF metadata defaults and saved-project reload pass. Embedded shader code stays saved in File mode. [Checks](evidence/mapgyver-m18-shader-formats-2026-11-19.json)
- [x] **Shader texture inputs:** pick up to four Media or Channels. Live updates, removal, Undo and project reload passed. [Checks](evidence/mapgyver-m18-shader-textures-2026-11-19.json)
- [x] **ISF multipass:** ordered passes sample earlier named temporary textures. Live Media input, file replacement and reopening passed. [Checks](evidence/mapgyver-m18-isf-multipass-2026-11-19.json)
- [x] **ISF previous-frame buffers:** verified accumulation, history across compatible edits, and clean resets. [Checks](evidence/mapgyver-m18-isf-history-2026-11-19.json)
- [x] **ShaderToy Buffer passes:** Common, Buffer and Image code with live Media input and previous-frame feedback. Project reopen and file reload passed. [Checks](evidence/mapgyver-m18-shadertoy-buffers-2026-11-19.json)
- [x] **Embedded ISF uniforms:** Add typed inputs in the Shader Editor. Live edits, Undo/Redo and project reopen passed. [Checks](evidence/mapgyver-m18-isf-uniforms-2026-11-19.json)
- [x] **GLSL and ShaderToy uniforms:** Add individual declarations or choose Add All. FragCoord defaults and ranges, live edits and project reopen passed. [Checks](evidence/mapgyver-m18-glsl-uniforms-2026-11-20.json)
- [x] **File-mode uniforms:** The open controls follow external shader replacements. Matching values survive the change and project reopen. [Checks](evidence/mapgyver-m18-file-uniforms-2026-11-20.json)
- [x] **Local Shader Library:** Browse and drop GLSL, ISF and ShaderToy files. Windows verified rendered output, Undo/Redo and project reopen. [Checks](evidence/mapgyver-m18-local-shader-library-2026-11-20.json)
- [x] **Online ISF Library:** Search VIDVOX's catalog and drag shaders into Medias or a folder as embedded code. Windows verified Undo/Redo, output and reopen. [Checks](evidence/mapgyver-m18-online-isf-library-2026-11-20.json)
- [x] **FragCoord.XYZ:** Browse and import licensed GLSL Main and Common source with attribution. Windows verified Undo/Redo, output and reopen. [Checks](evidence/mapgyver-m18-fragcoord-library-2026-11-20.json)
- [x] **Inspector selection:** Channel creation keeps the Media inspected. Matrix Media creation and drops inspect the destination Cell. Both children can still be inspected explicitly. [Checks](evidence/mapgyver-m18-creation-selection-2026-11-20.json)
- [x] **Initial document/editors:** startup and New create and immediately show the automatic Screen and Surface. Hidden/reopened panels, saved targets/locks, reload and empty projects pass; no extra Undo steps.
- [x] **Surface Editor display:** related Surfaces across Screens/References, independent toggles/locks, bounded paging, selection/Inspector and Undo/reload. ColorBars/Grid work without a source and obey its crop in Screen Warping/output mapping. Native patterns/contours, cropped ColorBars/Grid, mixed pattern/channel panels and70-Surface toolbar paging pass.
- [x] **Surface-handle links:** same-Screen snapping/link/unlink, supplied colored/grey magnet and live updates of all peers. Five native drag/link/Undo cycles, three-member groups, held-pointer GPU images and foreground image/control alignment pass.
- [x] **Object custom colors:** Screens, Surfaces, Medias/References and nested Folders; generic Inspector set/reset, matching accents, scripts, Undo/Redo, independent copies and reload. Normal/default and light/dark checks pass.
- [x] **Output settings:** GPU preferences, per-Screen frame-rate settings and optional/custom sender names.
- [x] **Display recovery:** the previously tested unplug/replug and monitor-rearrangement cases.
- [x] **Composition:** owned media, transforms, front/back ordering, contextual actions, enable state and zero-scale handling.
- [x] **Matrix:** Layers, Columns, Cells, Groups, sparse scrolling, media drops/copies/references and normal playback/crossfades.
- [x] **Matrix transport:** top-left Previous/Next Column, Previous/Next top-level Column Group and Stop All. The same triggers are in the Inspector. Keyboard, script and transport checks passed. [Checks](evidence/mapgyver-m16-matrix-transport-2026-11-18.json)
- [x] **Layer transport:** Previous/Next skips empty and disabled Cells and keeps the Matrix Column and other Layers playing. Row header, Inspector, script and transport checks passed. [Checks](evidence/mapgyver-m16-layer-transport-2026-11-18.json)
- [x] **Column Group transport:** Play starts the first enabled Column. Previous/Next steps through direct children, counting each nested Group as one step. Reorder, keyboard, Inspector, script and transport checks passed. [Checks](evidence/mapgyver-m16-column-group-transport-2026-11-18.json)
- [x] **Cell Auto Next:** advances one Layer after a timer or at the cropped Video’s end. Transition lead, Stop cancellation and unchanged Undo history passed. [Checks](evidence/mapgyver-m16-cell-auto-next-2026-11-18.json)
- [x] **Previous/Next wait modes:** launch immediately or wait for the first or all active Videos in scope. A looping Video counts at its next wrap. Repeated commands replace the wait, and Stop cancels it. [Checks](evidence/mapgyver-m16-navigation-waits-2026-11-18.json)
- [x] **Matrix transition indicators:** green Cell and Column lines show the current mix through crossfades, interruptions, Cuts and scrolling. Verified in the Matrix editor and with a live Spout output. [Checks](evidence/mapgyver-m16-transition-lines-2026-11-18.json)
- [x] **Empty Cells:** clicking an unused Cell keeps the current selection. The `+` button opens Add Media, and crossing onto it keeps the background and outline steady. Mouse and keyboard Add Media access both passed. [Checks](evidence/mapgyver-m16-empty-cell-hover-2026-11-18.json)
- [x] **Compact Matrix controls:** slim vertical Opacity sliders sit inside the right edge of each Layer header. Opacity and Blend Mode use the Inspector's Parameters. Browser checks verified the slider layout at 100%, 125%, and 150% scaling. [Earlier control checks](evidence/mapgyver-m16-compact-controls-2026-11-18.json)
- [x] **Matrix selection/colors:** only explicitly selected objects highlight; modifier multiselection works; Layers/Columns/Cells/Groups show Golden custom colors. Inspector color edits, Undo, copies and reload pass focused checks.
- [x] **Matrix color menus:** right-click Layers, Cells, Columns and both Group kinds to Set Color / Reset Color. Shared picker, grouped Undo/Redo, empty Cells, unchanged multiselection, Inspector synchronization, offscreen access and reload pass.
- [x] **Compact Layer Groups:** one-line headers; Opacity/Blend Mode in the Inspector. Nested/empty collapse, hidden selection, Undo/Redo, old/new projects, reload and scaled/narrow geometry pass.
- [x] **Nested Group membership:** Inspector-inspired tinted/rounded wraps, inset child selection contours, separate nesting levels, custom/reset colors, disabled state, collapse/reparent/reorder and offscreen ancestors. Light/dark, narrow/scroll and input checks pass.
- [x] **Geometry alignment:** edge/center snapping, colored magnet and temporary guides in Surface Source, Composition and Matrix. Move/resize, zoom/pan, toggle, Undo/Redo and held native Composition pixels pass.
- [x] **Matrix appearance:** quieter borders, small rounded corners and a short active-Cell shadow. Selection/colors remain distinct; reduced motion works; app visual review and focused scaling/narrow-grid checks pass.
- [x] **Column enable/disable:** Inspector toggles mute current/outgoing content; group inheritance, disabled editing, Undo, copy/reload and resume pass focused checks.
- [x] **Matrix presets:** capture/update/rename/delete/recall Layer/Column order and enabled states; contextual menus, commands and named scripts. Paged member Inspector editing, offscreen access, Undo/Redo, copies/reload and live/outgoing playback pass. Cells stay intact.
- [x] **App Control:** normal 64-bit launch/readiness/sharing/reconnect/stop workflows. The remaining 32-bit case is listed below.
- [x] **Video:** common H.264/HEVC/ProRes profiles and HAP; playback, loop, seek/frame stepping, transport Inspector and Matrix activation/deactivation policies.
- [x] **Video fade-loop:** enable Loop and set **Auto-Smooth Loop** in the Video Inspector. Zero gives a hard loop. Direct and Reference output show the same held blend, and playback resumes from the part of the beginning already shown.
- [x] **Video time cropping:** Start and End bound playback, seeking, stepping and loops. Seeking to End holds the last frame in the crop. NVIDIA numbered output, smooth-loop Reference parity and save/reload passed.
- [x] **Video subtitles are per channel.** Select the Video in Medias, expand **Inspector → Channels → [channel name] → Subtitles**, enable the folder, and choose that channel’s embedded track or subtitle file. Size, font, and position are also per channel. Editing one preserved the other’s output with two different files and with two channels sharing one MKV. Both MKV selections survived reload. [Two files](evidence/mapgyver-m10-channel-subtitles-2026-11-18.json) · [Shared MKV](evidence/mapgyver-m10-mkv-tracks-2026-11-19.json)
- [x] **MKV playback with track selection.** Verified H.264 with stereo PCM and embedded SubRip. Choose **Inspector → Channels → [channel name] → Audio → Track** to switch tracks during playback. Automatic follows the file’s default, and a missing explicit track is silent. Selections survived reload. [Checks](evidence/mapgyver-m10-mkv-tracks-2026-11-19.json)
- [x] **WebP Pictures.** Choose a still `.webp` in **Import Picture** or set a Picture channel's **File**. Lossy and lossless transparency and EXIF rotation passed. [Checks](evidence/mapgyver-m10-webp-picture-2026-11-19.json)
- [x] **DXV playback.** Open DXV MOV files as a Video channel File. Legacy and DXV3 Normal and High Quality passed native pixel checks. Playback, seeking and translucent compositing passed for Normal and High Quality. [Checks](evidence/mapgyver-m10-dxv-2026-11-19.json)
- [x] **Moving MKV tracks and loops.** HEVC with two Opus and two SubRip tracks passed live switching and cropped hard and smooth loops. Native picture and caption samples matched the corresponding WASAPI mixer signals. [Checks](evidence/mapgyver-m10-mkv-loop-2026-11-19.json)
- [x] **Video file recovery.** Missing or malformed input clears stale track options and silences the channel while keeping its selections. Restoring a valid MKV at the same path recovered picture, captions, and audio during playback. Relinking to animated WebP and DXV also passed. [Checks](evidence/mapgyver-m10-file-replacement-2026-11-19.json)
- [x] **Editor manipulation:** the smoothness improvements you accepted in Screen Warping, Surface Source and Composition.
- [x] **Editor feedback — functional:** shared120ms hover/selection paint, softer handles, keyboard cues and reduced motion. Geometry/input stay immediate; browser and native Screen/Source input checks pass.
- [x] **App presentation:** MapGyver identity/icon, official calibration pattern, font/menu/Inspector improvements and thumbnail Fit behavior.
- [x] **Audio Preferences:** enable output, choose System Default or a specific device, and add or remap virtual outputs. Native stream, missing-device selection, project replacement and restart passed. [Checks](evidence/mapgyver-m16-audio-output-settings-2026-11-18.json)
- [x] **Video audio:** open **Inspector → Channels → Audio** to assign the left and right signals to named virtual outputs, set Volume, or mute that channel. Two clips, live remapping, gain, pause/seek/stop, device reopening and project reload passed. [Checks](evidence/mapgyver-m16-video-audio-2026-11-18.json)
- [x] **Single-file Video intro and loop:** enable Loop and set **Loop Start** to a source time. The intro plays once and the picture and audio repeat the tail. Natural wraps, held smooth blend, seek, Stop, and saved settings passed. [Checks](evidence/mapgyver-m16-video-intro-loop-2026-11-18.json)
- [x] **Separate Video loop files:** enable **Loop** on the Media, then set **Loop File** on each Video channel. Set **Audio → Loop Audio File** to override the embedded audio during the repeat. Native handoff, two wraps, held smooth blend, seek, Stop and missing-file recovery passed. Undo, copy and reload passed. [Checks](evidence/mapgyver-m16-separate-loop-2026-11-18.json)
- [x] **Matrix Cell audio** is under **Inspect Cell settings → Audio**. Choose **Media** to inherit the selected Video channel’s audio, or **File** to use its own routed audio. Use **File** on an empty Cell for audio-only playback. All modes, crossfade, Stop and reload passed on live WASAPI output. [Checks](evidence/mapgyver-m16-matrix-cell-audio-2026-11-18.json)
- [x] **Local OSCQuery browsing** is under **Preferences → OSCQuery**. Enable the folder to browse the live hierarchy and parameter values on the same machine. MapGyver defaults to 43000 and Chataigne to 42000. Both apps passed activation, saved port and project replacement checks. [Checks](evidence/mapgyver-m16-oscquery-hierarchy-2026-11-18.json)
- [x] **Local OSC edits:** send OSC to a parameter's published address on the same port. Values and triggers use Golden validation and history. Changing Surface Opacity from an OSC client, then using Undo and Redo, passed in MapGyver. [Checks](evidence/mapgyver-m16-oscquery-input-2026-11-18.json)
- [x] **Local OSCQuery commands:** add, move, remove and edit nodes over WebSocket with the workbench's permissions and history. MapGyver passed Screen naming, color, removal and Undo/Redo, plus Video creation from the Media catalog. [Checks](evidence/mapgyver-m16-oscquery-commands-2026-11-18.json)
- [x] **OSCQuery on the LAN:** enable **Allow LAN Control** under **Preferences → OSCQuery** for Zeroconf discovery and control from the active IPv4 subnet. Discovery, port changes, Surface Opacity over UDP, saved settings across project replacement, and return to local-only access passed. [Checks](evidence/mapgyver-m16-oscquery-lan-2026-11-18.json)
- [x] **Live OSCQuery feedback:** each client receives the values and triggers it subscribes to. Individual updates arrive immediately and bursts are bundled. Two-client isolation, trigger delivery and project replacement passed. [Checks](evidence/mapgyver-m16-oscquery-feedback-2026-11-19.json)
- [x] **OSC addresses:** right-click a node to copy its address. Enable **Preferences → Interface → Show OSC Address in Help** to see the hovered node's address in the bottom bar. Copy, duplicate names, move/Undo and project replacement passed. [Checks](evidence/mapgyver-m16-oscquery-addresses-2026-11-19.json)
- [x] **Seamless previews:** Surface Editor and Media Preview held the previous image until a delayed Spout sender produced frames. Switching between Surfaces using the same channel, including through a Reference, reused one live receiver. [Checks](evidence/mapgyver-m16-seamless-previews-2026-11-19.json)
- [x] **Linked outlines:** both Screen Warping outlines follow the latest drag before the engine replies, including with snapping off. Release, Undo/Redo, native hit testing and independent Spout pixels passed. [Checks](evidence/mapgyver-m16-linked-outlines-2026-11-19.json)
- [x] **Gradient and Color animation:** choose **Animation** on a Color parameter and expand **Gradient** to edit its stops. Standalone Gradients are available in a Golden Context. Native Spout verified the color blend, alpha, reload, and return to Manual. [Checks](evidence/mapgyver-m15-gradient-2026-11-19.json)
- [x] **File drops:** Medias, Screen Warping, Composition and Matrix imports passed Windows WebView2 checks. Matrix also accepts `.exe` files for App Control. Direct File Explorer verification remains below. [Medias](evidence/mapgyver-m18-medias-file-drop-2026-11-19.json) · [Editors](evidence/mapgyver-m18-editor-file-drop-2026-11-19.json) · [Matrix](evidence/mapgyver-m18-matrix-app-drop-2026-11-19.json)

## Planned features

[NDI with audio](../plan/MapGyver_Implementation_Plan.md#m17--deliver-ndi-media-and-screen-output) is implemented and verified locally.

- **NDI Media.** The Windows x64 package includes the NDI runtime and falls back automatically if the bundled DLL fails. A fresh profile verified picture, gain, mute and sender recovery. Clean Windows installation and cross-machine A/V checks remain pending. [Startup checks](evidence/mapgyver-m17-ndi-startup-2026-11-19.json)
- **NDI Screen Output.** Choosing NDI on an enabled Screen started picture and routed audio without activation. Both resumed after restart and reopening the saved project. Cross-machine A/V offset and drift, recovery, and final-use qualification remain pending. [Startup checks](evidence/mapgyver-m17-ndi-startup-2026-11-19.json)
- **NDI runtime guidance.** The Media and Screen Inspectors show what to install if NDI cannot load. Ordinary Shared Texture output kept working in both failure cases. [Checks](evidence/mapgyver-m17-ndi-startup-2026-11-19.json)
- **Concurrent NDI Screens.** Both delivered picture and stereo audio while minimized. The live format change rebuilt the renderer and the peer receiver recovered. [Checks](evidence/mapgyver-m17-ndi-screen-audio-2026-11-19.json)
- **NDI device and project recovery.** Virtual audio remained available with the selected WASAPI device missing. New withdrew both senders and reopening restored them. [Checks](evidence/mapgyver-m17-ndi-screen-audio-2026-11-19.json)

[Browser video previews (M18/6)](evidence/mapgyver-m18-browser-spatial-2026-11-20.json) passed in standalone Chromium across Media Preview, Screen Warping, Surface Source, Composition, the Matrix grid and the Cell editor.

## Pending qualification — in order

- **ShaderToy Library (M18/4).** Search, buffered import, Undo/Redo and reopened output passed a local API fixture. Verify the live API with a valid app key when it is reachable. [Checks](evidence/mapgyver-m18-shadertoy-library-2026-11-20.json)

1. **Extended Video formats — deferred:**
   - **DXV3 Normal:** both GPUs sustained 4K30 and retired all tracked resources. Intel meets visible deadlines and connected image age in one run. Minimized deadlines and full visible frame capture remain open. Minimized GPU p95 was 17.7 ms against the 12 ms target. [DXV checks](evidence/mapgyver-m10-dxv-2026-11-19.json)
   - **Direct 4K upload timing:** the audit traced every evaluated DXV batch to its upload. Intel exceeds the GPU budget. NVIDIA timing remains inconclusive. [Timing evidence](evidence/mapgyver-m10-direct-upload-gpu-2026-11-19.json)
   - **Full-frame draw experiments parked:** projection removal left Intel render time flat. Combining Screen and Spout draws changed pixel values in native checks.
   - H.264 MKV passed frame identity, output cadence and CPU checks. The 4K latency upper bound exceeds 50 ms. [MKV run](evidence/mapgyver-m10-mkv-sustained-2026-11-19.json)
   - Sustained DXV alpha and High Quality, additional profiles, repeat runs, complete GPU timing and device recovery remain open.
3. **Physical editor pointer verification:** The recovery fix passes both GPUs. Finish OS-level physical input and same-frame hit agreement when Computer Use or hands-on input is available. [Checks](mapgyver-m14-editor-recovery.md)
4. **External-display defaults:** allocation code and no-external checks pass. **Manual later:** one/two external monitors, occupied destinations and native New shortcut; the two-external case may need another monitor.
5. **Matrix enabled state and fading Layer Stop:** implemented and functionally checked. Complete interrupted fades, shared-consumer and final-resource retirement combinations.
7. **4× anti-aliasing:** implemented; normal/tiled output checks pass. Finish sustained 1080p/4K cost, memory/overload and recovery checks.
8. **Video fade-loop:** finish moving-seam pixels in preview and output, reverse and multichannel cases, and file/device recovery during the overlap.
9. **Video time cropping:** native output and Reference pixel checks passed. Verify moving Media Preview and the native mixed-duration and replacement cases.
10. **Video subtitles:** verified moving MKV output through cropped hard and smooth loops and an active file replacement. Finish moving Media Preview parity, speed and reverse checks, and the remaining file replacement combinations.
11. **Video performance/stress:** normal playback/seeking is usable. Intel stress efficiency, 4K latency, shared consumers, complete GPU-path and repeated codec/profile runs remain unqualified. Separate-file loops use software decoding and need sustained timing checks at larger sizes.
12. **Shared Texture format coverage:** ordinary supported formats work. Remaining NVIDIA/D3D9, adapter-loss and allocation cases need matching sender/hardware coverage.
13. **32-bit App Control retirement:** the 64-bit path works; the WOW64 accounting/retirement case still fails. Requires a correction before release.
14. **Long-run resources and physical latency:** retained peak-memory growth and GPU/timing limits are not all met. Complete the sustained and mixed-display timing checks; fix remaining failures before release.
15. **Video audio hardware qualification:** capture the actual routed device signal and verify physical audio endpoint disconnect/reconnect while Video is playing. The native mixer, unavailable-device selection and output reopen checks passed.
16. **File drops:** repeat the drop targets with direct File Explorer input, including rejection, Undo and reload. Desktop automation supports single-window drags. [Cases](evidence/mapgyver-m18-matrix-app-drop-2026-11-19.json)
17. **Full regression and Windows qualification:** reconcile the remaining parallel Video and older Matrix/tool-fixture failures, including the exact-record reload comparison after Cell relocation. Then run the complete app/installer checks, including both NDI directions with audio, Windows runtime loading and the six post-NDI workflows.

## Checking cadence from now on

- **Each feedback change:** targeted behavior tests, relevant type/build checks and a short app smoke check; then commit/push.
- **Reuse passing checks and compiled artifacts** when their owning code has not changed. Avoid repeated full-engine/cross-product rebuilds for small edits.
- **Deep/Intel/GPU/edge-case testing is a later batch**, as listed above. It still has to be finished before release.
- **Ordinary-use crashes, data loss and broken required behavior are fixed immediately.**
- I will prepare an exact test and ask when a manual action is actually needed; none is blocking feature work now.
