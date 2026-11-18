# MapGyver — simple status

- **Updated:** 19 November 2026.
- **Overall plan acceptance:** `████████░░` **84% — 115/137 checks accepted, 8/17 phases complete**.
- **Latest delivered:** OSCQuery clients can subscribe to live values and triggers.
- **Working on next:** Copy OSC Address and show it in the bottom-bar Help.
- **Manual action needed from you now: none.**

## Done — code and normal-use checks passed

- [x] **Parameter controls:** out-of-range values clamp safely; Opacity/Video Speed animation works; bindings/scripts update correctly; Undo and reload work, including missing range/step metadata.
- [x] **Projects:** save/load, normal Undo/Redo and copy/duplicate workflows.
- [x] **Workbench:** media/screen managers, folders, shared selection, generic Inspector, contextual menus and panel targeting/locking.
- [x] **Pictures and Solid Color:** real channel settings/files, independent copies, file reload and color controls.
- [x] **Shared Texture:** receiving, previews and preview cadence, sender retention across Manual/Auto, and ordinary reconnect. New sources pick the first external sender or None. **(internal)** marks Screen outputs. Native pixel checks passed for deliberate internal selection, same-name takeover and external recovery.
- [x] **Screens/Surfaces:** creation, warping, crop/source editing, opacity, enable/disable, calibration and native outputs.
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
- [x] **Previous/Next wait modes:** launch immediately or wait for the first or last active finite Video in scope. Loops are skipped. Repeated commands replace the wait, and Stop cancels it. Inspector, toolbar and playback checks passed. [Checks](evidence/mapgyver-m16-navigation-waits-2026-11-18.json)
- [x] **Matrix transition indicators:** green Cell and Column lines show the current mix through crossfades, interruptions, Cuts and scrolling. Verified in the Matrix editor and with a live Spout output. [Checks](evidence/mapgyver-m16-transition-lines-2026-11-18.json)
- [x] **Empty Cell hover:** the outline stays visible over the `+` button and clears when the pointer leaves. Mouse and keyboard Add Media access both passed. [Checks](evidence/mapgyver-m16-empty-cell-hover-2026-11-18.json)
- [x] **Compact Matrix controls:** right-edge Layer Opacity sliders and compact Blend Mode selectors use the same Parameters as the Inspector. Keyboard access, Undo/Redo, a narrow dock and scrolling passed. [Checks](evidence/mapgyver-m16-compact-controls-2026-11-18.json)
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
- [x] **Video subtitles:** select the Video Media, then open **Inspector → Channels → [channel name] → Subtitles**. Each channel has its own enable toggle, embedded track or subtitle file, and style. In the latest NVIDIA smoke check, two channels in one Media displayed different captions. Editing the second or disabling the first left the other channel’s caption intact. Reference pixels, held seek/crop timing, missing-file recovery and saved settings passed. [Checks](evidence/mapgyver-m10-channel-subtitles-2026-11-18.json)
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

## Next features — implementation order

1. **NOW — OSC addresses.** Add Copy OSC Address to node menus and the address display in Help.
2. **Seamless previews.** Remove black flashes in Surface Editor and Media Preview, and reuse the source when two Surfaces share media.
3. **Linked outlines in Screen Warping.** Fix outline flicker while dragging linked handles.
4. **Extended Video formats.** Add DXV, WebP (including animated WebP), and MKV playback
    with selectable embedded subtitles and audio tracks.
5. **Golden Gradient:** reusable Gradient node and Color-parameter animation using that model.

## Built or partly built — deeper checks/fixes after those features, in this order

1. **Shared editor feedback presentation:** built and functionally checked. Native paint packets are accepted, but matching installation/presented-frame proof is missing. Finish native paint/alignment and actual Composition/Matrix presentation checks.
2. **Shared editor selection/hover:** ordinary behavior exists. Reconnect can split a Source drag into multiple Undo steps; an old Screen request after New can be rejected. Fix and finish interruption checks. Wider Matrix stress/interruption coverage is part of the final regression batch.
3. **External-display defaults:** allocation code and no-external checks pass. **Manual later:** one/two external monitors, occupied destinations and native New shortcut; the two-external case may need another monitor.
4. **Matrix enabled state and fading Layer Stop:** implemented and functionally checked. Complete interrupted fades, shared-consumer and final-resource retirement combinations.
5. **Smooth Frame/Home navigation:** implemented and functionally checked. Finish Matrix-specific, resize/large-document and independent image/control-alignment checks.
6. **4× anti-aliasing:** implemented; normal/tiled output checks pass. Finish sustained 1080p/4K cost, memory/overload and recovery checks.
7. **Video fade-loop:** finish moving-seam pixels in preview and output, reverse and multichannel cases, and file/device recovery during the overlap.
8. **Video time cropping:** native output and Reference pixel checks passed. Verify moving Media Preview and the native mixed-duration and replacement cases.
9. **Video subtitles:** verify moving Media Preview and output at multiple speeds, including reverse and cropped hard/smooth loops. Cover channel file replacement during playback.
10. **Video performance/stress:** normal playback/seeking is usable. Intel stress efficiency, 4K latency, shared consumers, complete GPU-path and repeated codec/profile runs remain unqualified. Separate-file loops use software decoding and need sustained timing checks at larger sizes.
11. **Shared Texture format coverage:** ordinary supported formats work. Remaining NVIDIA/D3D9, adapter-loss and allocation cases need matching sender/hardware coverage.
12. **32-bit App Control retirement:** the 64-bit path works; the WOW64 accounting/retirement case still fails. Requires a correction before release.
13. **Long-run resources and physical latency:** retained peak-memory growth and GPU/timing limits are not all met. Complete the sustained and mixed-display timing checks; fix remaining failures before release.
14. **Video audio hardware qualification:** capture the actual routed device signal and verify physical audio endpoint disconnect/reconnect while Video is playing. The native mixer, unavailable-device selection and output reopen checks passed.
15. **Full regression and Windows qualification:** reconcile the remaining parallel Video and older Matrix/tool-fixture failures, including the exact-record reload comparison after Cell relocation. Then run the complete app/installer checks.

## Checking cadence from now on

- **Each feedback change:** targeted behavior tests, relevant type/build checks and a short app smoke check; then commit/push.
- **Reuse passing checks and compiled artifacts** when their owning code has not changed. Avoid repeated full-engine/cross-product rebuilds for small edits.
- **Deep/Intel/GPU/edge-case testing is a later batch**, as listed above. It still has to be finished before release.
- **Ordinary-use crashes, data loss and broken required behavior are fixed immediately.**
- I will prepare an exact test and ask when a manual action is actually needed; none is blocking feature work now.
