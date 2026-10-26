/** Bounded passive diagnostics: never records credentials or full graph/protocol payloads. */
export function createWireDiagnostics() {
	const rows = [];
	return {
		observe({ page }) {
			page.on('websocket', (socket) => {
				for (const direction of ['framesent', 'framereceived'])
					socket.on(direction, ({ payload }) => {
						const text = typeof payload === 'string' ? payload : payload.toString('utf8');
						const kind = /"kind"\s*:\s*"([^"]+)"/.exec(text.slice(0, 1024))?.[1];
						const row = { at: performance.now(), direction, kind, bytes: Buffer.byteLength(text) };
						if (text.length < 65536) {
							try {
								const value = JSON.parse(text);
								row.fields = Object.keys(value);
								row.id = value.id;
								const acks = value.acks ?? (value.ack ? [value.ack] : []);
								row.acks = acks.map(({ success, status, error_code }) => ({
									success,
									status,
									error_code
								}));
							} catch {
								/* Binary frame: byte count remains evidence. */
							}
						}
						rows.push(row);
						if (rows.length > 256) rows.shift();
					});
			});
		},
		get rows() {
			return [...rows];
		}
	};
}
