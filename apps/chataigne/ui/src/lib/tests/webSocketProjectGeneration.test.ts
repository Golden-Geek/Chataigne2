import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest';
import { createWebSocketUiClient } from 'golden_ui/transport/ws';
import { snapshotCursor, projectGeneration } from 'golden_ui/transport/replay-cursor';
import { testSessionProvider } from 'golden_ui/testing/transport';
import type { UiReplayCursor, UiTransportBatch } from 'golden_ui/types';
import { FakeWebSocket } from './fixtures/websocket';

const time = (tick: number) => ({ tick, micro: 0, seq: 0 });
const cursor = (generation: number, tick: number): UiReplayCursor => ({
	project_generation: generation,
	at: time(tick)
});
const drain = async () => {
	for (let index = 0; index < 15; index++) await Promise.resolve();
};
const sent = (socket: FakeWebSocket, kind: string) =>
	socket.sent.map((value) => JSON.parse(value)).filter((message) => message.kind === kind);
function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => (resolve = done));
	return { resolve, promise };
}
const delta = (socket: FakeWebSocket, id: string, generation: number, tick: number) =>
	socket.receive({
		kind: 'delta',
		project_generation: generation,
		subscription_id: id,
		deltas: [
			{
				plane: 'observation',
				batch: {
					events: [{ time: time(tick), kind: { kind: 'custom', topic: 'fixture', payload: null } }],
					from: time(tick - 1),
					to: time(tick),
					runtime: { engine_hz: tick, generation_id: 1, graph_version: 1, structure_epoch: 1 }
				}
			}
		]
	});

describe('project lifetime on the live transport', () => {
	let frames: FrameRequestCallback[] = [];
	const paint = () => {
		while (frames.length) frames.shift()!(0);
	};
	beforeEach(() => {
		FakeWebSocket.instances = [];
		frames = [];
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
			frames.push(callback);
			return frames.length;
		});
		vi.stubGlobal('cancelAnimationFrame', () => {});
	});
	afterEach(() => {
		vi.unstubAllGlobals();
	});
	it('captures the snapshot cursor instead of borrowing its mutable event time', async () => {
		const from = cursor(7, 20);
		const applied: UiTransportBatch[] = [];
		const client = createWebSocketUiClient({
			sessionProvider: testSessionProvider,
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
		});
		const stop = client.subscribe({ kind: 'wholeGraph' }, from, (batch) => applied.push(batch));
		from.at.tick = 999;
		from.project_generation = 999;
		await drain();
		const socket = FakeWebSocket.instances[0];
		socket.open();
		await drain();
		const message = sent(socket, 'subscribe')[0];
		expect(message.from).toEqual(cursor(7, 20));
		delta(socket, message.subscription_id, 7, 21);
		paint();
		expect(applied[0].project_generation).toBe(7);
		stop();
	});
	it('resumes a reset clock in the new project and drops late packets from the previous lifetime', async () => {
		const reply = deferred<UiReplayCursor>(),
			applied: UiTransportBatch[] = [];
		const resync = vi.fn(() => reply.promise);
		const client = createWebSocketUiClient({
			sessionProvider: testSessionProvider,
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket,
			onResyncRequired: resync
		});
		const stop = client.subscribe({ kind: 'wholeGraph' }, cursor(7, 20), (batch) =>
			applied.push(batch)
		);
		await drain();
		const socket = FakeWebSocket.instances[0];
		socket.open();
		await drain();
		const id = sent(socket, 'subscribe')[0].subscription_id;
		delta(socket, id, 8, 2);
		await drain();
		expect(resync).toHaveBeenCalledTimes(1);
		paint();
		expect(applied).toEqual([]);
		const boundary = cursor(8, 2);
		reply.resolve(boundary);
		await drain();
		boundary.at.tick = 999;
		expect(sent(socket, 'subscribe')[1].from).toEqual(cursor(8, 2));
		delta(socket, id, 7, 999);
		paint();
		expect(applied).toEqual([]);
		delta(socket, id, 8, 3);
		paint();
		expect(applied).toHaveLength(1);
		expect(applied[0].project_generation).toBe(8);
		expect(applied[0].to).toEqual(time(3));
		stop();
	});
	it('rejects malformed snapshot identity without leaving the read promise pending', async () => {
		const client = createWebSocketUiClient({
			sessionProvider: testSessionProvider,
			webSocketImpl: FakeWebSocket as unknown as typeof WebSocket
		});
		const result = expect(client.snapshot()).rejects.toThrow('project generation');
		await drain();
		const socket = FakeWebSocket.instances[0];
		socket.open();
		await drain();
		const request = sent(socket, 'snapshot')[0];
		socket.receive({
			kind: 'snapshot',
			request_id: request.request_id,
			snapshot: {
				project_generation: 0,
				at: time(1),
				scope: 'wholeGraph',
				schema: { enums: [], node_types: [], declared_descriptions: [] },
				nodes: [],
				protocol_version: 'test'
			}
		});
		await result;
		socket.close();
	});
	it('keeps generation and event time together and rejects invalid generation scalars', () => {
		expect(snapshotCursor({ project_generation: 2, at: time(1) })).toEqual(cursor(2, 1));
		for (const value of [undefined, 0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1])
			expect(() => projectGeneration(value)).toThrow('project generation');
	});
});
