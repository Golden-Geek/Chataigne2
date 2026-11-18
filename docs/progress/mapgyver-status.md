# MapGyver — simple status

- **Updated:** 18 November 2026.
- **Overall plan acceptance:** `████████░░` **82% — 99/121 checks accepted**. Screen-handle links are now accepted. Deferred verification is still counted as unfinished.
- **Latest delivered:** Same-Screen Surface snapping and link/unlink controls; all linked Surfaces update during dragging. Your magnet SVG is colored when on, grey when off; no paging arrows.
- **Working on next:** Custom colors for Screens, Surfaces, Medias and Folders, including reset to the default.
- **Manual action needed from you now: none.** The Screen-link check passed and its test window closed.

## Done — code and normal-use checks passed

- [x] **Parameter controls:** out-of-range values clamp safely; Opacity/Video Speed animation works; bindings/scripts update correctly; Undo and reload work, including missing range/step metadata.
- [x] **Projects:** save/load, normal Undo/Redo and copy/duplicate workflows.
- [x] **Workbench:** media/screen managers, folders, shared selection, generic Inspector, contextual menus and panel targeting/locking.
- [x] **Pictures and Solid Color:** real channel settings/files, independent copies, file reload and color controls.
- [x] **Shared Texture:** receiving, previews, manual/automatic sender controls, sender retention when switching modes, ordinary reconnect and the preview-cadence fix.
- [x] **Screens/Surfaces:** creation, warping, crop/source editing, opacity, enable/disable, calibration and native outputs.
- [x] **Initial document/editors:** startup and New create and immediately show the automatic Screen and Surface. Hidden/reopened panels, saved targets/locks, reload and empty projects pass; no extra Undo steps.
- [x] **Surface Editor display:** related Surfaces across Screens/References, independent toggles/locks, bounded paging, selection/Inspector and Undo/reload. ColorBars/Grid work without a source and obey its crop in Screen Warping/output mapping. Native patterns/contours, cropped ColorBars/Grid, mixed pattern/channel panels and70-Surface toolbar paging pass.
- [x] **Surface-handle links:** same-Screen snapping/link/unlink, supplied colored/grey magnet and live updates of all peers. Five native drag/link/Undo cycles, three-member groups, held-pointer GPU images and foreground image/control alignment pass.
- [x] **Output settings:** GPU preferences, per-Screen frame-rate settings and optional/custom sender names.
- [x] **Display recovery:** the previously tested unplug/replug and monitor-rearrangement cases.
- [x] **Composition:** owned media, transforms, front/back ordering, contextual actions, enable state and zero-scale handling.
- [x] **Matrix:** Layers, Columns, Cells, Groups, sparse scrolling, media drops/copies/references and normal playback/crossfades.
- [x] **Matrix selection/colors:** only explicitly selected objects highlight; modifier multiselection works; Layers/Columns/Cells/Groups show Golden custom colors. Inspector color edits, Undo, copies and reload pass focused checks.
- [x] **Matrix appearance:** quieter borders, small rounded corners and a short active-Cell shadow. Selection/colors remain distinct; reduced motion works; app visual review and focused scaling/narrow-grid checks pass.
- [x] **Column enable/disable:** Inspector toggles mute current/outgoing content; group inheritance, disabled editing, Undo, copy/reload and resume pass focused checks.
- [x] **Matrix presets:** capture/update/rename/delete/recall Layer/Column order and enabled states; contextual menus, commands and named scripts. Paged member Inspector editing, offscreen access, Undo/Redo, copies/reload and live/outgoing playback pass. Cells stay intact.
- [x] **App Control:** normal 64-bit launch/readiness/sharing/reconnect/stop workflows. The remaining 32-bit case is listed below.
- [x] **Video:** common H.264/HEVC/ProRes profiles and HAP; playback, loop, seek/frame stepping, transport Inspector and Matrix activation/deactivation policies.
- [x] **Editor manipulation:** the smoothness improvements you accepted in Screen Warping, Surface Source and Composition.
- [x] **Editor feedback — functional:** shared120ms hover/selection paint, softer handles, keyboard cues and reduced motion. Geometry/input stay immediate; browser and native Screen/Source input checks pass.
- [x] **App presentation:** MapGyver identity/icon, official calibration pattern, font/menu/Inspector improvements and thumbnail Fit behavior.

## Next features — implementation order

1. **NOW — Custom colors:** Screens, Surfaces, Medias and Folders can have custom colors, with reset to the normal default.
2. **Matrix color menus:** Set Color / Reset Color on Layer, Cell and Column context menus, including corresponding Groups.
3. **Compact Layer Groups:** one-line default headers, with Opacity and Blend Mode in the Inspector.
4. **Readable nested Groups:** visible left-side wrapping rails in each Group's color, with clear membership at multiple nesting levels.
5. **Geometry alignment:** temporary snapping/guides while moving or resizing in Surface/Composition/Matrix editors.
6. **Golden Gradient:** reusable Gradient node and Color-parameter animation using that model.
7. **Shared Texture defaults:** auto-select external senders only; mark internal outputs and keep deliberate manual choices.
8. **Extended Video codecs:** DXV and WebP/animated WebP.
9. **Subtitles:** optional, disableable subtitle controls and display.

## Built or partly built — deeper checks/fixes after those features, in this order

1. **Shared editor feedback presentation:** built and functionally checked. Native paint packets are accepted, but matching installation/presented-frame proof is missing. Finish native paint/alignment and actual Composition/Matrix presentation checks.
2. **Shared editor selection/hover:** ordinary behavior exists. Reconnect can split a Source drag into multiple Undo steps; an old Screen request after New can be rejected. Fix and finish interruption checks. Wider Matrix stress/interruption coverage is part of the final regression batch.
3. **External-display defaults:** allocation code and no-external checks pass. **Manual later:** one/two external monitors, occupied destinations and native New shortcut; the two-external case may need another monitor.
4. **Matrix enabled state and fading Layer Stop:** implemented and functionally checked. Complete interrupted fades, shared-consumer and final-resource retirement combinations.
5. **Smooth Frame/Home navigation:** implemented and functionally checked. Finish Matrix-specific, resize/large-document and independent image/control-alignment checks.
6. **4× anti-aliasing:** implemented; normal/tiled output checks pass. Finish sustained 1080p/4K cost, memory/overload and recovery checks.
7. **Video performance/stress:** normal playback/seeking is usable. Intel stress efficiency, 4K latency, shared consumers, complete GPU-path and repeated codec/profile runs remain unqualified; some measurements miss their targets.
8. **Shared Texture format coverage:** ordinary supported formats work. Remaining NVIDIA/D3D9, adapter-loss and allocation cases need matching sender/hardware coverage.
9. **32-bit App Control retirement:** the 64-bit path works; the WOW64 accounting/retirement case still fails. Requires a correction before release.
10. **Long-run resources and physical latency:** retained peak-memory growth and GPU/timing limits are not all met. Complete the sustained and mixed-display timing checks; fix remaining failures before release.
11. **Full regression and Windows qualification:** reconcile/fix the remaining parallel Video and older Matrix/tool-fixture failures, then run the complete app/installer checks.

## Checking cadence from now on

- **Each feedback change:** targeted behavior tests, relevant type/build checks and a short app smoke check; then commit/push.
- **Reuse passing checks and compiled artifacts** when their owning code has not changed. Avoid repeated full-engine/cross-product rebuilds for small edits.
- **Deep/Intel/GPU/edge-case testing is a later batch**, as listed above. It still has to be finished before release.
- **Ordinary-use crashes, data loss and broken required behavior are fixed immediately.**
- I will prepare an exact test and ask when a manual action is actually needed; none is blocking feature work now.
