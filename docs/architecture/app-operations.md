# App-owned operations

Golden owns transport admission, actor execution, atomic authored edits, history and completion.
The app owns operation schemas, validation, labels, identity remapping and domain mutation policy.
Generic graph and Golden packages must not import Chataigne types to implement these operations.

The application facade calls `ProjectLifecycle::validate_ui_intent` before admitting any intent or
entering history. Apps can require their typed operations for structures with coupled ownership,
such as Matrix layers and their exported channels. Rejection returns `app_intent_rejected` without
mutating the document; production UI/transport and `apply_project_intent` share the same hook.
Its default accepts ordinary intents. Pure engine edits remain trusted internal mechanisms.

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

## Embedded scripts

Golden's script runtime prepares bounded JSON and queues `gc.invokeAppOperation(requestId,
namespace, operation, payload)`. A true return means admitted, not committed. The same app hook
and atomic journal apply the request; scripts do not implement their own domain mutation policy.
The requesting runtime receives `onAppOperationResult(reply)`. Its Rust-defined,
generated `ScriptAppOperationReply` is either `status: "applied"` with the exact `value`, including
null, or `status: "rejected"` with `message`; both include the caller's `request_id`.

The deferred engine edit observes the entire originating callback's edit receipt. Application
service waits for commit, rejects a failed receipt, and fences requests against script deletion,
reload and project retirement. A completion targets that runtime directly through transient
inbox delivery. It does not depend on subscriptions, UI projection, replay or a guessed identity.
Result callbacks use ordinary runtime edit policy, without adding authored undo steps. History
restoration rejects authored requests after replay rather than silently losing them.

The input/response lifetime pool has 64 slots and a 512 MiB ceiling. Every request admits both
input and response capacity before JSON parsing; each envelope is limited to 32 MiB, two million
JSON values and 128 levels. Result admission leaves room for its callback envelope and occurs
inside the atomic journal, so an oversized result rejects the mutation before commit. Payloads
retain one background disposal owner. VM instruction, wall-time and memory budgets still apply;
transport packet limits are independent of this local script boundary.

The production runtime and public synchronous project intent facade service bounded turns.
An exclusive engine driver running app-capable scripts calls
`golden_core::application::service_script_app_operations` after draining callback edits. At most
eight requests run per turn; requests created by completion callbacks wait for the next turn.
Bare engine UI intents still cannot route app namespaces without a project lifecycle.

Chataigne's Formula adapter and caller acceptance are described in
[Alchemist runtime](alchemist-runtime.md#authored-fragment-operations). The current checkpoint
status stays in the implementation ledger.
