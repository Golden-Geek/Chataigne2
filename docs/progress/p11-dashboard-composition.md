# P11 dashboard composition and coordinate-frame ownership

Golden UI now qualifies container and registered node-widget transforms across all six dashboard
layouts, plus mixed destination insertion and pending-operation lifetimes. The backend continues
to own move, placement and sizing policy through protocol 0.11.0. This milestone does not close
the dashboard checkpoint: composed barriers and the fixed action/timing matrix remain required.

## Runtime corrections

Active free transforms and constrained resize capture their parent surface in `layout-frame.ts`.
Input, confirmation and unsent-write admission validate rendered bounds, local/root fonts, scroll,
surface dimensions and viewport dimensions. An invalid frame cancels unsent samples and waits for
the admitted prefix before closing history. Checks are driven by input/admission and window resize,
with no polling or measurement in every draft read. Confirmed work releases its DOM frame and
retains its captured authored sample independently of view disposal.

The first held-receipt probe changed the root font and still admitted a newer sample computed
in the old frame. That stale tail is now retired. It also exposed right/bottom resize snapping
that moved the untouched authored origin from `(2, 2)` to `(0, 0)`. Snapping now affects only the
edited edges, preserving opposite edges, unedited axes and minimum-size constraints.

A free-to-free-container move exposed reactive nested placement objects crossing the intent
boundary. Fixture serialization failed before recording the move. `dashboard-actions.ts` now
copies scalar dimensions and enablement into owned DTOs. The browser move and a unit case that
changes the original draft after submission verify independent payload ownership.

## Fixed matrix and evidence

The [contract](p11-dashboard-composition-contract.md) fixes **164 added groups**:

- 24 container/node-widget parent-layout cases: move under all six layouts; resize under free,
  horizontal and vertical; refuse resize under grid, tabs and accordion.
- 18 mixed destinations: generic, registered-node and container sources enter all six layouts.
  Ordered destinations receive authored index one before the middle of three real anchors.
  Inactive tabs are checked by header identity and activated through the public view operation.
- 78 active frame changes: three source families, free move/resize and horizontal/vertical resize,
  six frame changes each, plus actual scrolling for the two constrained contexts.
- 24 free/constrained cases: confirmed tails survive canvas disposal/remounting; rejected samples
  withdraw their drafts and restore the original geometry within 0.5 CSS pixels.
- 20 ordered container/node moves: confirmed or rejected requests survive disposal while preserving
  the authoritative destination or original parent, respectively.

All scenes retain **600 logical source widgets and three destination anchors**. Containers keep
their subtrees; node widgets use the public renderer registry. Layout fields use a declared folder.
Cloned fixture nodes have distinct UUIDs, with an explicit uniqueness invariant. Scroll probes
constrain the existing viewport before starting; they do not remove logical widgets. An explicit
canvas unmount exercises disposal where a small ordered collection correctly stays mounted during
panning. Existing hierarchy/detail budgets and the 0.5 CSS-pixel geometry tolerance stay unchanged.

The [final full browser evidence](evidence/p11-dashboard-composition-2026-10-25.json) passes
**485 correctness groups and five sparse tiers** through 100k authored nodes, including all prior
321 groups. It records 129 current source fingerprints, production asset hashes and no page
errors. The preceding full run also passed; final qualification includes distinct cloned UUIDs.
The [failed observations](evidence/p11-dashboard-composition-failures-2026-10-25.json) retain eight
application/fixture failures, source and asset fingerprints, and the explicitly interrupted held
receipt observation. Unchanged sparse/dense/hierarchy and earlier lifecycle suites remain required.

The owned headless browser calls actual component presentation commands and the real session
pipeline, with controlled receipts and typed graph publications. This qualifies UI consumers;
backend policy retains the preceding real-engine evidence. No keyboard or pointer event is
synthesized. Fixture dimension changes and explicit unmounts are presentation probes, not native
gesture qualification. Physical input remains P17.

Root UI tests pass **359 cases**: audio 27 / graph 52 / Golden UI 165 / app 115. Six added unit
cases cover four resize edges, minimum-size precedence and placement payload ownership. Targeted
TypeScript diagnostics for both touched unit sources and browser dependencies pass; browser/app
Svelte checks report zero errors and warnings. The locked default app build rebuilt the UI and
passed in **2m16s**. Rust implementation and generated protocol are unchanged. Source inventory is
**2,164 files / 27 oversized**; all touched runtime and test sources stay below 1,000 lines.
Executable SHA-256:
`5d039ad981bd6edb1a2110300ae58d69df318ee5ce98737014ddea781b8b1324`.

## Remaining acceptance

Qualify host/project/reparent/save/undo barriers over confirmed pending tails before the fixed
three-warmup / 31-sample action matrix, including exact dense picking. Raw request-queue drainage
alone does not establish that managed operations have submitted their final samples. Staged and
unknown outcomes remain unqualified. These correctness checks establish no action-latency
distribution or composed-product throughput claim.

P11 remains **2/6 fixed checkpoints complete (33% rounded)** and whole-plan scope coverage stays
**57% (57.41% exact)**. Dashboard, tree keyboard, graph keyboard and composed workflow checkpoints
remain open. P03 remains complete with zero closure packages left. The full P00–P18 goal stays active.
