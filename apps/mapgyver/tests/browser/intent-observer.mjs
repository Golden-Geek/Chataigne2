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

// Native WebViews can connect before Playwright attaches. CDP also observes frames on
// those existing sockets; a browser reload would change the session being qualified.
export async function observeNativeRejectedRequests(page, stage) {
	const rejected = [];
	const observed = { requests: 0, receipts: 0 };
	const sockets = new Map();
	const client = await page.context().newCDPSession(page);
	client.on('Network.webSocketFrameSent', ({ requestId, response }) => {
		try {
			const frame = JSON.parse(response.payloadData);
			if (frame.kind !== 'intent' && frame.kind !== 'intentBatch') return;
			observed.requests++;
			let pending = sockets.get(requestId);
			if (!pending) sockets.set(requestId, (pending = new Map()));
			pending.set(frame.request_id, { stage: stage(), intent: frame.intent ?? frame.intents });
		} catch {}
	});
	client.on('Network.webSocketFrameReceived', ({ requestId, response }) => {
		try {
			const frame = JSON.parse(response.payloadData);
			if (frame.kind !== 'control') return;
			const update = frame.update;
			if (update.phase !== 'applied' && update.phase !== 'rejected') return;
			observed.receipts++;
			const pending = sockets.get(requestId);
			const request = pending?.get(update.request_id);
			pending?.delete(update.request_id);
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
	await client.send('Network.enable');
	return { rejected, observed };
}
