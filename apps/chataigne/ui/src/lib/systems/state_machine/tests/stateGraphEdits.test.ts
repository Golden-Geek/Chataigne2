import { describe, expect, it, vi } from 'vitest';
import { createFixtureSession, fixtureNode } from 'golden_ui/testing/session';
import { createStateGraphEdits, type StateGraphEditSession } from '../stateGraphEdits';
import type { StateGraphIdentity } from '../generated/StateGraphIdentity';
import { STATE_GRAPH_INPUT, STATE_GRAPH_OUTPUT } from '../generated/state-graph-constants';

const deferred = <T>() => {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
};
const drain = async () => {
	for (let index = 0; index < 12; index++) await Promise.resolve();
};
const nodes = () => [
	fixtureNode(0, 'Root', [1]),
	fixtureNode(1, 'States', [2, 3], 'state_machine_manager'),
	fixtureNode(2, 'First', [], 'state'),
	fixtureNode(3, 'Second', [], 'state')
];
function fixture() {
	const source = createFixtureSession();
	source.loadNodes(nodes());
	let generation = 0;
	const invoke = vi.fn<StateGraphEditSession['invokeAppOperation']>();
	const session: StateGraphEditSession = {
		graph: source.session.graph,
		editOperations: source.session.editOperations,
		get snapshotGeneration() {
			return generation;
		},
		invokeAppOperation: invoke
	};
	let current: StateGraphEditSession | null = session;
	const edits = createStateGraphEdits(() => ({
		session: current,
		manager: current?.graph.state.nodesById.get(1) ?? null
	}));
	const identity = (id: number): StateGraphIdentity => ({
		node: id,
		uuid: session.graph.state.nodesById.get(id)!.uuid
	});
	const result = (...ids: number[]) => ({ states: ids.map(identity), connection: null });
	return {
		source,
		session,
		edits,
		invoke,
		identity,
		result,
		advance: () => {
			generation++;
		},
		replace: () => {
			source.loadNodes(nodes());
		},
		disconnect: () => {
			current = null;
		}
	};
}

describe('state graph app adapter', () => {
	it('captures a complete move and never exposes private layout parameter edits', async () => {
		const f = fixture();
		f.invoke.mockResolvedValue(f.result(2, 3));
		const moves = [
			{ nodeId: '2', position: { x: 4, y: -2 } },
			{ nodeId: '3', position: { x: 9, y: 1 } }
		];
		const done = f.edits.move(moves);
		moves[0].position.x = 999;
		await done;
		expect(f.invoke).toHaveBeenCalledExactlyOnceWith({
			kind: 'invokeAppOperation',
			namespace: 'chataigne.stateGraph',
			operation: 'edit',
			payload: {
				manager: f.identity(1),
				edit: {
					kind: 'move',
					moves: [
						{ state: f.identity(2), position: [4, -2] },
						{ state: f.identity(3), position: [9, 1] }
					]
				}
			}
		});
		expect(f.session.editOperations.size).toBe(0);
	});

	it('returns backend and malformed acknowledgement failures to the canvas caller', async () => {
		const f = fixture();
		f.invoke.mockRejectedValueOnce(new Error('Native rejection'));
		await expect(f.edits.move([{ nodeId: '2', position: { x: 1, y: 2 } }])).rejects.toThrow(
			'Native rejection'
		);
		f.invoke.mockResolvedValueOnce(f.result(3));
		await expect(f.edits.collapse('2', true)).rejects.toThrow('different target identities');
		f.invoke.mockResolvedValueOnce({ states: [], connection: 'invalid' });
		await expect(f.edits.rename('2', 'Name')).rejects.toThrow('invalid committed result');
		f.invoke.mockResolvedValueOnce(f.result(2));
		await expect(f.edits.collapse('2', false)).resolves.toBeUndefined();
		expect(f.session.editOperations.size).toBe(0);
	});

	it.each(['snapshot', 'manager', 'session'] as const)(
		'never dispatches a queued edit after %s replacement',
		async (kind) => {
			const f = fixture();
			const firstResult =
				deferred<Awaited<ReturnType<StateGraphEditSession['invokeAppOperation']>>>();
			const reply = f.result(2);
			f.invoke.mockReturnValueOnce(firstResult.promise);
			const first = f.edits.collapse('2', true);
			await drain();
			expect(f.invoke).toHaveBeenCalledTimes(1);
			const queued = expect(
				f.edits.move([{ nodeId: '3', position: { x: 8, y: 9 } }])
			).rejects.toThrow('owner changed');
			if (kind === 'snapshot') f.advance();
			else if (kind === 'manager') f.replace();
			else f.disconnect();
			firstResult.resolve(reply);
			await first;
			await queued;
			expect(f.invoke).toHaveBeenCalledTimes(1);
			expect(f.session.editOperations.size).toBe(0);
		}
	);

	it('drains confirmed graph edits before a session save/history/project barrier', async () => {
		const f = fixture();
		const reply = deferred<Awaited<ReturnType<StateGraphEditSession['invokeAppOperation']>>>();
		f.invoke.mockReturnValueOnce(reply.promise);
		const move = f.edits.move([{ nodeId: '2', position: { x: 2, y: 3 } }]);
		await drain();
		const saved = vi.fn(async () => {});
		const barrier = f.session.editOperations.runBarrier(saved);
		await drain();
		expect(saved).not.toHaveBeenCalled();
		await expect(f.edits.collapse('2', true)).rejects.toThrow('busy');
		reply.resolve(f.result(2));
		await move;
		await barrier;
		expect(saved).toHaveBeenCalledTimes(1);
	});

	it('retains connection direction and all native metadata/size requests', async () => {
		const f = fixture();
		f.invoke.mockResolvedValueOnce({
			states: [f.identity(2), f.identity(3)],
			connection: { node: 99, uuid: 'connection' }
		});
		await f.edits.connect({
			from: { nodeId: '2', socketId: STATE_GRAPH_OUTPUT },
			to: { nodeId: '3', socketId: STATE_GRAPH_INPUT }
		});
		expect(f.invoke.mock.calls[0][0].payload).toEqual({
			manager: f.identity(1),
			edit: {
				kind: 'connect',
				from: { state: f.identity(2), socket: STATE_GRAPH_OUTPUT },
				to: { state: f.identity(3), socket: STATE_GRAPH_INPUT }
			}
		});
		f.invoke.mockResolvedValue(f.result(2));
		await f.edits.resize({ nodeId: '2', mode: 'custom', size: { width: 22, height: 14 } });
		await f.edits.resize({ nodeId: '2', mode: 'automatic' });
		await f.edits.rename('2', 'New name');
		await f.edits.enable('2', false);
		expect(f.invoke.mock.calls.slice(1).map(([request]) => request.payload)).toEqual([
			{ manager: f.identity(1), edit: { kind: 'resize', state: f.identity(2), size: [22, 14] } },
			{ manager: f.identity(1), edit: { kind: 'resize', state: f.identity(2), size: null } },
			{ manager: f.identity(1), edit: { kind: 'rename', state: f.identity(2), label: 'New name' } },
			{ manager: f.identity(1), edit: { kind: 'enable', state: f.identity(2), enabled: false } }
		]);
	});

	it('retires queued work on unmount and rejects absent targets without a dispatch', async () => {
		const f = fixture();
		await expect(f.edits.move([{ nodeId: 'missing', position: { x: 1, y: 2 } }])).rejects.toThrow(
			'unavailable'
		);
		const reply = deferred<Awaited<ReturnType<StateGraphEditSession['invokeAppOperation']>>>();
		f.invoke.mockReturnValueOnce(reply.promise);
		const first = f.edits.collapse('2', true);
		await drain();
		const queued = expect(f.edits.enable('3', false)).rejects.toThrow('owner changed');
		f.edits.dispose();
		reply.resolve(f.result(2));
		await first;
		await queued;
		await expect(f.edits.collapse('2', true)).rejects.toThrow('current manager');
		expect(f.invoke).toHaveBeenCalledTimes(1);
	});
});
