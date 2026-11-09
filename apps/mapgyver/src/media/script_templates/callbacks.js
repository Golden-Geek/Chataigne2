function onAppOperationResult(reply) {
	// Delivered after commit or rejection. Inspect reply.request_id and reply.status.
	// Applied results are in reply.value; rejection includes reply.message and no value.
	// Created/PictureImported results contain reply.value.reference.media/channel UUIDs.
	// Composition creation returns InstancesCreated: reply.value.instances[0].instance is the owned media UUID.
	// Reads and rejected operations do not add history. Undo/Redo does not replay requests.
	// Video transport operations return reply.value.snapshot: settings, ready and playing.
	// inspectVideoFrame returns reply.value.frame with exact previous/next timestamps, or null.
	// inspectVideoSource returns reply.value.status with readiness, typed failure and automatic retry state.
	// Prepared means timing only; Ready means a decoded stream is available. Source failures do not fault the renderer.
	// A step pauses and changes runtime Current Position atomically; unavailable timestamps reject without a partial edit.
	// An accepted playback intent is not proof that a file decoder is ready or a frame presented.
	if (reply.status === 'rejected') log('Media request rejected', reply.request_id, reply.message);
	// Example: if (reply.request_id === "create" && reply.status === "applied") {
	//   const ref = reply.value.reference;
	//   local.configureMediaResolution("size", ref.media, true, 1920, 1080);
	// }
}

// No update callback: inline scripts remain event-driven after loading.
