// Called after the operation commits or is rejected. Direct gc.invokeAppOperation requests
// use this same callback. Manager listeners also receive results from its direct commands.
// reply = {status: "applied", request_id, value: MediaEditResult}
//      or {status: "rejected", request_id, message}
function onAppOperationResult(reply) {
  if (reply.status === "rejected") {
    warn("Media operation", reply.request_id, reply.message);
    return;
  }
  // Persist/reference reply.value identities; never invent media, channel or asset ids.
  void reply;
}

// Ordered process-phase transitions, distinct from coalesced runtime snapshots.
// Listen to the Media Manager in init with local.listen({level: 1}).
// event = {project, sequence, elapsed_ms, status: ApplicationRuntimeStatus}.
// Use project/sequence/launch to associate delayed delivery with its actual owned launch.
function onApplicationLifecycle(event) {
  void event;
}
