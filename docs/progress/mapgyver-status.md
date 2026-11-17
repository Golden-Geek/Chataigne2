# MapGyver — simple status

- **Updated:** 17 November 2026.
- **Overall plan acceptance:** `████████░░` **79% — 93/117 checks accepted**. Deferred verification is still counted as unfinished.
- **Latest finished:** parameter-control safety, including animation and save/reload.
- **Working on next:** Matrix selection and custom colors.
- **Manual action needed from you now: none.**

## Done — code and normal-use checks passed

- [x] **Parameter controls:** out-of-range values clamp safely; Opacity/Video Speed animation works; bindings/scripts update correctly; Undo and reload work, including missing range/step metadata.
- [x] **Projects:** save/load, normal Undo/Redo and copy/duplicate workflows.
- [x] **Workbench:** media/screen managers, folders, shared selection, generic Inspector, contextual menus and panel targeting/locking.
- [x] **Pictures and Solid Color:** real channel settings/files, independent copies, file reload and color controls.
- [x] **Shared Texture:** receiving, previews, manual/automatic sender controls, sender retention when switching modes, ordinary reconnect and the preview-cadence fix.
- [x] **Screens/Surfaces:** creation, warping, crop/source editing, opacity, enable/disable, calibration and native outputs.
- [x] **Initial document:** an initial Screen and Surface are created automatically. Automatically showing them in the editors is still in the next-feature list.
- [x] **Output settings:** GPU preferences, per-Screen frame-rate settings and optional/custom sender names.
- [x] **Display recovery:** the previously tested unplug/replug and monitor-rearrangement cases.
- [x] **Composition:** owned media, transforms, front/back ordering, contextual actions, enable state and zero-scale handling.
- [x] **Matrix:** Layers, Columns, Cells, Groups, sparse scrolling, media drops/copies/references and normal playback/crossfades.
- [x] **App Control:** normal 64-bit launch/readiness/sharing/reconnect/stop workflows. The remaining 32-bit case is listed below.
- [x] **Video:** common H.264/HEVC/ProRes profiles and HAP; playback, loop, seek/frame stepping, transport Inspector and Matrix activation/deactivation policies.
- [x] **Editor manipulation:** the smoothness improvements you accepted in Screen Warping, Surface Source and Composition.
- [x] **App presentation:** MapGyver identity/icon, official calibration pattern, font/menu/Inspector improvements and thumbnail Fit behavior.

## Next features — implementation order

1. **NOW — Matrix selection and custom colors:** highlight only explicitly selected Layers/Columns/Cells/Groups; show Golden custom colors.
2. **Matrix appearance:** subtle borders/rounded corners and clearer active-Cell feedback.
3. **Matrix configuration presets:** save/recall Layer/Column order and enabled states without changing Cell contents.
4. **Shared editor feedback:** short hover/selection transitions and subtler handles/outlines.
5. **Startup/New editor targets:** immediately show the automatic Screen and Surface without manual selection.
6. **Surface Editor:** show related Surfaces sharing a source; show test patterns even without a source.
7. **Screen-handle links:** magnet snapping creates persistent linked handle groups; provide unlink controls.
8. **Geometry alignment:** temporary snapping/guides while moving or resizing in Surface/Composition/Matrix editors.
9. **Golden Gradient:** reusable Gradient node and Color-parameter animation using that model.
10. **Shared Texture defaults:** auto-select external senders only; mark internal outputs and keep deliberate manual choices.
11. **Extended Video codecs:** DXV and WebP/animated WebP.
12. **Subtitles:** optional, disableable subtitle controls and display.

## Built or partly built — deeper checks/fixes after those features, in this order

1. **Shared editor selection/hover:** ordinary behavior exists. Reconnect can split a Source drag into multiple Undo steps; an old Screen request after New can be rejected. Fix and finish interruption checks.
2. **External-display defaults:** allocation code and no-external checks pass. **Manual later:** one/two external monitors, occupied destinations and native New shortcut; the two-external case may need another monitor.
3. **Matrix enabled state and fading Layer Stop:** implemented and functionally checked. Complete interrupted fades, shared-consumer and final-resource retirement combinations.
4. **Smooth Frame/Home navigation:** implemented and functionally checked. Finish Matrix-specific, resize/large-document and independent image/control-alignment checks.
5. **4× anti-aliasing:** implemented; normal/tiled output checks pass. Finish sustained 1080p/4K cost, memory/overload and recovery checks.
6. **Video performance/stress:** normal playback/seeking is usable. Intel stress efficiency, 4K latency, shared consumers, complete GPU-path and repeated codec/profile runs remain unqualified; some measurements miss their targets.
7. **Shared Texture format coverage:** ordinary supported formats work. Remaining NVIDIA/D3D9, adapter-loss and allocation cases need matching sender/hardware coverage.
8. **32-bit App Control retirement:** the 64-bit path works; the WOW64 accounting/retirement case still fails. Requires a correction before release.
9. **Long-run resources and physical latency:** retained peak-memory growth and GPU/timing limits are not all met. Complete the sustained and mixed-display timing checks; fix remaining failures before release.
10. **Full regression and Windows qualification:** reconcile/fix the remaining parallel Video and older Matrix/tool-fixture failures, then run the complete app/installer checks.

## Checking cadence from now on

- **Each feedback change:** targeted behavior tests, relevant type/build checks and a short app smoke check; then commit/push.
- **Reuse passing checks and compiled artifacts** when their owning code has not changed. Avoid repeated full-engine/cross-product rebuilds for small edits.
- **Deep/Intel/GPU/edge-case testing is a later batch**, as listed above. It still has to be finished before release.
- **Ordinary-use crashes, data loss and broken required behavior are fixed immediately.**
- I will prepare an exact test and ask when a manual action is actually needed; none is blocking feature work now.
