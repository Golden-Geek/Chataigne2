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
