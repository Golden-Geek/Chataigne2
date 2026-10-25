# P11 ordered-widget operation lifetimes

Free and ordered widgets now share explicit move/resize/update/finish/cancel presentation commands.
Ordered moves and constrained resize have independent owners behind a thin composition facade.
The backend still owns authored move/placement/size semantics through protocol 0.11.0 intents.
This milestone qualifies generic-widget lifetimes and fixes observed geometry/identity errors;
it does not close dashboard viewport awareness or native usability.

## Ownership and corrections

`ordered-move.svelte.ts` captures the source ancestry, destination surface, host and transition
revision. Session admission serializes its atomic move after preceding managed work. An active
cancel sends nothing; confirmation retains the request independently of component disposal.
An older rejection can clear only its own commit preview. Mode, layout, identity, source path and
destination changes retire unconfirmed work. Confirmed requests preserve their authoritative
outcome. Foreign history groups retain the existing helper policy.

`constrained-resize.svelte.ts` uses the same frame/latest-sample queue as free edits and owns one
history group. Cancellation drops unsent samples and waits for the admitted prefix before closure.
Confirmed tails survive disposal. Parameter drafts now explicitly carry presentation enablement;
they never mutate authored metadata. Size and enablement remain projected through late graph
events and withdraw together on rejection. Native pointer IDs remain in adapters. Pointer cancel
and window blur use the same cancellation path; physical input/focus-loss qualification remains P17.

The first probe exposed a **246.324 CSS-pixel size increase for a 24-pixel movement** because
automatic sizing started from the stored preferred size. Resize now measures the rendered starting
size when sizing is automatic and preserves the authored unit. A subsequent vertical probe exposed
**7.171 pixels of extra border extent**; widget shells now consistently use border-box sizing.
The minimum-size rule remains unchanged.

A distant retained tab header also exposed insertion by mounted position: moving source tab 0
before tab 80 was classified as a no-op. `SurfaceItemOrder` now retains the authored sequence at
the page boundary. Point queries inspect mounted anchors and resolve real indices, excluding the
source; that request correctly produces index **79**. Value-only publications do not rescan the
sibling list. Membership/order/role changes refresh the index. Queries refuse to invent a position
when an ordered surface has hidden items but no usable anchor.

## Evidence

The [full browser evidence](evidence/p11-dashboard-ordered-lifetimes-2026-10-25.json) passes
**321 correctness groups and five sparse tiers** through 100k authored nodes, with **124 source
fingerprints**, production asset hashes and no page errors. All prior 215 groups pass. The 106
added groups comprise:

- **36 resize groups** in horizontal/vertical 600-widget containers: rendered-size deltas,
  unsent cancellation, coalescing, delayed begin, mode/layout/removal/reparent/identity retirement,
  confirmed disposal/remounting, late size/enablement events, rejection, separate widget history,
  and leading-edge edits preserving rem/em/percent/vw/vh with changed root/local fonts.
- **69 move groups** using 600-widget containers in five ordered source layouts: atomic movement
  to the free page, cancellation, readonly refusal, source/destination retirement, confirmed mode/
  disposal survival, rejection isolation and queued admission after resize history closure or
  snapshot replacement.
- **One retained-header case** starts with 100 tabs, focuses tab 80, then reduces the page budget
  to eight. The sparse header keeps its authored identity/index through the move. This is a
  separate focus/budget transition, alongside the unchanged 600-widget lifecycle workloads.

The [five failed probes](evidence/p11-dashboard-ordered-failures-2026-10-25.json) retain all source/
asset records and distinguish application failures from fixture corrections. The move fixture
initially advertised no creatable widget types, so real resolution correctly rejected it; it now
advertises the backend's supported types. Another probe queried a never-mounted header in a
600-tab preview; the focused-header transition above exercises the actual retention path.
Timeouts are recorded as failed observations, not evidence of a stopped process or application stall.

The fixture calls actual component operations and the real session intent pipeline, controls
receipts, and publishes typed graph events. It does not requalify backend placement policy,
which retains the preceding real-engine evidence. No keyboard, pointer or drag event is synthesized.
Native adapters' mapping is source wiring, not physical-input qualification.

Root UI tests pass **353 cases**: audio 27 / graph 52 / Golden UI 159 / app 115. Four added unit
cases cover layered presentation enablement and retained authored order, including 10k items,
source exclusions, structural/role changes and bounded graph reads across repeated point queries.
Targeted TypeScript diagnostics for both added/extended unit sources and dependencies are clean.
Browser/app Svelte checks report zero errors/warnings. Source/report Prettier, whitespace and both
Cargo formatters pass. Rust implementation and protocol generation are unchanged from `500775e8`.

The final locked default app build passes in **2m12s**, rebuilding the frontend bundle. The preceding
build before the focus-loss wiring passed in 2m15s. Final executable SHA-256:
`b718e7540cb3fced0e6f5876976f648fe69c4dc4a956ccde3d475c835a7361e4`.
Inventory is **2,159 source files / 27 oversized**. DashboardCanvas is 977 lines and the ordered
facade 46; all touched runtime/test sources remain below 1,000 lines without a new exception.

## Remaining acceptance

Continue active container/node-widget transforms, nested mixed destinations, and geometry where
parent layout/font/viewport changes during work. The source-lifetime matrix above mostly targets
the free page; it does not close arbitrary cross-destination insertion. Qualify composed host/
project/reparent/save/undo barriers over pending confirmed tails. Broader staged/unknown outcomes
remain unqualified. Then execute the fixed three-warmup / 31-sample action matrix, including exact
dense picking, before closing dashboard viewport awareness. This run establishes no action-latency
distribution. Tree keyboard, graph keyboard and composed workflow checkpoints remain open.

Reproduce with `npm run test:browser:dashboard --workspace golden_ui`.
`-- --ordered-only` selects all 106 new diagnostics; `-- --ordered-move-only` selects the 70 moves.
**P11 remains 2/6 checkpoints (33% rounded); whole-plan scope coverage remains 57% (57.41% exact).**
