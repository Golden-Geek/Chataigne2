import type { GraphConnectionRequest, GraphNodeMove, GraphNodeResize } from 'golden_graph_ui';
import type { UiNodeDto, WorkbenchSession } from 'golden_ui';
import type { StateGraphEdit } from './generated/StateGraphEdit';
import type { StateGraphIdentity } from './generated/StateGraphIdentity';
import {
	STATE_GRAPH_NAMESPACE,
	STATE_GRAPH_EDIT_OPERATION
} from './generated/state-graph-constants';

export type StateGraphEditSession = Pick<
	WorkbenchSession,
	'graph' | 'snapshotGeneration' | 'editOperations' | 'invokeAppOperation'
>;
interface Source {
	readonly session: StateGraphEditSession | null;
	readonly manager: UiNodeDto | null;
}
const record = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);
const identity = (value: unknown): value is StateGraphIdentity =>
	record(value) &&
	typeof value.node === 'number' &&
	Number.isSafeInteger(value.node) &&
	value.node >= 0 &&
	typeof value.uuid === 'string' &&
	value.uuid.length > 0;

/** Captures authored identities before admission; domain policy stays in the backend operation. */
export function createStateGraphEdits(source: () => Source) {
	let disposed = false;
	const cancellations = new Set<() => void>();
	const capture = () => {
		const current = source();
		if (disposed || !current.session || !current.manager)
			throw new Error('State graph editing requires a current manager and session.');
		return {
			session: current.session,
			generation: current.session.snapshotGeneration,
			manager: { node: current.manager.node_id, uuid: current.manager.uuid }
		};
	};
	type Owner = ReturnType<typeof capture>;
	const current = (owner: Owner) => {
		const next = source();
		return (
			!disposed &&
			next.session === owner.session &&
			owner.session.snapshotGeneration === owner.generation &&
			next.manager?.node_id === owner.manager.node &&
			next.manager.uuid === owner.manager.uuid
		);
	};
	const node = (owner: Owner, id: string): StateGraphIdentity => {
		const parsed = Number(id);
		const value =
			Number.isSafeInteger(parsed) && parsed >= 0
				? owner.session.graph.state.nodesById.get(parsed)
				: undefined;
		if (!value) throw new Error('State graph target is unavailable.');
		return { node: value.node_id, uuid: value.uuid };
	};
	const apply = async (owner: Owner, edit: StateGraphEdit): Promise<void> => {
		const expected =
			edit.kind === 'move'
				? edit.moves.map((move) => move.state)
				: edit.kind === 'connect'
					? [edit.from.state, edit.to.state]
					: [edit.state];
		const operation = owner.session.editOperations.open({
			current: () => current(owner),
			build: (scope) => {
				let cancelled = false;
				return {
					accepted: true,
					async finish() {
						await scope.ready;
						if (cancelled || !scope.current)
							throw new Error('State graph owner changed before dispatch.');
						const result = await owner.session.invokeAppOperation({
							kind: 'invokeAppOperation',
							namespace: STATE_GRAPH_NAMESPACE,
							operation: STATE_GRAPH_EDIT_OPERATION,
							payload: { manager: owner.manager, edit }
						});
						if (
							!record(result) ||
							!Array.isArray(result.states) ||
							!result.states.every(identity) ||
							(result.connection !== null && !identity(result.connection)) ||
							(edit.kind === 'connect' ? result.connection === null : result.connection !== null)
						)
							throw new Error('State graph edit returned an invalid committed result.');
						if (
							result.states.length !== expected.length ||
							result.states.some(
								(state, index) =>
									!identity(state) ||
									state.node !== expected[index].node ||
									state.uuid !== expected[index].uuid
							)
						)
							throw new Error('State graph edit returned different target identities.');
					},
					async cancel() {
						cancelled = true;
					}
				};
			}
		});
		if (!operation) throw new Error('State graph editing is busy or crossing a project barrier.');
		const cancel = () => {
			void operation.cancel().catch(() => undefined);
		};
		cancellations.add(cancel);
		try {
			await operation.confirm();
		} finally {
			cancellations.delete(cancel);
		}
	};
	return {
		async move(moves: GraphNodeMove[]): Promise<void> {
			const owner = capture();
			return apply(owner, {
				kind: 'move',
				moves: moves.map((move) => ({
					state: node(owner, move.nodeId),
					position: [move.position.x, move.position.y]
				}))
			});
		},
		async resize(resize: GraphNodeResize): Promise<void> {
			const owner = capture();
			return apply(owner, {
				kind: 'resize',
				state: node(owner, resize.nodeId),
				size: resize.mode === 'custom' ? [resize.size.width, resize.size.height] : null
			});
		},
		async rename(id: string, label: string): Promise<void> {
			const owner = capture();
			return apply(owner, { kind: 'rename', state: node(owner, id), label });
		},
		async collapse(id: string, collapsed: boolean): Promise<void> {
			const owner = capture();
			return apply(owner, { kind: 'collapse', state: node(owner, id), collapsed });
		},
		async enable(id: string, enabled: boolean): Promise<void> {
			const owner = capture();
			return apply(owner, { kind: 'enable', state: node(owner, id), enabled });
		},
		async connect(connection: GraphConnectionRequest): Promise<void> {
			const owner = capture();
			return apply(owner, {
				kind: 'connect',
				from: { state: node(owner, connection.from.nodeId), socket: connection.from.socketId },
				to: { state: node(owner, connection.to.nodeId), socket: connection.to.socketId }
			});
		},
		dispose() {
			disposed = true;
			for (const cancel of cancellations) cancel();
		}
	};
}
