# M14/1 editor selection and hover

Updated 2026-11-15. Golden owns reusable input/capture/selection dispatch; app adapters resolve
their edited owners and shared Inspector selection. M14/1 remains **Active, unaccepted**.
Overall acceptance stays88/106 (83%); no new fixed gate is closed by this slice.

The user's2026-11-14 instruction selects M14 before full M10/4 qualification. Every M10
requirement remains unfinished and release-blocking; only execution order changed.

## Delivered behavior

Golden `Viewport2D.onselectionclear` replaces the old background-click callback. It dispatches
unconsumed primary-down and idle Escape, preserving pan capture, tool-owned keys, editable
fields and toolbar controls. Its consumer supplies the owner; Golden contains no MapGyver policy.

Screen Warping selects a Surface on ordinary pointer-down without starting geometry edits.
Keyboard/accessibility clicks remain supported; physical mouse-up cannot repeat the selection.
Background-down and idle Escape call the existing Screen-owner selection path. Alt-drag and
corner editing remain on their existing capture/admission path. Existing selected-only handles
are verified in the DOM fixture, including unavailable-source and invalid geometry.

Composition's backend query now honors explicit edited-owner selection with no selected child,
including stale off-page hints and nested owned Compositions. It no longer selects a first child
implicitly. Locked content and explicit retained hints for unrelated selection remain supported;
their complete independent-editor policy is still unaccepted. The adapter uses authoritative page
selection rather than projected geometry to retain its selected ID, and forwards background/idle
Escape through the shared viewport contract to the Composition media owner.

Golden's public `createViewportPointer` replaces the app-local helper. Composition and Matrix
select a primary hit before mutation admission and retain capture/cancel behavior. Selection
consumes the hit even when editing is refused, preserving viewport keyboard focus when the hit
control is replaced. The browser test reproduces and verifies this focus correction without
manually focusing the viewport before Escape. Physical clicks cannot reselect through the
keyboard/accessibility click fallback.

Matrix Layer background-down and idle Escape select the Matrix media owner; Cell layout selects
its Layer owner without leaving that Layer. Explicit backend owners clear retained Layer/Cell
hints, while owned Cell descendants resolve to their containing Cell. Both queries preserve
bounded visible pages and independent off-page definitions without authoring or history changes.
Unlocked nested Matrix/Composition owner selection now preserves the edited domain instead of
following the nested media's outer Composition placement.

Golden's public `selectedEditorItem` matches the current selected identity against bounded visible
and independently queried definitions. Matrix controls update immediately on a new pointer choice
or deselection; an old cached definition cannot restore handles. Cell selection follows raw
authoritative pages, so parameter projection copies cannot restore the previous selected Cell.

Surface Source uses the existing backend-bound Crop Origin/Size Parameters as its selected
objects, with Surface as edited owner. A body press selects Origin; corner press or keyboard
activation selects Size. Background-down/idle Escape inspect Surface and hide crop handles.
Selection is shared with the ordinary Inspector, including its direct Parameter editor; there
is no extra local selection store. The selectable unselected contour remains available without
a source. Native overlay declarations withdraw corner paint/hit IDs alongside DOM controls.

The crop keeps native capture and gesture admission on press, while sending geometry samples
only after movement. Click release sends no geometry intent. Escape during capture cancels and
consumes the key; a subsequent idle Escape clears selection. Window blur, selection loss and
project/source lifetime replacement stop the owned interaction. The browser fixture covers
refused edit admission, natural focus, typed keyboard selection, capture/cancel and final samples.
Corner selection remains available during metadata refresh: edit admission uses the busy state,
while focus and pointer/keyboard selection remain available. A deterministic busy-state fixture
reproduced the disabled-button failure and now passes for both primary-down and keyboard input.

Golden graph background-down now publishes empty node/edge selection immediately, with no
second callback on release. Idle Escape uses that same public selection contract after connection
cancellation. Text editing, toolbar/finder keys, modifier/marquee selection, middle pan and active
pointer ownership remain isolated. Only selected standard graph nodes show resize handles;
unselected valid nodes get a subtle contour hover without handles or selection changes. The
obsolete mouse-up-only clear flag and its movement bookkeeping are removed. Touched controllers
and the existing browser runner follow the repository formatter.

No additional owner callback or selection store was introduced. Existing Alchemist/State Machine
adapters already select their Formula/manager for empty graph selection. Spatializer's custom
world now selects its module through the same callback, guarded against active endpoint input.
These are shared Golden/consumer changes; MapGyver has no new graph panel from this slice.

Graph pointer cancellation is now separate from commit. Escape first cancels captured input and
any keyboard connection endpoint without deselecting; the next idle Escape selects the owner.
Pointercancel/lost capture/blur and scope/topology/disposal boundaries retire matching ownership
before capture release, discard uncommitted movement/resize/marquee/connection hints, and cannot
submit mutation callbacks. Previously submitted optimistic work remains pending until its own
acknowledgement/rejection. Normal release still commits once. Custom-world capture policy remains
with its specialized adapter.

## Applicable editor inventory

Screen, Source, Composition and Matrix now share Golden's contour action and pure paint
projection. Hover changes only the outline, preserving selection, geometry, handle admission,
disabled dashes and invalid red cues. Native Screen/Source declarations consume the same paint;
the existing native enter/leave path supplies hover. Press/leave/cancel/capture loss/blur and
disposal clear it; explicit Composition/Matrix owner identity replacement clears retained hover.
Actual native input/pixels remain unqualified for this new scope.

Unselected Composition items no longer expose Move gizmos. Known bounds use their actual
contour; unavailable Composition/Matrix bounds retain a selection-only square at the known
pivot. Selection reveals only that item's real controls, with no invented source size. A new
keyboard test reproduced focus loss when a selection target was replaced; the shared viewport
now retains focus before keyboard activation, preserving immediate idle Escape in both editors.

| Editor / owner | Current scope | Exact remaining work |
| --- | --- | --- |
| Golden `Viewport2D` | Browser verified primary-down/idle Escape, child/text/tool isolation, navigation and camera lifetime | Complete independent-editor, capture/cancel and app adapter acceptance |
| Screen Warping / Screen | Pointer-down/owner/idle Escape and selected-only DOM handles; shared contour hover and matching native paint declarations verified | Actual native input/pixel and workbench/Inspector agreement, overlapping hits, off-page and active-gesture Escape checks |
| Surface Source / Surface | Shared typed Crop Parameter selection, owner clearing, selected-only controls; shared hover/native paint declarations; browser capture/cancel and earlier ordinary-host Inspector/history checks | Actual current native pixel/hit agreement, two-editor/offscreen and project/snapshot interaction acceptance |
| Composition / Composition media | Root/nested explicit owner clears old hints; immediate choice/clear and unlocked nested owner retention; selected-only gizmos, actual-bounds hover and unavailable-bounds selection target; pointer/keyboard replacement focus verified | Actual Inspector/native/independent-editor and snapshot acceptance; overlap/off-page and full active-gesture behavior |
| Matrix Layer spatial / Matrix media | Shared owner clear and no-child projection, immediate controls/cached-definition clearing; shared hover, selected-only controls and keyboard/pointer unavailable-bounds selection verified | Actual adapter/Inspector/native checks, independent-editor and active-gesture behavior |
| Matrix Cell spatial / Layer | Shared Layer-owner clear without navigation; backend descendant/off-page selection; shared hover, selected-only controls and unavailable-bounds keyboard/pointer selection verified | Actual adapter/Inspector/native checks, active/outgoing/off-page interaction acceptance |
| Golden graph canvases | Browser verifies primary-down nodes/edges, immediate background/idle-Escape empty selection, selected-only resize/hover, modifier/marquee/pan/text/finder isolation; captured cancellation/focus/identity/lifecycle boundaries and pending-work preservation; Spatializer owner callback wired and consumers typecheck | Actual domain Inspector/native/independent-editor/offscreen/permission and specialized custom-world capture acceptance |
| Dashboard, curve and watcher projection canvases | Specialized Golden canvases must be reviewed | Resolve applicable authored-object/point selection versus read-only projection, then verify specialized tool/focus behavior; no exemption claimed |
| Media Preview / Matrix grid | Preview is not the spatial placement editor; grid has its own selection | Confirm applicable owner/child manipulation paths, including read-only preview and virtualized selection; no qualification claimed from classification |

This inventory retains the complete gate. DOM callback fixtures do not establish actual
Inspector transactions, native pixels, GPU parity or all editor selection policies.

## Validation

Contour slice (2026-11-15):15 Golden viewport unit cases and78 Screen/Composition/Matrix cases
pass. The expanded production browser fixture verifies hover without selection/fill/handles,
known/unavailable bounds, only selected gizmos, invalid cues, native Screen hover declarations,
pointer-down-before-release, keyboard replacement focus and Source cancellation. Golden's full
viewport browser regression also passes with0 page errors. Both app typechecks report0/0;
MapGyver lint, scoped shared formatting, both production UI builds and root/Core formatting pass.
Logs: `target/m14-contour-*-2026-11-15.log`, with final browser receipt
`m14-contour-browser-focus-2026-11-15.log`. Earlier failure after keyboard target replacement is
retained in `m14-contour-browser-final-2026-11-15.log`. No Rust/DTO or native binary changed;
the current UI bundles include this slice, while debug binaries below retain earlier embedded UI.
No new native GPU or whole-gate acceptance is inferred.

- `cargo test --locked -p MapGyver --features native-qualification --lib -- --test-threads=4`:
  689 pass,8 ignored. New Matrix cases verify media/channel/Layer owner clears, owned Cell
  descendants, bounded independent off-page focus, nested edited-domain retention and unchanged
  history/project data. Prior Composition/Screen cases retain their acceptance scope.
- Strict all-target MapGyver Clippy passes with `native-qualification` and `-D warnings`.
- `npm run check:mapgyver`:0 errors,0 warnings, including the typed expanded app fixture.
- Matrix/spatial Node suites:59 pass, including immediate owner clearing while a backend reply
  is held, bounded geometry and reader/session/project/history fencing. Golden viewport/selection
  suites:14 pass, including cached-definition clearing, off-page identity and capture/admission.
- The previous36 Composition/spatial Node cases retain their unchanged source scope.
- The prior40 Screen unit passes retain their unchanged source scope.
- Golden viewport browser runner: passes new down-before-release and idle Escape/text/tool
  checks plus existing zoom anchoring, intermediate wheel frames, pan/camera/reduced-motion tests.
- Expanded app browser runner: passes real `SurfaceHandles`, `CompositionPreview`, Matrix
  `RectLayoutControls`/`SpatialViewport` and Golden
  viewport selection, unavailable-source/invalid geometry, denied-edit selection, selected gizmo
  clearing, owner callbacks, natural idle Escape focus, keyboard and pan. Both Matrix modes
  verify down-before-up selection, refused-edit selection and no stale cached handles after clear.
  These are control/callback fixtures, not full Matrix adapter/Inspector or native GPU credit.
- `npm run lint --workspace mapgyver-ui` and scoped shared Prettier: pass.
- Production UI build and ordinary/native-fixture Cargo build: exit0. The desktop build
  explicitly rebuilt its embedded UI. Root/Core formatting and scoped diff checks complete delivery.

The initial Screen background assertion targeted the zoom toolbar; inspecting the actual hit
identified the fixture error. The corrected point is the gap between Surfaces. No product defect
or native failure is inferred from that coordinate mistake. The first new Rust fixtures omitted
the required document identity and created a Reference instead of an owned nested Composition;
corrected fixtures pass without relaxing document validation. Rust query behavior changes here;
wire DTOs/generated contracts remain unchanged and their existing tests pass.
The first Matrix test compile used a channel-reference field on a `MediaId`; the corrected
fixture uses its typed UUID. No runtime API or acceptance requirement changed to make it pass.

Surface Source's later slice runs86 backend Screen tests (all pass), including typed Crop
Parameter/Surface owner resolution with bounded bindings and unchanged project/history. No
runtime Rust/DTO changed in that slice; the earlier689 full-suite passes retain their scope.
Its41 Screen Node cases pass, including native selected-only handle/hit withdrawal. The expanded
browser fixture passes Source owner clearing, keyboard selection, denied editing, capture/cancel
and final samples in addition to Screen/Composition/Matrix. Source typecheck is0/0 and app lint,
strict all-target Clippy and rebuilt ordinary/native binaries pass. Updated native alignment and
gesture fixture setup explicitly selects crop controls; syntax/format checks do not requalify
their native pixels or timing.

The ordinary-host Source fixture now uses the default Screen and current bounded reference
widgets, waits for fonts and enabled controls, and selects real Crop Parameters before handle/
Inspector checks. Its no-history baseline waits for the preceding Inspector group to close;
the initial assertion included that still-closing group. No history requirement was weakened.
The final2026-11-15 ordinary-host run passes actual down-before-release Inspector/control
selection, background/idle-Escape Surface inspection, no authored/history changes from selection,
all four crop handles, origin/clamping, live Inspector feedback before release, Escape, source
loss/Undo, and Save/New/Load. It reports0 page errors and0 rejected crop requests. The initial
post-Undo keyboard failure is resolved by keeping corner selection available during refresh;
geometry admission remains fenced. This browser/host check establishes no native pixel alignment.

Graph slice validation (2026-11-15):60 Golden graph unit tests pass. The graph component/fixture
Svelte check and both Chataigne/MapGyver app typechecks report0 errors/0 warnings. The production
Edge browser suite passes its existing movement/socket/async/lifetime checks plus five selection
groups: down/hover/controls, idle Escape/focus, modifiers/marquee/pan, connection capture/cancel,
and edges/read-only movement callbacks. The new fixture verifies no duplicate release clear.
Its old extra move-commit argument was removed to match the existing zero-argument API. The
corrected Escape dispatch now reaches idle deselection after checking connection cancellation.

Scoped Prettier/syntax checks and both production UI builds pass. One Chataigne build attempt
lost its generated server index during route analysis; the final isolated build exits0. Its cause
is unproven, and the failed log is retained. No Rust/runtime DTO changed or native GPU check ran.
Graph validation logs are `target/m14-graph-*-2026-11-15.log`; the final Chataigne build uses
`m14-graph-chataigne-build-final-2026-11-15.log`. The graph browser receipt is
`target/qualification/p11-graph-keyboard-2026-11-15T14-25-29-041Z.jsonl`.
Current UI bundles include these changes; the Source debug-binary hashes below retain their
earlier Source scope and do not qualify this graph slice or new native behavior.

Graph cancellation audit: the pre-fix browser baseline confirms one move callback and changed
positions on pointercancel. The corrected production suite verifies movement and resize across
pointercancel/Escape/lost capture/blur, foreign pointer rejection, source scope/topology/disposal
replacement, pending optimistic work, concurrent keyboard/pointer tools, pan/marquee/connection
cancellation and exactly one normal-release commit. It reports0 page errors. The fixture targets
the visible control and observes its real pointer ID; capture loss is processed with the next
pointer event. A constructor initialization-order error was corrected before validation passed.
Final60 graph unit cases, graph component/fixture and both app typechecks (0/0), scoped Prettier,
and both production UI builds pass. Logs: `target/m14-graph-cancel-*-2026-11-15.log`; the intentional
failing baseline is retained separately. Final browser receipt:
`target/qualification/p11-graph-keyboard-2026-11-15T16-18-42-497Z.jsonl`.
No native GPU or actual domain Inspector acceptance is inferred from these callback/DOM tests.

Logs live under `target/m14-selection-*-2026-11-14.log` and
`target/m14-composition-*-2026-11-14.log`.
`target/m14-matrix-*-2026-11-14.log` holds Matrix validation; current Source results are
`target/m14-source-*-2026-11-15.log` with retained Rust/Clippy scope in the2026-11-14 Source logs.
Latest desktop build log SHA256:
`0818509733cba660717301553ade2d170bb6867f75ce2f278a8c63a3791a55f9`.
Built ordinary app SHA256:
`484879f411c2597e2cc9fda24e6776ceb45453dfcef6d60561b3d86c8cc42a1f`;
native fixture SHA256:
`4b1390515b8c9adc6104f37cec48e884f6fb9dc1f58cc4df965072fc0777d04a`.
These replace the debug binaries from the historical M10 build receipt; refresh coherent
qualification inputs on return. Existing Video evidence retains its original source/build scope.

Next deliverable: finish the specialized-editor selection inventory, then verify current actual
Inspector/native/independent-editor behavior with coherent rebuilt binaries. No completed slice
or passing software suite substitutes for whole-gate acceptance.
