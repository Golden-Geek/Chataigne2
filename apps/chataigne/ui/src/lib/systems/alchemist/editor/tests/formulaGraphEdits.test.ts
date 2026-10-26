import { describe, it, expect, vi } from 'vitest';
import { createFixtureSession, fixtureNode } from 'golden_ui/testing/session';
import { createFormulaMutations, type FormulaEditorSession } from '../formulaMutations';
import { createFormulaGraphEdits } from '../formulaGraphEdits';

const deferred = <T>() => {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => (resolve = done));
	return { promise, resolve };
};
const drain = async () => {
	for (let i = 0; i < 12; i++) await Promise.resolve();
};
function fixture() {
	const f = createFixtureSession();
	f.loadNodes([
		fixtureNode(0, 'Root', [1]),
		fixtureNode(1, 'Formula', [2, 3], 'alchemist_formula'),
		fixtureNode(2, 'First', [], 'alchemist_anode'),
		fixtureNode(3, 'Second', [], 'alchemist_anode')
	]);
	let generation = 0,
		readOnly = false,
		owner = f.session.graph.state.nodesById.get(1)!,
		current: FormulaEditorSession | null;
	const invoke = vi.fn<FormulaEditorSession['invokeAppOperation']>();
	const session: FormulaEditorSession = {
		...f.session,
		get graph() {
			return f.session.graph;
		},
		get snapshotGeneration() {
			return generation;
		},
		invokeAppOperation: invoke
	};
	current = session;
	const status = vi.fn();
	const mutations = createFormulaMutations(
		() => ({ session: current, formula: owner, readOnly }),
		status
	);
	const edits = createFormulaGraphEdits(mutations);
	const id = (node: number) => ({ node, uuid: f.session.graph.state.nodesById.get(node)!.uuid });
	return {
		f,
		session,
		invoke,
		status,
		mutations,
		edits,
		id,
		result: (...nodes: number[]) => ({ nodes: nodes.map(id) }),
		replace: (kind: string) => {
			if (kind === 'snapshot') generation++;
			else if (kind === 'formula') owner = { ...owner, uuid: 'replacement' };
			else if (kind === 'readOnly') readOnly = true;
			else current = null;
		}
	};
}
describe('Alchemist captured graph edits', () => {
	it('captures full identities and positions before shared session admission', async () => {
		const f = fixture();
		f.invoke.mockResolvedValue(f.result(2, 3));
		const moves = [
			{ nodeId: '2', position: { x: 4, y: 5 } },
			{ nodeId: '3', position: { x: 6, y: 7 } }
		];
		const done = f.edits.move(moves);
		moves[0].position.x = 999;
		await done;
		expect(f.invoke).toHaveBeenCalledExactlyOnceWith({
			kind: 'invokeAppOperation',
			namespace: 'chataigne.alchemist',
			operation: 'editGraph',
			payload: {
				formula: f.id(1),
				edit: {
					kind: 'move',
					moves: [
						{ anode: f.id(2), position: [4, 5] },
						{ anode: f.id(3), position: [6, 7] }
					]
				}
			}
		});
		expect(f.session.editOperations.size).toBe(0);
	});
	it('returns connection failures and invalid acknowledgements, then recovers', async () => {
		const f = fixture();
		f.invoke.mockRejectedValueOnce(new Error('Backend rejected connection'));
		await expect(
			f.edits.connect({
				from: { nodeId: '2', socketId: 'value' },
				to: { nodeId: '3', socketId: 'value1' }
			})
		).rejects.toThrow('Backend rejected');
		f.invoke.mockResolvedValueOnce({ connection: null, replaced: [] });
		await expect(
			f.edits.connect({
				from: { nodeId: '2', socketId: 'value' },
				to: { nodeId: '3', socketId: 'value1' }
			})
		).rejects.toThrow('invalid result');
		f.invoke.mockResolvedValueOnce({ connection: { node: 99, uuid: 'connection' }, replaced: [] });
		await expect(
			f.edits.connect({
				from: { nodeId: '2', socketId: 'value' },
				to: { nodeId: '3', socketId: 'value1' }
			})
		).resolves.toBeUndefined();
		expect(f.status).toHaveBeenLastCalledWith('saved');
		// Exact new connection identity comes from the backend, independent of existing UI projection.
	});
	it.each(['snapshot', 'formula', 'readOnly', 'session'])(
		'retires queued moves after %s replacement',
		async (kind) => {
			const f = fixture(),
				reply = deferred<any>();
			f.invoke.mockReturnValueOnce(reply.promise);
			const first = f.edits.collapse('2', true);
			await drain();
			const queued = expect(
				f.edits.move([{ nodeId: '3', position: { x: 8, y: 9 } }])
			).rejects.toThrow('owner changed');
			f.replace(kind);
			reply.resolve(f.result(2));
			await first;
			await queued;
			expect(f.invoke).toHaveBeenCalledTimes(1);
			expect(f.session.editOperations.size).toBe(0);
		}
	);
	it('drains confirmed moves before barriers and rejects admission while saving', async () => {
		const f = fixture(),
			reply = deferred<any>();
		f.invoke.mockReturnValueOnce(reply.promise);
		const move = f.edits.move([{ nodeId: '2', position: { x: 1, y: 2 } }]);
		await drain();
		const save = vi.fn(async () => {}),
			barrier = f.session.editOperations.runBarrier(save);
		await drain();
		expect(save).not.toHaveBeenCalled();
		await expect(f.edits.collapse('2', true)).rejects.toThrow('busy');
		reply.resolve(f.result(2));
		await move;
		await barrier;
		expect(save).toHaveBeenCalledTimes(1);
		expect(f.status).toHaveBeenLastCalledWith('saved');
	});
	it('bounds pending work and cancels queued dispatch on unmount', async () => {
		const f = fixture(),
			reply = deferred<any>();
		f.invoke.mockReturnValueOnce(reply.promise);
		const first = f.edits.collapse('2', true);
		await drain();
		const queued = Array.from({ length: 63 }, () =>
			expect(f.edits.collapse('3', true)).rejects.toThrow('owner changed')
		);
		await expect(f.edits.collapse('3', false)).rejects.toThrow('busy');
		f.mutations.dispose();
		reply.resolve(f.result(2));
		await first;
		await Promise.all(queued);
		expect(f.invoke).toHaveBeenCalledTimes(1);
	});
	it('validates committed identities and forwards backend-owned delete/size/name/enable intents', async () => {
		const f = fixture();
		f.invoke.mockResolvedValueOnce(f.result(3));
		await expect(f.edits.rename('2', 'Name')).rejects.toThrow('invalid committed');
		f.invoke.mockResolvedValue(f.result(2));
		await f.edits.rename('2', '  Name  ');
		await f.edits.resize({ nodeId: '2', mode: 'automatic' });
		await f.edits.enable('2', false);
		await f.edits.remove(['2']);
		expect(f.invoke.mock.calls.slice(1).map(([intent]) => intent.payload)).toEqual([
			{ formula: f.id(1), edit: { kind: 'rename', anode: f.id(2), label: '  Name  ' } },
			{ formula: f.id(1), edit: { kind: 'resize', anode: f.id(2), size: null } },
			{ formula: f.id(1), edit: { kind: 'enable', anode: f.id(2), enabled: false } },
			{ formula: f.id(1), edit: { kind: 'delete', nodes: [f.id(2)] } }
		]);
	});
});
