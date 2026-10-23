# Gamepad worker ownership

Gamepad is an app-owned input module under
`apps/chataigne/src/module/modules/controllers/gamepad`. Its existing `gilrs` dependency owns
platform input; Golden IO supplies bounded workers, retained payloads, lifetime admission,
notifications and retry deadlines. No Gamepad policy lives in a Golden package.

`gamepad_runtime/native.rs` owns Gilrs construction, event polling, discovery, selected-state
queries and final destruction. A native turn reads at most eight SDK events, producing at most
sixteen records including catalog updates. Catalogs admit at most 64 devices; identity text
admits 1024 bytes per field. Native endpoints cannot open in test builds. These application
bounds do not establish bounds on Gilrs' private queues or SDK allocations.

## Lifetimes and preparation

Four process owner slots and one per-module slot include setup, operation and cleanup. The actor
never waits for native readiness or joins a worker. Retirement immediately invalidates actor
publications; worker exit invalidates its native source and notifies before provider destruction.
Blocked setup or cleanup retains its slots, preventing replacement thread growth.

The shared source pool admits 128 lifetimes / 32 MiB. A prepared frame charges 64 KiB; each
immutable catalog charges 2 MiB and retains its charge through all shared consumers. The mailbox
admits 32 frames / 8 MiB. Immutable choice domains use a canonical 128-item / 64 MiB pool, and
callbacks use the existing canonical native-callback pool. Drained rows, exported callback Arcs
and queued edits retain original quotas through application, rejection and disposal.

`gamepad_runtime/preparation.rs` validates finite state and bounded backing storage. It prepares
calibration, D-pad synthesis, messages, logs, callback JSON and immutable choices on the worker.
Button callbacks preserve occurrences and existing argument shapes. Selection retains the
existing device ID and label matching behavior; a missing requested ID remains a missing choice.
No Input suppresses selected-device values while preserving device connection callbacks.

Selection and calibration changes advance a publication revision while reusing the native owner.
The worker queries current held state and prepares a new calibrated snapshot. Full source,
callback, choice or mailbox capacity pauses forwarding with cancellable 10 ms waits; a new
configuration can cancel old pending input without first freeing capacity. Healthy worker idle
waits are 8 ms and never execute on the actor.

## Passive projection and recovery

`service.rs` handles worker notifications and drains at most sixteen publications per actor turn.
Healthy idle service has no recurring actor callbacks, wakes or snapshots. Failures use absolute
deadlines with backoff from 250 ms to five seconds. Disabling or destroying the module retires
its source; enabling requests a new bounded owner. Missing snapshots leave input queued.

`projection.rs` captures module identity, every fixed value target, named ancestors and authored
selection/calibration before the turn's owned warning or choice edits. Each row then adds its
actor/native/configuration fence at its projection position. Queued target moves, renames,
configuration changes, disable and source retirement reject stale values and callbacks.

The actor projects prepared values, device metadata, connection status and incoming traffic.
Typed choice-only edits preserve parameter UUID and metadata. The module has no custom output
commands or script methods; base functions, callbacks and its app-owned template remain wired.
Blank persisted selections normalize atomically to Auto. A projection turn tracks that normalized
value so later catalog rows in the same turn can still publish their current choice domain.

The [execution ledger](../progress/implementation-status.md) records qualification evidence.
Controlled providers exercise ownership without accessing physical devices. Device/platform
qualification and full-product scale retain their later plan owners. Gamepad is one of three
families in C11; its delivery alone does not close that package.
