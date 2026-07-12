# Product Evidence Runner

The real Chataigne binary owns deterministic backend evidence scenarios. An invocation is explicit and exclusive:

```text
cargo run -- --product-evidence phase0.osc-loopback.v1
```

Normal `cargo run`, `cargo run -- --dev`, headless, and host arguments follow the unchanged `golden_core` launch path. Evidence mode does not start the desktop or UI host. It exits nonzero on an unknown scenario, malformed invocation, failed assertion, transport failure, or save/reload mismatch.

The runner writes a versioned JSON result to stdout. The Phase 0 OSC scenario creates the normal `AppEngine`, locates the app's real Module Manager, creates the production Generic OSC module and command through engine/UI intents, uses real UDP input and output, observes command-effect ordering, saves through the sparse project codec, reloads through the app node decoder, and compares stable semantic digests. Ephemeral ports and UUIDs are deliberately excluded from the digest.

The digest is identified as `fnv1a64` and covers canonical JSON evidence. Changing the evidence schema or digest algorithm requires a new scenario/schema version rather than silently changing an existing evidence ID.

## Canonical fixture boundary

No `test-samples/canonical` files are recorded yet. The current repository has no implementation of `P50-L1` or `P5-L127`, and recording topology-only JSON would create false evidence.

- The app can create a Signal module, States, and built-in Action processors separately, but no existing backend authoring contract identifies and binds the shared changing Signal as the canonical input for 50 input-only Actions or defines the threshold-crossing observation used by `P50-L1`.
- Current multiplex characterization constructs `SnapshotProcessorContextProvider` data directly. There is no app-owned project/UI intent that authors one 127-lane processor context with lane-specific comparison values and then validates those lanes after project reload, which `P5-L127` requires.

The fixtures can be added only after those app-owned authoring seams exist. Each committed fixture must load through `from_sparse_project_json::<AppNode>`, expose its required semantic shape through the real engine, execute deterministic threshold crossings, and produce the same semantic digest before and after save/reload. Provider-only tests, hand-authored placeholder JSON, and node-count checks do not qualify.
