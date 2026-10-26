# P11 project-generation cursors

Golden owns project-lifetime transport fencing. App graph adapters consume that public boundary;
Alchemist supplies the native replacement fixture. This follows `39f3536c` and completes the
unsubscribe/snapshot/re-subscribe overlap requirement identified in
[Alchemist qualification](p11-alchemist-graph-authoring.md). **P11 remains 4/6 checkpoints (67%
rounded), whole-plan coverage 59%; P03 has zero remaining closure packages.**

## Contract

Protocol 0.12 snapshots pair their event boundary with the captured project generation. Subscribe
and replay use the generated `UiReplayCursor`; delta envelopes and replay responses identify
their generation. Event time alone cannot identify a project after replacement. Rust is the single
wire-contract owner; all consumers and seven changed/generated bindings are updated together.
This is an intentional ephemeral protocol break, without a persisted-project format change.

Cached snapshots and deferred materialization retain the same generation as their projection
roots. Stale subscribe cursors request resync immediately, including when no subscription was
active at cutover. Replay reads generation before and after retained-event filtering; a replacement
discards that result and returns only a resync marker. No publication lock spans that filtering.

Applied transaction results retain the generation captured inside their actor turn. Origin
filtering cannot suppress another project's equal-time event, and latest-wins queue keys include
generation. The client discards old-generation frames, requests resync for future frames, and
stages only its snapshot's lifetime. Snapshot/replay read completion rejects malformed DTOs
promptly; a failed decoder cannot remove the timer and leave the promise unresolved.

## Qualification

Six new host regressions cover stale subscription after cutover, stale replay, a snapshot encoder
held across replacement, equal-time origin filtering, transaction result lifetime, and queue
isolation. The existing active-subscription equal-clock test remains. Four new UI cases cover
captured cursor ownership, future/retired frames and reset clocks, malformed snapshot rejection,
and strict generation decoding. Shared socket fixtures use the supported public testing export.

The native test mounts the actual Alchemist panel against the default executable with authenticated
HTTP/WebSocket. It holds one real snapshot response, replaces the project, observes unsubscribe,
then releases the old response. It requires an actual stale-generation subscribe on the wire,
server resync of both active and stale-subscribe lifetimes, and recovery to the authoritative
read-only replacement. Existing move/history, collapse/connection, save/reload/cancellation and
atomic rejection groups remain. No success-path diagnostic backend snapshot is added.

The first run recovered correctly but failed a harness assumption requiring equal event clocks:
the replacement reset from tick 2 to tick 1/sequence 5. That failure is preserved. Native project
initialization does not guarantee clock equality, so the final gate requires generation fencing
and reports clock equality rather than treating it as a prerequisite. Deterministic backend tests
remain the equal-clock gate; the next native run also happened to have equal clocks and passed.
This changes the fixture precondition, not the required resubscription/recovery assertions.

Checks pass: **634 engine cases / three manual cases ignored**, **81 transport cases / one
manual benchmark ignored**, three protocol-only cases, the public headless-host case and 24
Formula operation cases / two separately invoked native cases. All **425 UI cases** pass
(27 audio / 58 graph / 193 Golden / 147 app). App and graph-fixture Svelte checks have zero
errors/warnings. Strict app/all-target Clippy passes, all seven generated bindings match the Rust
exporter, package-owned formatting and both Cargo formatters pass. Inventory is **2,235 sources /
26 oversized**; no touched source exceeds 1,000 lines.

The [preserved harness failure](evidence/p11-project-cursors-native-failure-2026-10-26.jsonl) and
[first native pass](evidence/p11-project-cursors-native-first-pass-2026-10-26.jsonl) retain the
original wire observations. Their executable hash is recorded in each report.

The final locked default build passes in **4m23s**, with a rebuilt frontend. Binary SHA-256:
`7f3295794714cd31b9ba61a4d7c9ad578353c5091739bedfd4babfee359c635a`.
That exact executable passes the [five-group Alchemist workflow](evidence/p11-project-cursors-native-2026-10-26.jsonl),
the [four-group State regression](evidence/p11-project-cursors-state-regression-2026-10-26.jsonl),
and both native Formula HTTP/WebSocket/script and exact history cases. The
[source/build/check record](evidence/p11-project-cursors-source-2026-10-26.json) identifies the
inputs, fixture assets, toolchain and checks. The final Alchemist run resets to a lower event clock;
the earlier equal-clock pass is retained separately.

## Reproduction and remaining scope

```powershell
$env:CARGO_INCREMENTAL='0'
cargo test --locked -p golden_engine --lib
cargo test --locked -p golden_protocol
cargo test --locked -p golden_transport_server --lib
cargo test --locked -p golden_transport_server --test public_headless_host
npm test
npm run check --workspace chataigne-ui
npm run check:browser:graphs --workspace chataigne-ui
cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings
cargo build --locked
# Use the already-built app test harness after the final default build.
& target/debug/deps/Chataigne2-<test-hash>.exe formula_graph_native_browser_adapter --ignored --nocapture
```

Spatializer's actual empty-document/custom-world integration, actual-app scale and composed
workflows remain under the two open P11 checkpoints. Small native fixtures do not qualify those
workloads. Native physical usability remains P17; full-product throughput remains P16.
