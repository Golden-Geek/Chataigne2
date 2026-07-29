import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createWebSocketUiClient } from '../../../../../../packages/golden-ui/transport/ws';
import type {
	EventTime,
	UiAck,
	UiControlLifecycle,
	UiEventBatch
} from '../../../../../../packages/golden-ui/types';

class FakeWebSocket {
	static readonly CONNECTING = 0;
	static readonly OPEN = 1;
	static readonly CLOSING = 2;
	static readonly CLOSED = 3;
	static instances: FakeWebSocket[] = [];

	readonly sent: string[] = [];
	readyState = FakeWebSocket.CONNECTING;
	onopen: (() => void) | null = null;
	onmessage: ((event: { data: string }) => void) | null = null;
	onerror: (() => void) | null = null;
	onclose: (() => void) | null = null;

	constructor(readonly url: string | URL) {
		FakeWebSocket.instances.push(this);
	}

	send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
		this.sent.push(String(data));
	}

	open(): void {
		this.readyState = FakeWebSocket.OPEN;
		this.onopen?.();
	}

	close(): void {
		this.readyState = FakeWebSocket.CLOSED;
		this.onclose?.();
	}

	emitStaleClose(): void {
		this.onclose?.();
	}

	receive(message: unknown): void {
		this.onmessage?.({ data: JSON.stringify(message) });
	}
}

interface SentMessage {
	kind: string;
	subscription_id?: string;
	request_id?: string;
	from?: EventTime;
}

const time = (tick: number): EventTime => ({ tick, micro: 0, seq: 0 });

const deferred = <T>() => {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});
	return { promise, resolve, reject };
};

const flushMicrotasks = async (): Promise<void> => {
	for (let index = 0; index < 6; index += 1) {
		await Promise.resolve();
	}
};

const messages = (socket: FakeWebSocket, kind: string): SentMessage[] =>
	socket.sent
		.map((entry) => JSON.parse(entry) as SentMessage)
		.filter((message) => message.kind === kind);

const wireEvent = (tick: number) => ({
	time: time(tick),
	kind: 'custom',
	topic: 'test.resync',
	origin: null,
	payload: { tick },
	retention: 'replay'
});

const receiveDelta = (
	socket: FakeWebSocket,
	subscriptionId: string,
	from: EventTime,
	ids: number[]
): void => {
	socket.receive({
		kind: 'delta',
		subscription_id: subscriptionId,
		deltas: [
			{
				plane: 'structure',
				batch: {
					from,
					to: time(ids.at(-1) ?? from.tick),
					runtime: null,
					events: ids.map(wireEvent)
				}
			}
		]
	});
};

const appliedAcknowledgement = (): UiAck => ({
	success: true,
	status: 'applied',
	history: {
		can_undo: true,
		can_redo: false,
		undo_len: 1,
		redo_len: 0,
		active_edit_session: false,
		current_history_state_id: 1
	}
});

describe('websocket resync and socket-generation barriers', () => {
	let frames: FrameRequestCallback[];

	beforeEach(() => {
		vi.useFakeTimers();
		vi.spyOn(console, 'warn').mockImplementation(() => undefined);
		vi.spyOn(console, 'error').mockImplementation(() => undefined);
		FakeWebSocket.instances = [];
		frames = [];
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback): number => {
			frames.push(callback);
			return frames.length;
		});
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.unstubAllGlobals();
		vi.useRealTimers();
	});

	const runFrame = (): void => {
		const callback = frames.shift();
		if (!callback) {
			throw new Error('no animation frame was scheduled');
		}
		callback(0);
	};

	const runAllFrames = (): void => {
		while (frames.length > 0) {
			runFrame();
		}
	};

	it('drops staged and in-flight deltas until the exact snapshot boundary is resumed', async () => {
		const snapshots: Array<ReturnType<typeof deferred<EventTime>>> = [];
		const applied: UiEventBatch[] = [];
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket,
			onResyncRequired: () => {
				const snapshot = deferred<EventTime>();
				snapshots.push(snapshot);
				return snapshot.promise;
			}
		});
		const unsubscribe = client.subscribe({ kind: 'wholeGraph' }, time(0), (batch) =>
			applied.push(batch)
		);
		const socket = FakeWebSocket.instances[0];
		if (!socket) {
			throw new Error('websocket was not created');
		}
		socket.open();
		await flushMicrotasks();
		const initialSubscriptions = messages(socket, 'subscribe');
		expect(initialSubscriptions).toHaveLength(1);
		const subscriptionId = initialSubscriptions[0].subscription_id;
		if (!subscriptionId) {
			throw new Error('subscription id is missing');
		}

		const burst = Array.from({ length: 1_024 }, (_, index) => index + 1);
		receiveDelta(socket, subscriptionId, time(0), burst);
		runFrame();
		expect(applied.flatMap((batch) => batch.events)).toHaveLength(512);
		expect(frames).toHaveLength(1);

		socket.receive({
			kind: 'resyncRequired',
			subscription_id: subscriptionId,
			plane: 'structure',
			reason: 'cursor_out_of_retention_window'
		});
		await flushMicrotasks();
		expect(snapshots).toHaveLength(1);
		expect(messages(socket, 'unsubscribe')).toHaveLength(1);

		receiveDelta(socket, subscriptionId, time(1_024), [1_025, 1_026]);
		runAllFrames();
		expect(applied.flatMap((batch) => batch.events)).toHaveLength(512);

		snapshots[0].resolve(time(2_000));
		await flushMicrotasks();
		const resumed = messages(socket, 'subscribe');
		expect(resumed).toHaveLength(2);
		expect(resumed[1].from).toEqual(time(2_000));

		receiveDelta(socket, subscriptionId, time(2_000), [2_001]);
		runAllFrames();
		expect(
			applied.flatMap((batch) =>
				batch.events.map((event) =>
					event.kind.kind === 'custom'
						? (event.kind.payload as { tick: number }).tick
						: -1
				)
			)
		).toEqual([...burst.slice(0, 512), 2_001]);
		unsubscribe();
	});

	it('coalesces repeated resync requests into one snapshot and one resume subscription', async () => {
		const snapshot = deferred<EventTime>();
		const onResyncRequired = vi.fn(() => snapshot.promise);
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket,
			onResyncRequired
		});
		const unsubscribe = client.subscribe({ kind: 'wholeGraph' }, time(0), () => undefined);
		const socket = FakeWebSocket.instances[0];
		if (!socket) {
			throw new Error('websocket was not created');
		}
		socket.open();
		await flushMicrotasks();
		const subscriptionId = messages(socket, 'subscribe')[0]?.subscription_id;
		if (!subscriptionId) {
			throw new Error('subscription id is missing');
		}

		for (const reason of ['first_gap', 'second_gap']) {
			socket.receive({
				kind: 'resyncRequired',
				subscription_id: subscriptionId,
				plane: null,
				reason
			});
		}
		await flushMicrotasks();
		expect(onResyncRequired).toHaveBeenCalledTimes(1);
		expect(messages(socket, 'unsubscribe')).toHaveLength(1);

		snapshot.resolve(time(50));
		await flushMicrotasks();
		expect(messages(socket, 'subscribe')).toHaveLength(2);
		expect(messages(socket, 'subscribe')[1].from).toEqual(time(50));
		unsubscribe();
	});

	it('invalidates an old resync completion and restarts the barrier after reconnect', async () => {
		const snapshots: Array<ReturnType<typeof deferred<EventTime>>> = [];
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket,
			onResyncRequired: () => {
				const snapshot = deferred<EventTime>();
				snapshots.push(snapshot);
				return snapshot.promise;
			}
		});
		const unsubscribe = client.subscribe({ kind: 'wholeGraph' }, time(0), () => undefined);
		const firstSocket = FakeWebSocket.instances[0];
		if (!firstSocket) {
			throw new Error('websocket was not created');
		}
		firstSocket.open();
		await flushMicrotasks();
		const subscriptionId = messages(firstSocket, 'subscribe')[0]?.subscription_id;
		if (!subscriptionId) {
			throw new Error('subscription id is missing');
		}
		firstSocket.receive({
			kind: 'resyncRequired',
			subscription_id: subscriptionId,
			plane: null,
			reason: 'restart_during_snapshot'
		});
		await flushMicrotasks();
		expect(snapshots).toHaveLength(1);

		firstSocket.close();
		await vi.advanceTimersByTimeAsync(251);
		const secondSocket = FakeWebSocket.instances[1];
		if (!secondSocket) {
			throw new Error('reconnect websocket was not created');
		}
		secondSocket.open();
		await flushMicrotasks();
		expect(snapshots).toHaveLength(2);
		expect(messages(secondSocket, 'subscribe')).toHaveLength(0);

		snapshots[0].resolve(time(40));
		await flushMicrotasks();
		expect(messages(secondSocket, 'subscribe')).toHaveLength(0);

		snapshots[1].resolve(time(60));
		await flushMicrotasks();
		expect(messages(secondSocket, 'subscribe')).toHaveLength(1);
		expect(messages(secondSocket, 'subscribe')[0].from).toEqual(time(60));
		unsubscribe();
	});

	it('ignores stale socket events without rejecting work owned by the replacement socket', async () => {
		const phases: UiControlLifecycle[] = [];
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
		});
		const unsubscribe = client.subscribe({ kind: 'wholeGraph' }, time(0), () => undefined);
		const firstSocket = FakeWebSocket.instances[0];
		if (!firstSocket) {
			throw new Error('websocket was not created');
		}
		firstSocket.open();
		await flushMicrotasks();
		firstSocket.close();
		await vi.advanceTimersByTimeAsync(251);
		const secondSocket = FakeWebSocket.instances[1];
		if (!secondSocket) {
			throw new Error('reconnect websocket was not created');
		}
		secondSocket.open();
		await flushMicrotasks();

		const completion = client.sendIntent(
			{ kind: 'duplicateNode', source: 1, new_parent: 2 },
			(phase) => phases.push(phase)
		);
		await flushMicrotasks();
		const requestId = messages(secondSocket, 'intent')[0]?.request_id;
		if (!requestId) {
			throw new Error('intent request id is missing');
		}

		firstSocket.emitStaleClose();
		secondSocket.receive({
			kind: 'control',
			update: { request_id: requestId, phase: 'received' }
		});
		secondSocket.receive({
			kind: 'control',
			update: {
				request_id: requestId,
				phase: 'applied',
				acknowledgement: appliedAcknowledgement()
			}
		});
		await expect(completion).resolves.toMatchObject({ success: true });
		expect(phases).toEqual(['received', 'applied']);
		unsubscribe();
	});

	it('recovers from close-before-open and subscribes exactly once on the winning generation', async () => {
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
		});
		const unsubscribe = client.subscribe({ kind: 'wholeGraph' }, time(0), () => undefined);
		const firstSocket = FakeWebSocket.instances[0];
		if (!firstSocket) {
			throw new Error('websocket was not created');
		}
		firstSocket.close();
		await flushMicrotasks();
		await vi.advanceTimersByTimeAsync(251);
		const secondSocket = FakeWebSocket.instances[1];
		if (!secondSocket) {
			throw new Error('replacement websocket was not created');
		}
		secondSocket.open();
		await flushMicrotasks();

		expect(messages(secondSocket, 'subscribe')).toHaveLength(1);
		firstSocket.open();
		await flushMicrotasks();
		expect(messages(firstSocket, 'subscribe')).toHaveLength(0);
		expect(messages(secondSocket, 'subscribe')).toHaveLength(1);
		unsubscribe();
	});
});
