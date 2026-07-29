import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createWebSocketUiClient } from '../../../../../../packages/golden-ui/transport/ws';
import type { EventTime, UiEventBatch } from '../../../../../../packages/golden-ui/types';

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

interface SentSubscribe {
	kind: 'subscribe';
	subscription_id: string;
	from?: EventTime;
}

const time = (tick: number): EventTime => ({ tick, micro: 0, seq: 0 });

const flushMicrotasks = async (): Promise<void> => {
	await Promise.resolve();
	await Promise.resolve();
};

const sentSubscribe = (socket: FakeWebSocket): SentSubscribe => {
	const message = socket.sent
		.map((entry) => JSON.parse(entry) as { kind: string })
		.find((entry) => entry.kind === 'subscribe') as SentSubscribe | undefined;
	if (!message) {
		throw new Error('subscription request was not sent');
	}
	return message;
};

const sentIntentRequestId = (socket: FakeWebSocket): string => {
	const message = socket.sent
		.map((entry) => JSON.parse(entry) as { kind: string; request_id?: string })
		.find((entry) => entry.kind === 'intent');
	if (!message?.request_id) {
		throw new Error('intent request was not sent');
	}
	return message.request_id;
};

const wireEvent = (id: number) => ({
	time: time(id),
	kind: 'custom',
	topic: 'test.burst',
	origin: null,
	payload: { id },
	retention: 'replay'
});

const eventIds = (batch: UiEventBatch): number[] =>
	batch.events.map((event) => {
		if (event.kind.kind !== 'custom') {
			throw new Error(`unexpected event kind ${event.kind.kind}`);
		}
		return (event.kind.payload as { id: number }).id;
	});

const receiveDelta = (
	socket: FakeWebSocket,
	subscriptionId: string,
	planeEventIds: number[][],
	from: EventTime,
	to: EventTime
): void => {
	socket.receive({
		kind: 'delta',
		subscription_id: subscriptionId,
		deltas: planeEventIds.map((ids, index) => ({
			plane: index === 0 ? 'structure' : 'trigger',
			batch: {
				from,
				to,
				runtime: null,
				events: ids.map(wireEvent)
			}
		}))
	});
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

describe('websocket event burst scheduling', () => {
	let frames: FrameRequestCallback[];

	beforeEach(() => {
		vi.useFakeTimers();
		FakeWebSocket.instances = [];
		frames = [];
		let nextFrameId = 0;
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
			frames.push(callback);
			nextFrameId += 1;
			return nextFrameId;
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

	it('stages an atomic multi-plane envelope in exact order before applying bounded frame slices', async () => {
		const applied: UiEventBatch[] = [];
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
		});
		const initial = time(0);
		const unsubscribe = client.subscribe({ kind: 'wholeGraph' }, initial, (batch) =>
			applied.push(batch)
		);
		const socket = FakeWebSocket.instances[0];
		if (!socket) {
			throw new Error('websocket was not created');
		}
		socket.open();
		await flushMicrotasks();
		const subscription = sentSubscribe(socket);

		const eventCount = 4_096;
		const ids = Array.from({ length: eventCount }, (_, index) => index + 1);
		const firstPlane = [...ids.filter((id) => id % 4 === 0), ...ids.filter((id) => id % 4 === 2)];
		const secondPlane = [...ids.filter((id) => id % 4 === 1), ...ids.filter((id) => id % 4 === 3)];
		receiveDelta(
			socket,
			subscription.subscription_id,
			[firstPlane, secondPlane],
			initial,
			time(eventCount)
		);

		expect(frames).toHaveLength(1);
		runFrame();
		expect(applied).toHaveLength(1);
		expect(applied[0].events).toHaveLength(512);
		expect(eventIds(applied[0])).toEqual(ids.slice(0, 512));
		expect(frames).toHaveLength(1);

		const lifecycle: string[] = [];
		const intentCompletion = client.sendIntent(
			{
				kind: 'duplicateNode',
				source: 10,
				new_parent: 20
			},
			(phase) => lifecycle.push(phase)
		);
		await flushMicrotasks();
		const requestId = sentIntentRequestId(socket);
		socket.receive({
			kind: 'control',
			update: { request_id: requestId, phase: 'received' }
		});
		expect(lifecycle).toEqual(['received']);
		expect(applied).toHaveLength(1);
		socket.receive({
			kind: 'control',
			update: {
				request_id: requestId,
				phase: 'applied',
				acknowledgement: appliedAcknowledgement
			}
		});
		await intentCompletion;

		while (frames.length > 0) {
			runFrame();
		}

		expect(applied.every((batch) => batch.events.length <= 512)).toBe(true);
		expect(applied.flatMap(eventIds)).toEqual(ids);
		expect(applied[0].from).toEqual(initial);
		for (let index = 1; index < applied.length; index += 1) {
			expect(applied[index].from).toEqual(applied[index - 1].to);
		}
		expect(applied.at(-1)?.to).toEqual(time(eventCount));
		unsubscribe();
	});

	it('reconnects from the last applied slice and discards the unsent staged tail', async () => {
		vi.spyOn(console, 'warn').mockImplementation(() => undefined);
		const applied: UiEventBatch[] = [];
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
		});
		const initial = time(0);
		const unsubscribe = client.subscribe({ kind: 'wholeGraph' }, initial, (batch) =>
			applied.push(batch)
		);
		const firstSocket = FakeWebSocket.instances[0];
		if (!firstSocket) {
			throw new Error('websocket was not created');
		}
		firstSocket.open();
		await flushMicrotasks();
		const firstSubscription = sentSubscribe(firstSocket);
		const eventCount = 2_048;
		const ids = Array.from({ length: eventCount }, (_, index) => index + 1);
		receiveDelta(firstSocket, firstSubscription.subscription_id, [ids], initial, time(eventCount));

		runFrame();
		expect(eventIds(applied[0])).toEqual(ids.slice(0, 512));
		expect(applied[0].to).toEqual(time(512));
		expect(frames).toHaveLength(1);

		firstSocket.close();
		await vi.advanceTimersByTimeAsync(251);
		const secondSocket = FakeWebSocket.instances[1];
		if (!secondSocket) {
			throw new Error('reconnect websocket was not created');
		}
		secondSocket.open();
		await flushMicrotasks();
		const secondSubscription = sentSubscribe(secondSocket);
		expect(secondSubscription.subscription_id).toBe(firstSubscription.subscription_id);
		expect(secondSubscription.from).toEqual(time(512));

		receiveDelta(
			secondSocket,
			secondSubscription.subscription_id,
			[ids.slice(512)],
			time(512),
			time(eventCount)
		);
		while (frames.length > 0) {
			runFrame();
		}

		expect(applied.flatMap(eventIds)).toEqual(ids);
		expect(applied.at(-1)?.to).toEqual(time(eventCount));
		unsubscribe();
	});
});
