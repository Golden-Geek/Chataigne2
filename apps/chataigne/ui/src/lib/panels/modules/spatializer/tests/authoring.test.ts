import { describe, expect, it, vi } from 'vitest';
import { createFixtureSession, fixtureNode } from 'golden_ui/testing/session';
import type { UiNodeDto } from 'golden_ui';
import { createSpatializerAuthoring, type SpatializerSession } from '../authoring';
import type { SpatializerEdit } from '../generated/SpatializerEdit';
import type { SpatializerEditRequest } from '../generated/SpatializerEditRequest';
import type { SpatializerSpace } from '../generated/SpatializerSpace';

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}
const drain = async () => {
	for (let i = 0; i < 30; i++) await Promise.resolve();
};
function fixture() {
	const source = createFixtureSession();
	const field = fixtureNode(3, 'Position', [], 'parameter');
	field.data = {
		kind: 'parameter',
		param: {
			value: { kind: 'vec2', value: [0, 0] },
			default_value: { kind: 'vec2', value: [0, 0] },
			event_behaviour: 'Coalesce',
			read_only: false,
			constraints: { enum_options: [], policy: 'ClampAdapt' },
			ui_hints: {},
			control: { mode: 'manual', spec: { mode: 'manual' } },
			reference_allowed_targets: [],
			reference_visible_nodes: []
		}
	};
	source.loadNodes([
		fixtureNode(0, 'Root', [1]),
		fixtureNode(1, 'Spatializer', [2], 'spatializer_module'),
		fixtureNode(2, 'Source', [3], 'spatializer_source'),
		field
	]);
	let generation = 0,
		space: SpatializerSpace = 'xy',
		connected = true;
	const get = (id: number): UiNodeDto => source.session.graph.state.nodesById.get(id)!;
	const identity = (id: number) => ({ node: id, uuid: get(id).uuid });
	const reply = (...ids: number[]) => ({ endpoints: ids.map(identity), created: null });
	const invoke = vi
		.fn<SpatializerSession['invokeAppOperation']>()
		.mockImplementation(async (request, applied) => {
			applied?.([{ success: true, status: 'applied' }]);
			const { edit } = request.payload as unknown as SpatializerEditRequest;
			return reply(
				...(edit.kind === 'move'
					? edit.moves.map((move) => move.endpoint.node)
					: edit.kind === 'createTarget'
						? []
						: [edit.endpoint.node])
			);
		});
	const send = vi.fn<SpatializerSession['sendIntent']>().mockResolvedValue(undefined);
	const session: SpatializerSession = {
		graph: source.session.graph,
		editOperations: source.session.editOperations,
		get snapshotGeneration() {
			return generation;
		},
		invokeAppOperation: invoke,
		sendIntent: send
	};
	const edits = createSpatializerAuthoring(() => ({
		session: connected ? session : null,
		module: get(1),
		space
	}));
	const move = (x: number): SpatializerEdit => ({
		kind: 'move',
		moves: [{ endpoint: identity(2), position: [x, 0] }]
	});
	const block = () => {
		const gate = deferred<void>();
		const operation = session.editOperations.open({
			current: () => true,
			build: () => ({ accepted: true, finish: () => gate.promise, async cancel() {} })
		})!;
		const done = operation.confirm();
		return { release: () => gate.resolve(), done };
	};
	return {
		source,
		session,
		edits,
		invoke,
		send,
		get,
		identity,
		reply,
		move,
		block,
		advance: () => {
			generation++;
		},
		space: () => {
			space = 'xz';
		},
		disconnect: () => {
			connected = false;
		}
	};
}

describe('Spatializer authoring admission', () => {
	it('closes an acknowledged history group after a same-project snapshot retires its gesture', async () => {
		const f = fixture(),
			gesture = f.edits.gesture(f.get(2), f.get(3), vi.fn());
		gesture.offer(f.move(1));
		await drain();
		expect(f.invoke).toHaveBeenCalledTimes(1);
		f.advance();
		await gesture.cancel();
		expect(f.send.mock.calls.map(([intent]) => intent.kind)).toEqual(['beginEdit', 'endEdit']);
		const begin = f.send.mock.calls[0][0];
		expect(begin.kind).toBe('beginEdit');
		if (begin.kind === 'beginEdit')
			expect(f.send.mock.calls[1][0]).toEqual({
				kind: 'endEdit',
				client_edit_id: begin.client_edit_id
			});
		expect(f.session.editOperations.size).toBe(0);
	});
	it('rejection while confirming shares terminal cleanup and ends the history group once', async () => {
		const f = fixture(),
			failed = vi.fn(),
			failure = new Error('read-only');
		let reject!: (error: Error) => void;
		f.invoke.mockImplementationOnce(
			() =>
				new Promise((_resolve, fail) => {
					reject = fail;
				})
		);
		const gesture = f.edits.gesture(f.get(2), f.get(3), failed);
		gesture.offer(f.move(1));
		await drain();
		const checked = expect(gesture.finish()).rejects.toBe(failure);
		reject(failure);
		await checked;
		await drain();
		expect(failed).toHaveBeenCalledTimes(1);
		expect(f.send.mock.calls.map(([intent]) => intent.kind)).toEqual(['beginEdit', 'endEdit']);
		expect(f.session.editOperations.size).toBe(0);
	});
	it('a rejected BeginEdit is observed and releases admission before any pointer sample', async () => {
		const f = fixture(),
			failed = vi.fn(),
			failure = new Error('connection lost before begin');
		f.send.mockRejectedValueOnce(failure);
		const gesture = f.edits.gesture(f.get(2), f.get(3), failed);
		try {
			await drain();
			expect(failed).toHaveBeenCalledWith(failure);
			expect(f.session.editOperations.size).toBe(0);
			expect(f.invoke).not.toHaveBeenCalled();
		} finally {
			await gesture.cancel().catch(() => undefined);
		}
	});
	it('a retired gesture cannot report a successful confirmation', async () => {
		const f = fixture(),
			gesture = f.edits.gesture(f.get(2), f.get(3), vi.fn());
		f.advance();
		await expect(gesture.finish()).rejects.toThrow('retired');
		expect(f.invoke).not.toHaveBeenCalled();
		expect(f.session.editOperations.size).toBe(0);
	});
	it('captures coordinates and owner before waiting without private parameter edits', async () => {
		const f = fixture(),
			block = f.block(),
			edit = f.move(2);
		const done = f.edits.run(edit);
		if (edit.kind === 'move') edit.moves[0].position[0] = 99;
		await drain();
		expect(f.invoke).not.toHaveBeenCalled();
		block.release();
		await done;
		expect(f.invoke.mock.calls[0][0].payload).toMatchObject({
			module: f.identity(1),
			space: 'xy',
			edit: { kind: 'move', moves: [{ endpoint: f.identity(2), position: [2, 0] }] }
		});
		expect(f.send).not.toHaveBeenCalled();
		expect(f.session.editOperations.size).toBe(0);
	});
	it('reports the last acknowledged publication only after validating the committed result', async () => {
		const f = fixture(),
			published = vi.fn();
		const first = { tick: 1, micro: 2, seq: 3 },
			last = { tick: 1, micro: 2, seq: 4 };
		f.invoke.mockImplementationOnce(async (_, applied) => {
			applied?.([
				{ success: true, status: 'applied', latest_event_time: last },
				{ success: true, status: 'applied', latest_event_time: first }
			]);
			expect(published).not.toHaveBeenCalled();
			return f.reply(2);
		});
		await expect(f.edits.run(f.move(2), published)).resolves.toEqual(f.reply(2));
		expect(published).toHaveBeenCalledExactlyOnceWith(last);
		published.mockClear();
		f.invoke.mockResolvedValueOnce({ endpoints: [], created: null });
		await expect(f.edits.run(f.move(2), published)).rejects.toThrow('different endpoint');
		expect(published).not.toHaveBeenCalled();
	});
	it.each(['generation', 'space', 'module', 'session'])(
		'retires queued requests when the %s changes',
		async (kind) => {
			const f = fixture(),
				block = f.block();
			const done = f.edits.run(f.move(2));
			const checked = expect(done).rejects.toThrow('retired');
			if (kind === 'generation') f.advance();
			else if (kind === 'space') f.space();
			else if (kind === 'session') f.disconnect();
			else f.get(1).uuid = 'replacement';
			block.release();
			await checked;
			expect(f.invoke).not.toHaveBeenCalled();
			expect(f.session.editOperations.size).toBe(0);
		}
	);
	it('rejects malformed committed identities and recovers on the next operation', async () => {
		const f = fixture();
		f.invoke.mockResolvedValueOnce({ endpoints: [], created: null });
		await expect(f.edits.run(f.move(2))).rejects.toThrow('different endpoint');
		await expect(f.edits.run(f.move(3))).resolves.toEqual(f.reply(2));
		expect(f.session.editOperations.size).toBe(0);
	});
	it('coalesces a blocked gesture to one latest sample and one history group', async () => {
		const f = fixture(),
			gate = deferred<ReturnType<typeof f.reply>>(),
			failed = vi.fn();
		f.invoke.mockImplementationOnce(() => gate.promise);
		const gesture = f.edits.gesture(f.get(2), f.get(3), failed);
		gesture.offer(f.move(1));
		await drain();
		expect(f.invoke).toHaveBeenCalledTimes(1);
		for (let i = 2; i <= 200; i++) gesture.offer(f.move(i));
		await drain();
		expect(f.invoke).toHaveBeenCalledTimes(1);
		gate.resolve(f.reply(2));
		await drain();
		await gesture.finish();
		expect(f.invoke).toHaveBeenCalledTimes(2);
		expect(f.invoke.mock.calls[1][0].payload).toMatchObject({
			edit: { moves: [{ position: [200, 0] }] }
		});
		expect(f.send.mock.calls.map(([intent]) => intent.kind)).toEqual(['beginEdit', 'endEdit']);
		expect(failed).not.toHaveBeenCalled();
		expect(f.session.editOperations.size).toBe(0);
	});
	it('a save barrier discards unsent gesture samples and waits for its accepted prefix', async () => {
		const f = fixture(),
			gate = deferred<ReturnType<typeof f.reply>>();
		f.invoke.mockImplementationOnce(() => gate.promise);
		const gesture = f.edits.gesture(f.get(2), f.get(3), vi.fn());
		gesture.offer(f.move(1));
		await drain();
		gesture.offer(f.move(9));
		let saved = false;
		const barrier = f.session.editOperations.runBarrier(async () => {
			saved = true;
		});
		await drain();
		expect(saved).toBe(false);
		gate.resolve(f.reply(2));
		await barrier;
		expect(saved).toBe(true);
		expect(f.invoke).toHaveBeenCalledTimes(1);
		expect(f.send.mock.calls.map(([intent]) => intent.kind)).toEqual(['beginEdit', 'endEdit']);
		expect(f.session.editOperations.size).toBe(0);
	});
	it('does not begin a gesture in a replacement project after waiting', async () => {
		const f = fixture(),
			block = f.block();
		const gesture = f.edits.gesture(f.get(2), f.get(3), vi.fn());
		gesture.offer(f.move(1));
		f.advance();
		const cancelled = gesture.cancel();
		block.release();
		await cancelled;
		expect(f.invoke).not.toHaveBeenCalled();
		expect(f.send).not.toHaveBeenCalled();
		expect(f.session.editOperations.size).toBe(0);
	});
	it('backend rejection releases the gesture and preserves a later recovery', async () => {
		const f = fixture(),
			failed = vi.fn(),
			failure = new Error('read-only');
		f.invoke.mockRejectedValueOnce(failure);
		const gesture = f.edits.gesture(f.get(2), f.get(3), failed);
		gesture.offer(f.move(1));
		await drain();
		await expect(gesture.finish()).rejects.toBe(failure);
		expect(failed).toHaveBeenCalledWith(failure);
		await expect(f.edits.run(f.move(2))).resolves.toEqual(f.reply(2));
		expect(f.session.editOperations.size).toBe(0);
	});
	it('uses bounded shared admission under a producer burst', async () => {
		const f = fixture(),
			gate = deferred<ReturnType<typeof f.reply>>();
		f.invoke.mockImplementationOnce(() => gate.promise);
		const work = Array.from({ length: 64 }, (_, i) => f.edits.run(f.move(i)));
		await drain();
		await expect(f.edits.run(f.move(999))).rejects.toThrow('busy');
		expect(f.session.editOperations.size).toBe(64);
		gate.resolve(f.reply(2));
		await Promise.all(work);
		expect(f.session.editOperations.size).toBe(0);
	});
});
