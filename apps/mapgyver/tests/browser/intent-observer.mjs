// Read-only receipt correlation shared by the real project-lifetime checks.
export function observeRejectedRequests(page, stage) {
	const rejected = [];
	page.on('websocket', (socket) => {
		const pending = new Map();
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				if (frame.kind === 'intent' || frame.kind === 'intentBatch')
					pending.set(frame.request_id, { stage: stage(), intent: frame.intent ?? frame.intents });
			} catch {}
		});
		socket.on('framereceived', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				if (frame.kind !== 'control') return;
				const update = frame.update;
				if (update.phase !== 'applied' && update.phase !== 'rejected') return;
				const request = pending.get(update.request_id);
				pending.delete(update.request_id);
				if (update.phase === 'rejected')
					rejected.push({
						request_id: update.request_id,
						request,
						errors: (update.acknowledgements ?? [update.acknowledgement])
							.filter(Boolean)
							.map((ack) => ({ error_code: ack.error_code, error_message: ack.error_message }))
					});
			} catch {}
		});
	});
	return rejected;
}
