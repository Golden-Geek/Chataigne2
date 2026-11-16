# M14/6 Screen creation defaults

MapGyver owns fresh Screen allocation; Golden supplies physical connection metadata and the
read-only detached creation context. See [ownership and lifecycle](../architecture/mapgyver-screen-defaults.md).
M14/6 remains Active until all original software, actual-app and physical cases are verified.

Implemented behavior:

- Fresh startup/New candidates seed their initial Screen and ephemeral display catalog before
  publication, using one metadata worker. Live creation performs no OS IO.
- Known external connections are eligible even when primary. Internal and unknown/virtual
  connections are excluded. Stable device-path ordering is independent of enumeration order.
- Disabled fullscreen assignments and earlier detached roots reserve their matched displays.
  Ordinary and dependent items in one atomic batch cannot claim the same free display.
- No free external means typed Fullscreen/empty Display, an unconfigured render projection
  and no implicit Shared Texture sender. Intrinsic decoding defaults remain unchanged.
- Existing authored targets, sparse load, history and runtime recovery retain their semantics.

The complete app library suite passes715 tests, with eight existing hardware tests ignored,
sequentially in108.33s. This includes all115 Screen tests, fresh-candidate seeding, generic/
domain/script creation, atomic ordinary/dependent allocation, exact history/reload, rendering,
display matching and generated-contract comparison. Screen code generation, the MapGyver UI
typecheck and scoped browser-fixture Prettier pass. Three shared creation-context tests and
five desktop display tests pass. Strict app/engine/desktop library/test Clippy passes with
warnings denied, and the ordinary default-feature app build exits0. The existing headless host
contract passes six groups: startup, repeated empty reload, exact three-Screen load and three
New operations with fresh identities and zero bootstrap history. HostPID50404 exits0 and
errors are empty. Each fresh target is compared with the current independent host catalog;
this does not test later creation after load or physical output pixels.

The current independent Windows catalog contains one connected internal panel and no known
external display. This host cohort exercises the no-external fallback; one/multiple external
physical cases are not claimed from these observations.

Receipts: `target/m14-display-app-tests-accepted-2026-11-16.log`,
`target/m14-display-codegen-2026-11-16.log` and `target/m14-display-ui-check-2026-11-16.log`.

The first full run passed713 tests and found two
no-native-host fixtures inheriting the new startup inventory while supplying independent
mock catalogs. Their setup now clears inherited metadata; identity matching and pixel/history
assertions remain intact. Earlier Screen fixture assumptions about implicit Spout targets and
primary/secondary classification were made explicit. Failed receipts remain in `target/`.

**Remaining correctness scope:** fresh startup/New seed the catalog, but raw project loading
recreates an empty ephemeral manager catalog. The headless host currently starts Video and
application services without a display-inventory service. Before claiming every creation path,
restore current metadata on loaded candidates and refresh the headless catalog across topology
changes without changing saved target Parameters. Add the loaded-empty-project → CreateScreen
case to the existing host contract. This is unfinished M14/6 work, not deferred qualification.

Shared/default receipts: `target/m14-display-core-creation-tests-2026-11-16.log`,
`target/m14-display-host-tests-2026-11-16.log`, `target/m14-display-clippy-2026-11-16.log`
and `target/m14-display-default-build-2026-11-16.log`, all exit0. The original long-running
Core test handle finished normally and was not restarted. Observer build and existing headless
host checks also exit0; see `target/m14-display-headless-host-2026-11-16.log` and the
[checkpoint receipt](evidence/mapgyver-m14-screen-defaults-2026-11-16.json). This checkpoint
delivers common allocation; loaded/headless integration and full gate acceptance remain open.

Required actual-app/native return: original startup/New/menu/headless creation checks, live
display/Inspector state, physical output and disappearance/recovery on both GPUs. Verify
zero/one/multiple externals and all-occupied behavior against actual available topology;
mock allocation does not establish missing physical cases. Existing M05 recovery evidence
retains only its original scope. Standing NVIDIA Overlay Off applies. No gate or percentage
is awarded by implementation alone, and no timing/release qualification is claimed.
