# Composition anchored editor acceptance

M06/4 is **Accepted** on 2026-11-03. The current owned-media editor has eight resize
handles and four invisible rotation targets outside the actual corners. The pivot has only
the move handle. Ordinary settings remain typed Placement Parameters in Golden's Inspector.

## Ownership and contract

MapGyver's backend owns resize semantics. `InstanceGeometryMode::Resize` names a typed
handle and the source extent observed at gesture start. The backend resolves that extent
without I/O and rejects unavailable or changed dimensions before any edits. It also rejects
a changed pivot/rotation basis. There is no UI size default or pivot-based Scale operation.
The Rust declaration generates TypeScript and all active geometry callers use it.

Absolute pointer deltas are rotated into the initial local axes. A corner changes both
signed scales and shifts Position to retain the opposite corner. An edge changes only its
local axis and retains the opposite edge. This supports mirrored/nonuniform transforms,
arbitrary pivots and off-center clicks without a jump. The existing latest-sample gesture
controller sends immutable origins and groups admitted Parameter edits into one history entry.

The UI projects real source bounds and directional resize cursors. Rotation targets keep
their screen-space reach in rem as the viewport zooms; their SVG cursor uses an intrinsic
24-pixel image and 12-pixel hotspot, an explicit native-cursor exception to relative layout.

## Verification

- **347 library tests passed, 2 explicit qualification tests ignored.** This includes generated
  contract equality, four-route caller parity and 24 anchored resize scenarios (eight handles
  at 0°, 37° and 90°, mirrored/nonuniform scale, pivot outside the source, off-center clicks,
  13 absolute samples, unchanged edge axis and exact single Undo/Redo).
- Degenerate/nonfinite, wrong-owner and stale-dimension edits reject atomically.
- All **10 focused Composition UI tests** pass. MapGyver Svelte check has **0 errors/warnings**.
  Strict all-test native-qualification Clippy, both Cargo formatters and touched-file formatting
  pass. Existing native raster evidence remains in [M06/3](mapgyver-m06-native.md).
- Rebuilt ordinary app passes headless Composition review: **30 samples, 3 grouped gestures**,
  structural/source/project/history checks. Bundled owned-media UI passes actual Reference and
  Alt-copy drops, independent variants, typed Position/degree Rotation, missing recovery and
  Save/New/Load. The isolated host exits normally with code 0.
- `native-navigation.mjs --composition` passes in the **actual bundled WebView2 app**:
  all eight real pointer resize drags retain the opposite anchor; all four outside-corner
  rotation zones rotate 45° and expose the rotation cursor; each repeated gesture has exact
  single Undo/Redo. Smooth anchored zoom has **45 sampled frames**, multiple intermediate
  zoom values and ≤1 CSS-pixel anchor error. Translation at zoom, shared Inspector selection,
  Escape discarding later held samples, unchanged peer and tab lifetime pass, with **0 page
  errors and 0 rejected requests**. The owned native app exits normally with code 0.

Two baseline native attempts passed all eight resize cases but stopped at the cursor check.
The second established the real fault: Vite's inlined SVG contains single quotes, invalidating
the original single-quoted CSS URL. Static bundle inspection identified the boundary; proper
URL quoting fixed it. The subsequent corrective native acceptance run passed the full suite.
The headless fixture's expected X position was also corrected from 85 to the independently
derived 105; no geometry tolerances were relaxed. No additional exploratory probes were added.

Executable SHA-256: `0DDB2901C2C3FCF7CDEA6C9713D729D5CA3E76907BEBB8D74685D09B9F3B552F`.
Receipts: `target/m06-geometry-{tests,clippy,ui-check,build,bundled,native}.log`.
The inspected `target/m06-composition-handles.png` records overlay/Inspector appearance;
Playwright screenshots omit the native underlay and are not pixel evidence.

## Remaining scope

M06/5 retains bounded independent follow/lock panels, saved panel state, project/tab/reconnect
lifetime and removal of internal heading/create/refresh controls. The obsolete form-based
`tests/browser/composition.mjs` still needs reconciliation with those final controls; it is
not cited as passing. Its former geometry assertions are covered by the actual native suite
above, while current typed authoring/structural behavior retains M06/1 and bundled owned-media
proof. No original physical, performance, scale or release acceptance is waived.
