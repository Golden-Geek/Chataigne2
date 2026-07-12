# Chataigne Workspace Tasks

`cargo xtask watch` is the checked-in live-development orchestrator. It owns the frontend and
backend process trees, fixed default ports, readiness timeouts, and coordinated shutdown.

The command emits newline-delimited JSON status events. A `watch.ready` version 2 event is emitted
only after all of these are true:

- the frontend serves a document on its configured port;
- `/api/ui/health` reports the backend and immutable engine read model ready;
- at least one real WebSocket UI client has an active subscription;
- the readiness response includes the currently published read-model revision.

`watch.ready` includes the frontend/backend URLs and ports, active WebSocket and subscribed-client
counts, and the read-model revision. A listening TCP port or an unsubscribed socket is not session
readiness.

Ctrl-C, application close, or a child failure shuts down the complete owned process tree and
releases both ports. Use `cargo xtask watch --help` for supported overrides.
