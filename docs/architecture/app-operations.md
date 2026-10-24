# App-owned operations

Golden owns transport admission, actor execution, atomic authored edits, history and completion.
The app owns operation schemas, validation, labels, identity remapping and domain mutation policy.
Generic graph and Golden packages must not import Chataigne types to implement these operations.

`UiEditIntent::InvokeAppOperation` carries an app namespace, operation name and JSON request.
`ProjectLifecycle::apply_app_operation` interprets the app DTO and queues ordinary engine edits.
App request/result DTOs should generate their TypeScript declarations from Rust. Golden transports
the payload without interpreting domain fields. Namespace and operation names each require
1–128 bytes; payload/fragment admission remains the owning app schema's responsibility.

The production application invokes the hook on its authoritative control thread. Native/headless
code with exclusive engine access uses `golden_core::application::apply_project_intent`. The bare
engine's generic UI entrypoint rejects app operations because it lacks the project lifecycle.
The default project hook rejects unsupported operations; a transport acknowledgement never
invents support for an app namespace.

The operation joins an existing edit group or receives one implicit history group. Its authored
edits, lifecycle callbacks and `settle_pending_operations` run inside the existing atomic edit
journal. A failure discards the app result and rolls back that operation while retaining an
already accepted prefix in an outer edit group. A successful `UiAck.result.value` is returned only
after stabilization and history completion. Domain handlers must queue authored changes through
engine edits and keep external side effects out of this rollback boundary. The typed result
container distinguishes an omitted result from a present JSON null using ordinary Serde and
generated declarations.

For creates, the backend can allocate detached UUIDs, apply one subtree/forest and return exact
created IDs/UUIDs. Callers consume those identities directly, independently of UI projection
timing. They must not identify creates by discovering the first unseen child.

Workbench callers use `session.invokeAppOperation(request)`. Its focused session adapter retains
the acknowledgement through the existing dispatch queue, history handling and visible activity,
then returns the exact app value. Missing/staged/rejected replies, transport failures and project
transition cancellation reject the call; the UI never fabricates a successful result. Presentation
completion may be delayed without changing the identities in that reply.

Protocol version 0.10.0 adds the operation envelope and optional result. Both are defined in
Rust and generated into `packages/golden-ui/generated/rust_protocol`; the UI result adapter
inherits the generated fields and the transport codec preserves the app result verbatim.
`SendNodeEvent` remains ephemeral inbox coordination and does not provide mutation completion.

P08 uses this contract for Formula paste/connect. That app implementation and its UI/script
callers remain separate checkpoints in the implementation ledger; the public boundary alone
does not remove the existing Formula UI mutation policy.
