# Protocol, transport, observation, and runtime UI

Phase 6 adds a generated boundary around the compiled runtime without giving transport code access to semantic state.

`golden-protocol` is the Rust source of truth for control, authoring, observation, catalog, preview, and resynchronization DTOs. `golden-codegen` renders those declarations into `golden-runtime-client`; CI checks the generated file for drift. High-rate numeric samples use the versioned `GVF1` binary frame and enforce sample and finite-value limits before allocation.

`golden-transport` owns per-client outbound queues, network admission policy, observation interests, and transport metrics. Reliable control messages apply bounded backpressure. Preview records coalesce by stable key, binary values retain only the newest frame, and dropped preview keys produce scoped resynchronization markers. Each client owns its queue, so a slow connection never blocks healthy clients. Engine commands cross a bounded channel handle containing no transport state or mutex.

Open-network bindings require TLS, a strong authentication token, explicit non-wildcard origins, bounded payloads, and a concurrent-client permit. Loopback development remains available without weakening non-loopback policy.

`golden-runtime-client` depends on a transport interface rather than WebSocket directly. Incoming reliable events, keyed previews, and the latest binary frame stage until one `requestAnimationFrame` callback and commit coherently into stable maps. A real Chromium gate exercises a 2,000-change preview burst and requires one frame commit with input-to-paint below 100 ms after browser warm-up.

Start with:

- `crates/golden-protocol/src/lib.rs` for DTO and binary-frame truth;
- `crates/golden-transport/src/queue.rs` and `security.rs`;
- `packages/golden-runtime-client/src/frame-stager.ts` and `store.ts`;
- `packages/golden-runtime-client/src/generated/protocol.ts` for generated output only.
