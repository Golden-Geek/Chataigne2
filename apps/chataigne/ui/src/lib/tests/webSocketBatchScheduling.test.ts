import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { UI_PROTOCOL_VERSION } from '../../../../../../packages/golden-ui/generated/rust_protocol/protocol-version';
import { createGraphStore } from '../../../../../../packages/golden-ui/store/graph.svelte';
import { createWebSocketUiClient } from '../../../../../../packages/golden-ui/transport/ws';
import type {
	EventTime,
	UiEventBatch,
	UiNodeDto,
	UiSnapshot
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

const nodeMeta = {
	label: 'Test',
	short_name: 'test',
	enabled: true,
	can_be_disabled: false,
	user_permissions: {
		can_edit_name: false,
		can_remove_and_duplicate: false,
		can_edit_constraints: false,
		can_edit_tags: false,
		can_edit_color: false
	},
	tags: []
};

const graphNode = (id: number, children: number[] = []): UiNodeDto => ({
	node_id: id,
	uuid: `test-node-${id}`,
	decl_id: `node-${id}`,
	node_type: 'test',
	meta: nodeMeta,
	data: { kind: 'node', node_type: 'test' },
	user_role: 'regular',
	user_item_kind: '',
	accepted_user_item_kinds: [],
	creatable_user_items: [],
	children
});

const parameterNode = (id: number): UiNodeDto => ({
	...graphNode(id),
	data: {
		kind: 'parameter',
		param: {
			value: { Int: id } as never,
			default_value: { Int: 0 } as never,
			event_behaviour: 'Coalesce',
			read_only: false,
			constraints: { enum_options: [], policy: 'ClampAdapt' },
			ui_hints: {},
			control: { mode: 'manual', spec: { mode: 'manual' } },
			reference_allowed_targets: [],
			reference_visible_nodes: []
		}
	}
});

const graphSnapshot = (at: EventTime): UiSnapshot => ({
	protocol_version: UI_PROTOCOL_VERSION,
	scope: { kind: 'wholeGraph' },
	at,
	nodes: [graphNode(1)],
	schema: { node_types: [], declared_descriptions: [], enums: [] },
	history: {
		can_undo: false,
		can_redo: false,
		undo_len: 0,
		redo_len: 0,
		active_edit_session: false,
		current_history_state_id: 0
	},
	logger: { max_entries: 100, records: [] },
	project_file: { display_name: 'Test', extension: 'noisette', current_path: null }
});

const subtreeTransaction = (eventAt: EventTime, nodeCount: number) => {
	const firstNode = 2;
	const lastNode = firstNode + nodeCount - 1;
	const children = Array.from({ length: nodeCount - 1 }, (_, index) => firstNode + index + 1);
	const nodes = [
		graphNode(firstNode, children),
		...children.slice(0, -1).map((id) => graphNode(id)),
		parameterNode(lastNode)
	];
	return {
		time: eventAt,
		kind: 'graphTransaction',
		tx_id: 1,
		epoch: 1,
		base_graph_version: 0,
		next_graph_version: 1,
		ops: [
			{
				kind: 'subtreeInserted',
				root: firstNode,
				parent: 1,
				nodes,
				parent_children_after: [firstNode]
			}
		]
	};
};

const receivePlaneDelta = (
	socket: FakeWebSocket,
	subscriptionId: string,
	plane: string,
	from: EventTime,
	to: EventTime,
	events: unknown[]
): void => {
	socket.receive({
		kind: 'delta',
		subscription_id: subscriptionId,
		deltas: [{ plane, batch: { from, to, runtime: null, events } }]
	});
};

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

	it('projects a 20,000-node subtree across frames and publishes every graph index atomically', async () => {
		vi.spyOn(console, 'warn').mockImplementation(() => undefined);
		const graph = createGraphStore();
		const initial = time(0);
		const committed = time(20_000);
		graph.loadSnapshot(graphSnapshot(initial));
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
		});
		const unsubscribe = client.subscribe(
			{ kind: 'wholeGraph' },
			initial,
			(batch) => graph.applyBatch(batch),
			{ createEventWork: (event) => graph.createEventWork(event) }
		);
		const firstSocket = FakeWebSocket.instances[0];
		if (!firstSocket) {
			throw new Error('websocket was not created');
		}
		firstSocket.open();
		await flushMicrotasks();
		const firstSubscription = sentSubscribe(firstSocket);
		const transaction = subtreeTransaction(committed, 20_000);
		receivePlaneDelta(
			firstSocket,
			firstSubscription.subscription_id,
			'structure',
			initial,
			committed,
			[transaction]
		);

		runFrame();
		expect(graph.state.nodesById.size).toBe(1);
		expect(graph.state.lastEventTime).toEqual(initial);
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
		expect(secondSubscription.from).toEqual(initial);

		receivePlaneDelta(
			secondSocket,
			secondSubscription.subscription_id,
			'structure',
			initial,
			committed,
			[transaction]
		);
		while (frames.length > 0) {
			runFrame();
			if (frames.length > 0) {
				expect(graph.state.nodesById.size).toBe(1);
				expect(graph.state.lastEventTime).toEqual(initial);
			}
		}

		expect(graph.state.nodesById.size).toBe(20_001);
		expect(graph.state.childrenById.get(1)).toEqual([2]);
		expect(graph.state.childrenById.get(2)).toHaveLength(19_999);
		expect(graph.state.parentById.get(2)).toBe(1);
		expect(graph.state.parentById.get(3)).toBe(2);
		expect(graph.state.parentById.get(20_001)).toBe(2);
		expect(graph.state.paramsById.size).toBe(1);
		expect(graph.state.paramsById.get(20_001)?.value).toEqual({
			kind: 'int',
			value: 20_001
		});
		expect(graph.state.lastEventTime).toEqual(committed);

		secondSocket.close();
		await vi.advanceTimersByTimeAsync(251);
		const thirdSocket = FakeWebSocket.instances[2];
		if (!thirdSocket) {
			throw new Error('second reconnect websocket was not created');
		}
		thirdSocket.open();
		await flushMicrotasks();
		expect(sentSubscribe(thirdSocket).from).toEqual(committed);
		unsubscribe();
	});

	it('bounds sustained value-plane input by keeping only the newest value per parameter', async () => {
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

		for (let start = 1; start <= 20_000; start += 1_000) {
			const end = start + 999;
			receivePlaneDelta(
				socket,
				subscription.subscription_id,
				'value',
				time(start - 1),
				time(end),
				Array.from({ length: 1_000 }, (_, index) => {
					const value = start + index;
					return {
						time: time(value),
						kind: 'paramChanged',
						param: 42,
						old_value: { Int: value - 1 },
						new_value: { Int: value }
					};
				})
			);
		}

		expect(frames).toHaveLength(1);
		runFrame();
		expect(applied).toHaveLength(1);
		expect(applied[0].events).toHaveLength(1);
		expect(applied[0].to).toEqual(time(20_000));
		expect(applied[0].events[0]?.kind).toMatchObject({
			kind: 'paramChanged',
			param: 42,
			new_value: { kind: 'int', value: 20_000 }
		});
		expect(frames).toHaveLength(0);
		unsubscribe();
	});

	it('never coalesces latest values across a reliable event in the same envelope', async () => {
		const applied: UiEventBatch[] = [];
		const client = createWebSocketUiClient({
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
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
		const subscription = sentSubscribe(socket);

		socket.receive({
			kind: 'delta',
			subscription_id: subscription.subscription_id,
			deltas: [
				{
					plane: 'value',
					batch: {
						from: time(0),
						to: time(3),
						runtime: null,
						events: [
							{
								time: time(1),
								kind: 'paramChanged',
								param: 42,
								old_value: { Int: 0 },
								new_value: { Int: 1 }
							},
							{
								time: time(3),
								kind: 'paramChanged',
								param: 42,
								old_value: { Int: 1 },
								new_value: { Int: 2 }
							}
						]
					}
				},
				{
					plane: 'structure',
					batch: {
						from: time(0),
						to: time(3),
						runtime: null,
						events: [wireEvent(2)]
					}
				}
			]
		});
		runFrame();

		expect(applied).toHaveLength(1);
		expect(applied[0].events.map((event) => event.time.tick)).toEqual([1, 2, 3]);
		unsubscribe();
	});
});
