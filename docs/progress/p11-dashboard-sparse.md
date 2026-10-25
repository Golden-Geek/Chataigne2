# P11 sparse dashboard viewport

Golden UI owns the page camera, free-layout geometry, clipped membership and UI selection.
Backend nodes, parameters, bindings and edit transactions remain authoritative. The viewport
consumes retained authored bounds through the public Golden Graph UI spatial index; widgets
do not need to mount to participate in direct free-child framing or rectangle selection.
See the [ownership map](../architecture/dashboard-ui.md) and [fixed contract](p11-workspace-qualification-contract.md#dashboard-sparse-editing-extension).

## Implementation

`VersionedNodeMap.forEachChange` skips shared trie branches between published graph versions.
The free-surface model tracks parameter/structural ownership, updates changed owners, preserves
authored paint order and reindexes when surface, viewport or font metrics change. Ordinary
camera queries do not enumerate all authored children. Disabling a free view releases its
retained graph/index references. The public spatial-index dependency is explicit in Golden UI.

Membership intersects clipping ancestors and adds two root rems of overscan. Focused controls,
active gesture shells and a bounded 64-widget native text-selection cohort survive navigation.
Focus is restored after keyed DOM moves; retired authored identity returns focus to the page.
Refreshes are event-driven and coalesced outside Svelte's DOM reconciliation. The same rectangle
selection operation serves marquee gestures and the imperative canvas API; additive selection
uses a set rather than repeated linear membership scans.

Page-owned tab/accordion state is keyed by authored UUIDs and pruned with graph identity changes.
Framing missing descendants temporarily materializes their ancestry in batches of 32 unresolved
selections, captures page-local bounds and releases the ancestry before animation. A newer view
request supersedes pending Frame output. Native pointer targets are retained by identity through
up/cancel/blur/owner retirement; physical gesture qualification remains P17.

Free widget shells now use border-box sizing to match their authored slots. Previously border
thickness could extend the shell beyond those bounds. The shared CSS conversion contract also
requires distinct local `em` and root `rem` measurements. Drag/resize/creation contexts carry
these measurements from the DOM; the preview completion path measures its actual surface.
No persisted data, wire DTO, backend policy or authored value changes.

## Correctness evidence

The [final report](evidence/p11-dashboard-sparse-2026-10-25.json) retains 51 captured source
fingerprints, production asset hashes, browser/machine details, painted/mounted identities,
authored/actual geometry observations and case results. Production Edge 156.0.4314.42 runs in an
owned headless scene on Windows 10.0.26300 / Intel Core Ultra 9 275HX / 24 logical CPUs /
33,752,997,888 bytes RAM / Node 26.5.0. No keyboard/mouse synthesis or desktop control is used.

Reference scenes load the delivered renderer and its canvas modules from `654a5f15` through a
Vite source plugin. The current renderer preserves every painted widget's own text and authored
slot rectangle within 0.5 CSS pixels. Shells fit those rectangles with the same tolerance.
The reference mounts 1,000 root widgets; current run/edit views mount 48/54, including overscan,
with 40/48 painted widgets. A 1,000-child clipped container falls from 1,001 total shells to 25,
including its parent, preserving painted child identities and positions.

All **14 correctness groups and five scale tiers pass**. Groups include run/edit preservation,
nested free clipping, all six layouts in edit/run mounted smoke checks, focused input across
reorder/navigation/font changes, retired-focus fallback, native text selection/release, live
position changes and local `em` font boundary selection. Each scale tier verifies an initially
unmounted distant selection, Frame/Home, logical rectangle selection/toggle and distant reveal.
Framing centers the authored widget within 0.5 CSS pixels. Tabs/accordion sections survive an
offscreen round trip; a nested child can be framed beneath an unmounted container. Framing 100
offscreen containers peaks at 32 mounted shells and centers the selected union within 0.015
CSS pixels. A newer navigation request wins over pending ancestry preparation.

| Target authored nodes | Actual authored nodes | Widgets | Initially mounted |
| ---: | ---: | ---: | ---: |
| 1k baseline | 1,001 | 90 | 18 |
| 10k baseline | 10,010 | 909 | 54 |
| 20k normal | 20,009 | 1,818 | 54 |
| 50k normal | 50,006 | 4,545 | 54 |
| 100k stress | 100,001 | 9,090 | 54 |

Authored counts include widget parameters and structural nodes. Large scenes exercise editing
outside a fixed authored page. Running retains its existing page clip. Single setup-ready
observations remain in the raw report; they include fixture construction, graph publication and
layout settlement, and may overlap unrelated build/tool activity. They establish no latency,
p95, frame-pacing or whole-product throughput gate.

The [retained failed probes](evidence/p11-dashboard-sparse-failures-2026-10-25.json) include:
an incorrect run-mode off-page framing expectation, synchronous focus refresh during rendering,
keyed-move focus loss, retirement before the viewport effect observed the removed DOM, and an
incorrect container-text oracle that included every descendant widget. Repairs preserve page
clipping, defer refresh/focus work, remember authored identity and compare each widget's own text.
The shell-size discrepancy led to the stronger border-box/0.5-pixel framing check. The fixture
now updates reference UUIDs with its generated snapshot identities and clears selection per setup.

Owning unit coverage adds 24 cases: three persistent-map change cases, 15 retained-geometry cases
and six CSS unit conversions. These exercise 50k-key structural sharing, independent versions,
deletion/undefined values, delta ownership, membership/order changes, all nine anchors, unit/minimum
geometry, local/root font separation and cleanup. The full UI gate passes **285 cases**:
audio 27 / graph 52 / Golden UI 91 / app 115. App and browser-fixture checks have zero
errors/warnings. Prettier, whitespace/source sizes and root/Golden Core Cargo formatting pass.
The locked default build rebuilds the frontend and passes in **3m21s**, executable SHA-256:
`77e405333ce7b8af8633f0d68f55abb56993823ec816163d97a38381c0931ea0`.

Reproduce current correctness from the repository root:

```powershell
npm run test:browser:dashboard --workspace golden_ui
npm run check:browser --workspace golden_ui
npm run check --workspace chataigne-ui
npm test
$env:CARGO_INCREMENTAL='0'
cargo build --locked -p Chataigne2
```

## Remaining checkpoint scope

Sparse correctness is a partial dashboard delivery. Dense/zoomed-out views still need a bounded
presentation path; a spatial query alone does not bound all-visible content. The broader
hierarchical/gesture matrix and fixed action/timing qualification remain required. Native
usability retains P17.
**P11 remains 2/6 checkpoints (33% rounded); whole-plan coverage remains 57%.**
