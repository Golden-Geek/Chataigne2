# P11 dashboard decomposition

Golden UI owns reusable dashboard rendering. Backend parameters and edit transactions remain
authoritative; app renderers enter through the public node-widget registry. The canvas now
composes focused camera, surface, widget, binding, menu, marquee and transform controllers.
See [ownership](../architecture/dashboard-ui.md) for the module map.

The original 6,423-line `DashboardCanvas.svelte` is now **870 lines**. All extracted controllers,
templates and three stylesheet files remain below 1,000 lines. Shared CSS uses an explicit
dashboard namespace across recursive components, with the same rule/declaration order. A
read-only PostCSS comparison confirms all **155 rules / 568 declarations** preserve their
ordered values after whitespace normalization. The initial literal declaration comparison
reported formatting indentation differences; the normalized comparison and browser comparison pass.
No source-size exception was added. The refreshed repository inventory finds 27 oversized sources.

Rendering no longer rewrites position when an anchor changes in edit mode. The prior effect
depended on whether a widget was mounted and sent an implicit domain repair intent from the
renderer. Anchors now use authored offsets directly in both edit and run modes. This changes
anchor-edit interaction to the backend's existing authored meaning; persisted values and schema
are unchanged. A browser regression changes a mounted edit-mode anchor and verifies unchanged
stored offsets and no repair intent. This is required before offscreen widgets may be unmounted.

## Browser evidence

The [retained report](evidence/p11-dashboard-decomposition-2026-10-25.json) contains exact source
and built-asset hashes, environment, case labels and per-case DOM/style/geometry hashes.
Production Vite scenes run in a private headless Edge 156.0.4314.42 browser on Windows
10.0.26300, Intel Core Ultra 9 275HX, 24 logical CPUs, 33,752,997,888 bytes RAM, Node 26.5.0.
Viewport sizes are 1680 x 1200 and 1200 x 900. The fixture fixes the root font at 16 CSS pixels
as a reproducible unit-conversion reference; layout dimensions use relative units.

The driver loads the original canvas directly from `git show c1816a5c:...` through a Vite
source plugin, then builds the current canvas separately. It never checks out or mutates a
baseline tree. Both scenes use the same actual graph/session, widgets, custom renderer registry
and camera command API. No keyboard/mouse event synthesis or desktop control is used.

**106 of 106 comparisons pass with exactly equal hashes**, exceeding the allowed 0.5 CSS-pixel
geometry tolerance. Only generated Svelte scope classes and the new explicit namespace marker
are omitted from the comparison. The case matrix covers:

- Free, horizontal, vertical, grid, accordion and tabs in both edit and run modes.
- Generic text, button, slider, checkbox and single/multiline inputs, external/inside labels,
  nested containers and a registered node-widget renderer.
- Session selection, restored camera, production Frame/Home commands, live percentage sizing,
  backend child reorder and responsive viewport changes.
- All nine authored anchors and the separate edit-mode no-repair regression.
- A sparse 1,000-widget page: both versions still mount all 1,000 shells and 18,677 descendants.

These are preservation checks, **not viewport-culling or timing acceptance**. The ordinary
fixture intentionally preserves the full pre-culling DOM. It does not qualify native gestures,
large-project latency, offscreen selection or the remainder of P11. Native usability retains P17.
The driver asserts its 39 captured source fingerprints remain unchanged throughout the final run.

The exact decomposition runner and source are retained at commit `654a5f15`.
Reproduce from a checkout of that commit; the current dashboard command runs the subsequent
viewport qualification:

```powershell
npm run test:browser:dashboard --workspace golden_ui
npm run check:browser --workspace golden_ui
npm run check --workspace chataigne-ui
npm test
$env:CARGO_INCREMENTAL='0'
cargo build --locked -p Chataigne2
cargo fmt --all
cargo fmt --manifest-path crates/golden_core/Cargo.toml --all
```

The fixed baseline comparison is a decomposition qualification harness. The next culling change
must qualify equivalent visible content and logical offscreen interactions instead of demanding
the same full mounted membership. Browser build output retains the existing large-chunk advisory.

Final gates pass **261 root UI tests** (audio 27 / graph 52 / Golden UI 67 / app 115), app and
browser-fixture type checks with zero errors/warnings, owned Prettier, source-size/whitespace
checks and root/Golden Core Cargo formatting. A TypeScript unused-symbol audit reports zero
findings in the extracted controller modules. The locked default build rebuilds the frontend
and passes in **2m08s**. The final recheck reuses unchanged frontend inputs and passes in
**1m59s**, executable SHA-256:
`7b6fda8bfc315815e35b98b875fd5fa8fac33bebd32385f796da0fc16878b93b`.

P11 remains **2/6 checkpoints (33% rounded)**; whole-plan scope coverage remains **57%**.
Dashboard viewport awareness is in progress, with bounded membership, logical bounds and
interaction/selection/focus qualification still required. No checkpoint is closed by this split.
