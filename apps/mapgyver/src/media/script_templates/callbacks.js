function onAppOperationResult(reply) {
	// Delivered after commit or rejection. Inspect reply.request_id and reply.status.
	// Applied results are in reply.value; rejection includes reply.message and no value.
	// Created/PictureImported results contain reply.value.reference.media/channel UUIDs.
	// Reads and rejected operations do not add history. Undo/Redo does not replay requests.
	if (reply.status === 'rejected') log('Media request rejected', reply.request_id, reply.message);
	// Example: if (reply.request_id === "create" && reply.status === "applied") {
	//   const ref = reply.value.reference;
	//   local.configureMediaResolution("size", ref.media, true, 1920, 1080);
	// }
}

// No update callback: inline scripts remain event-driven after loading.
