import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createWebSocketUiClient } from '../../../../../../packages/golden-ui/transport/ws';
import type { UiControlLifecycle } from '../../../../../../packages/golden-ui/types';

class FakeWebSocket {
	static readonly CONNECTING = 0;
	static readonly OPEN = 1;
	static readonly CLOSING = 2;
	static readonly CLOSED = 3;
	static instances: FakeWebSocket[] = [];

	readonly url: string;
	readonly sent: string[] = [];
	readyState = FakeWebSocket.CONNECTING;
	onopen: (() => void) | null = null;
	onmessage: ((event: { data: string }) => void) | null = null;
	onerror: (() => void) | null = null;
	onclose: (() => void) | null = null;

	constructor(url: string | URL) {
		this.url = String(url);
		FakeWebSocket.instances.push(this);
	}

	send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
		this.sent.push(String(data));
	}

	close(): void {
		this.readyState = FakeWebSocket.CLOSED;
		this.onclose?.();
	}

	open(): void {
		this.readyState = FakeWebSocket.OPEN;
		this.onopen?.();
	}

	receive(message: unknown): void {
		this.onmessage?.({ data: JSON.stringify(message) });
	}
}

const flushMicrotasks = async (): Promise<void> => {
	await Promise.resolve();
	await Promise.resolve();
};

const sentIntentRequestId = (socket: FakeWebSocket): string => {
	const request = socket.sent
		.map((message) => JSON.parse(message) as { kind: string; request_id?: string })
		.find((message) => message.kind === 'intent');
	if (!request?.request_id) {
		throw new Error('intent request was not sent');
	}
	return request.request_id;
};

const appliedAcknowledgement = {
	success: true,
	status: 'applied' as const,
	history: {
		can_undo: true,
		can_redo: false,
		undo_len: 1,
		redo_len: 0,
		active_edit_session: false,
		current_history_state_id: 1
	}
};

const startIntent = async (onLifecycle?: (phase: UiControlLifecycle) => void) => {
	const client = createWebSocketUiClient({
		webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
	});
	const completion = client.sendIntent(
		{
			kind: 'duplicateNode',
			source: 10,
			new_parent: 20
		},
		onLifecycle
	);
	const socket = FakeWebSocket.instances[0];
	if (!socket) {
		throw new Error('websocket was not created');
	}
	socket.open();
	await flushMicrotasks();
	return { completion, socket, requestId: sentIntentRequestId(socket) };
};

describe('websocket intent lifecycle deadlines', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		FakeWebSocket.instances = [];
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('rejects a request that cannot leave a stuck connecting socket', async () => {
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
		});
		const completion = client.sendIntent({
			kind: 'duplicateNode',
			source: 10,
			new_parent: 20
		});
		const socket = FakeWebSocket.instances[0];
		if (!socket) {
			throw new Error('websocket was not created');
		}
		const rejection = expect(completion).rejects.toThrow(
			/intent send timeout after 4 seconds .*request was not sent/
		);

		await vi.advanceTimersByTimeAsync(4_001);
		await rejection;

		socket.open();
		await flushMicrotasks();
		expect(
			socket.sent.some((message) => (JSON.parse(message) as { kind: string }).kind === 'intent')
		).toBe(false);
	});

	it('uses the short deadline only until the runtime confirms receipt', async () => {
		const phases: UiControlLifecycle[] = [];
		const { completion, socket, requestId } = await startIntent((phase) => phases.push(phase));
		let settled = false;
		void completion.then(
			() => {
				settled = true;
			},
			() => {
				settled = true;
			}
		);

		socket.receive({
			kind: 'control',
			update: { request_id: requestId, phase: 'received' }
		});
		await vi.advanceTimersByTimeAsync(4_001);
		expect(settled).toBe(false);

		socket.receive({
			kind: 'control',
			update: { request_id: requestId, phase: 'accepted' }
		});
		await vi.advanceTimersByTimeAsync(10_000);
		expect(settled).toBe(false);

		socket.receive({
			kind: 'control',
			update: {
				request_id: requestId,
				phase: 'applied',
				acknowledgement: appliedAcknowledgement
			}
		});
		await expect(completion).resolves.toMatchObject({ success: true, status: 'applied' });
		expect(phases).toEqual(['received', 'accepted', 'applied']);
	});

	it('reports an explicit receipt timeout when no lifecycle update arrives', async () => {
		const { completion } = await startIntent();
		const rejection = expect(completion).rejects.toThrow(
			/intent receipt timeout after 4 seconds .*runtime did not confirm receipt/
		);

		await vi.advanceTimersByTimeAsync(4_001);
		await rejection;
	});

	it('reports that the outcome is unknown when accepted processing exceeds the watchdog', async () => {
		const { completion, socket, requestId } = await startIntent();
		socket.receive({
			kind: 'control',
			update: { request_id: requestId, phase: 'accepted' }
		});
		const rejection = expect(completion).rejects.toThrow(
			/intent processing timeout after 120 seconds .*runtime outcome is unknown/
		);

		await vi.advanceTimersByTimeAsync(120_001);
		await rejection;
	});
});
