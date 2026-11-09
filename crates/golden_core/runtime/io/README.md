# IO ownership and admission

This crate provides app-agnostic worker, readiness and resource-lifetime primitives.
App/domain owners keep desired state and decide what to prepare; IO workers perform blocking
operations and publish bounded results. Engine, UI and render callbacks never wait for cleanup.

- `BoundedWorker` bounds commands/results and keeps a lifetime slot through actual worker cleanup.
- `RetirementPool` reserves a fixed number of active/retiring tasks; rejection retains ownership.
- `PayloadPool` separately bounds retained data bytes across their final observers.
- `RevisionSource` provides payload-free, one-shot invalidation wakes with race-safe subscription.

An owner waiting for retirement capacity can use `RetirementPool::availability()`. Capture its
revision **before** attempting admission or canceling an old worker. If admission is full, retain
the latest desired work and a subscription to that captured guard. A release invalidates the
guard only after the slot is available; a release before subscription causes an immediate wake.
The callback runs outside the pool lock and must remain nonblocking and infallible.

Availability is a retry hint, not a reservation. Retry normal admission after waking; never raise
the cap, infer completion from cancellation, or use `wait_for_idle` on an engine/UI/render path.
Drop the subscription when work is admitted, superseded or no longer wanted. Resource bytes
retained beyond worker retirement still belong to their separate payload/native allocation budget.
