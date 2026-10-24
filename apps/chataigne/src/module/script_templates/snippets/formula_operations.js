/*
 * Formula authoring uses the same backend operations as the editor and headless clients.
 * Tree handles expose id() and uuid; both identities fence a request against project changes.
 * Request and result shapes are generated from chataigne_alchemist::authoring.
 *
 * gc.invokeAppOperation("copy-1", "chataigne.alchemist", "copyFragment", {
 *   formula: { node: formula.id(), uuid: formula.uuid }, nodes: [anode.id()]
 * });
 * gc.invokeAppOperation("paste-1", "chataigne.alchemist", "pasteFragment", {
 *   formula: { node: formula.id(), uuid: formula.uuid }, fragment, offset: [2, 2]
 * });
 * gc.invokeAppOperation("connect-1", "chataigne.alchemist", "connect", {
 *   formula: { node: formula.id(), uuid: formula.uuid },
 *   from: { node: source.id(), uuid: source.uuid, socket: "value" },
 *   to: { node: target.id(), uuid: target.uuid, socket: "value1" }
 * });
 *
 * Requests are asynchronous; a true return means admitted, not committed.
 * function onAppOperationResult(reply) {
 *   // reply.request_id identifies the caller's request.
 *   // status === "applied": reply.value is the exact backend result, including null.
 *   // status === "rejected": reply.message explains atomic rejection.
 * }
 * Paste returns selected_nodes and identities for every imported record. Do not scan labels,
 * guess created children, remap references, or repair parameters in a later callback.
 */
