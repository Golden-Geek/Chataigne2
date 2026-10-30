// Delivered after commit or rejection, including results from direct child commands.
// reply = { status: "applied", request_id, value: ScreenEditResult }
//      or { status: "rejected", request_id, message }
function onAppOperationResult(reply) {
	if (reply.status === 'rejected') {
		warn('Screen operation', reply.request_id, reply.message);
		return;
	}
	// Use returned identities to route subsequent requests.
	void reply;
}
