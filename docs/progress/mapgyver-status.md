# MapGyver — simple status

- **Updated:** 18 November 2026.
- **Overall plan acceptance:** `████████░░` **82% — 113/137 checks accepted, 8/17 phases complete**. Compact Matrix controls passed.
- **Latest delivered:** Each Matrix Layer has a vertical Opacity slider on the right and a compact Blend Mode selector in its header.
- **Working on next:** Audio output and routing.
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
- [x] **Video subtitles:** expand each Video channel under **Inspector → Channels → Subtitles** to choose its own embedded track or subtitle file and adjust its style. Two different captions in one Media, independent enable toggles, Reference pixels, held seek/crop timing, missing-file recovery and saved settings passed. [Checks](evidence/mapgyver-m10-channel-subtitles-2026-11-18.json)
- [x] **Editor manipulation:** the smoothness improvements you accepted in Screen Warping, Surface Source and Composition.
- [x] **Editor feedback — functional:** shared120ms hover/selection paint, softer handles, keyboard cues and reduced motion. Geometry/input stay immediate; browser and native Screen/Source input checks pass.
- [x] **App presentation:** MapGyver identity/icon, official calibration pattern, font/menu/Inspector improvements and thumbnail Fit behavior.

## Next features — implementation order

1. **NOW — Audio output and routing.** Use Golden Audio for device output and playback across Video and Matrix. Map virtual channels to physical outputs in Preferences and set per-channel volume on Video media.
2. **Video intro and loop.** Loop the end of one file or play a separate loop file, with optional separate loop audio.
3. **Matrix Cell audio.** Use the media's audio, give the Cell its own audio, or play audio only.
4. **OSCQuery for all Golden apps.** Implement discovery, undoable node edits and live feedback in Golden Core, with configurable ports per app.
5. **Seamless previews.** Remove black flashes in Surface Editor and Media Preview, and reuse the source when two Surfaces share media.
6. **Linked outlines in Screen Warping.** Fix outline flicker while dragging linked handles.
7. **Extended Video formats.** Add DXV, WebP (including animated WebP), and MKV playback
    with selectable embedded subtitles and audio tracks.
8. **Golden Gradient:** reusable Gradient node and Color-parameter animation using that model.

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
10. **Video performance/stress:** normal playback/seeking is usable. Intel stress efficiency, 4K latency, shared consumers, complete GPU-path and repeated codec/profile runs remain unqualified; some measurements miss their targets.
11. **Shared Texture format coverage:** ordinary supported formats work. Remaining NVIDIA/D3D9, adapter-loss and allocation cases need matching sender/hardware coverage.
12. **32-bit App Control retirement:** the 64-bit path works; the WOW64 accounting/retirement case still fails. Requires a correction before release.
13. **Long-run resources and physical latency:** retained peak-memory growth and GPU/timing limits are not all met. Complete the sustained and mixed-display timing checks; fix remaining failures before release.
14. **Full regression and Windows qualification:** reconcile/fix the remaining parallel Video and older Matrix/tool-fixture failures, then run the complete app/installer checks.

## Checking cadence from now on

- **Each feedback change:** targeted behavior tests, relevant type/build checks and a short app smoke check; then commit/push.
- **Reuse passing checks and compiled artifacts** when their owning code has not changed. Avoid repeated full-engine/cross-product rebuilds for small edits.
- **Deep/Intel/GPU/edge-case testing is a later batch**, as listed above. It still has to be finished before release.
- **Ordinary-use crashes, data loss and broken required behavior are fixed immediately.**
- I will prepare an exact test and ask when a manual action is actually needed; none is blocking feature work now.
