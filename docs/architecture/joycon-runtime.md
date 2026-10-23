# Joy-Con worker ownership

The app-owned Joy-Con module projects authored values and callbacks. Its native runtime owns
`joycon-rs`, discovery, calibration, report reads, rumble/lights, device recovery and cleanup.
The module uses public `golden_io` admission, payload, revision and worker primitives.

## Runtime and service

Four global lifetime slots include opening, active and retiring workers. A module also reserves
one connection lease through native destruction, so old cleanup cannot stop a replacement's
rumble. Cancellation never needs command-mailbox admission, a join or a retirement thread.
Termination and queued command disposition precede potentially blocked provider cleanup.
Native endpoint creation is disabled in test builds; qualification uses controlled providers.

The actor registers its worker wake when the node is ready and stays passive. Worker receipts,
input and termination notify it; processing caps,
activity, a connected-runtime watchdog and startup recovery use explicit deadlines.
Startup retries back off from 250 ms to five seconds, independently of unrelated wakes.
Discovery/attach retry, stale reports, stick calibration and IMU filtering remain native-owned.

## Admission and delivery

The command mailbox admits 64 records / 256 KiB. Each command charges 4 KiB; a module admits
64 lifetimes including completed receipts and queued projection. Global output storage admits
512 lifetimes / 2 MiB. Text fields admit 256 bytes and rumble numbers must be finite.
Native turns service eight commands before checking input. Completed outcomes retain their
original admission independently of input pressure, retirement and cleanup. Unstarted
cancellation is **not sent**; an interrupted started effect is **unconfirmed**. No command replays.

The input mailbox admits 32 records / 256 KiB. Each publication charges 8 KiB; global input
storage admits 128 lifetimes / 1 MiB, retained through queued edit disposal. A native poll
returns at most eight bounded records, with at most sixteen buttons in each of the six button
lists and 1024-byte diagnostics. Pending input prevents further native reads, while command
handling and rumble-stop maintenance continue. This prevents a stalled consumer from extending
a rumble simply because input publication is full. Pressure transitions notify a warning.
Failed rumble-stop writes keep a 250 ms retry deadline for that controller; other controllers
continue servicing their own deadlines. A new rumble replaces its previous stop deadline.

Actor turns project two input records and sixteen command outcomes. Button and connection
changes are applied in arrival order; analog-only pending state follows the processing cap.
Retired worker input carries invalid revision guards. Input projection retains its original
payload through queued application/rejection; completed effects use the current module identity.
Startup failure diagnostics retain the actor guard after native termination, so the cause can
still be published while a live runtime handle observes the failed start.

## Remaining qualification

The [execution ledger](../progress/implementation-status.md) owns C08 acceptance. Native-prepared
callbacks and callback-capacity recovery, complete queued target/path guards, canonical command/
script capture with producer retirement, and engine-level pressure/reconfiguration/recovery
qualification remain open. These runtime bounds do not qualify the SDK's transient discovery
allocation, physical devices, cross-platform behavior or full-product scale.
