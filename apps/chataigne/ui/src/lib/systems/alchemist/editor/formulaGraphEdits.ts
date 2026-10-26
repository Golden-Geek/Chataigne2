import type { GraphConnectionRequest, GraphNodeMove, GraphNodeResize } from 'golden_graph_ui';
import type { FormulaOwnerDto } from '../generated/authoring/FormulaOwnerDto';
import type { FormulaGraphEdit } from '../generated/authoring/FormulaGraphEdit';
import { connectFormulaNodes, editFormulaGraph } from '../formulaOperations';
import type { FormulaMutations } from './formulaMutations';

export function createFormulaGraphEdits(mutations: FormulaMutations) {
	const capture = () => {
		const owner = mutations.capture();
		if (!owner.formula) throw new Error('Formula graph is unavailable.');
		const formula: FormulaOwnerDto = { node: owner.formula.node_id, uuid: owner.formula.uuid };
		const node = (id: string): FormulaOwnerDto => {
			const parsed = Number(id);
			const target = Number.isSafeInteger(parsed) ? owner.graph.nodesById.get(parsed) : undefined;
			if (!target) throw new Error('Formula graph target is unavailable.');
			return { node: target.node_id, uuid: target.uuid };
		};
		return { owner, formula, node };
	};
	const apply = (target: ReturnType<typeof capture>, edit: FormulaGraphEdit) =>
		mutations.run(async () => {
			if (!target.owner.current()) throw new Error('Formula owner changed before dispatch.');
			await editFormulaGraph(target.owner.session, { formula: target.formula, edit });
		});
	return {
		async remove(ids: readonly string[]) {
			const target = capture();
			return apply(target, { kind: 'delete', nodes: ids.map(target.node) });
		},
		async move(moves: GraphNodeMove[]) {
			const target = capture();
			return apply(target, {
				kind: 'move',
				moves: moves.map((move) => ({
					anode: target.node(move.nodeId),
					position: [move.position.x, move.position.y]
				}))
			});
		},
		async resize(resize: GraphNodeResize) {
			const target = capture();
			return apply(target, {
				kind: 'resize',
				anode: target.node(resize.nodeId),
				size: resize.mode === 'custom' ? [resize.size.width, resize.size.height] : null
			});
		},
		async rename(id: string, label: string) {
			const target = capture();
			return apply(target, { kind: 'rename', anode: target.node(id), label });
		},
		async collapse(id: string, collapsed: boolean) {
			const target = capture();
			return apply(target, { kind: 'collapse', anode: target.node(id), collapsed });
		},
		async enable(id: string, enabled: boolean) {
			const target = capture();
			return apply(target, { kind: 'enable', anode: target.node(id), enabled });
		},
		async connect(connection: GraphConnectionRequest) {
			const target = capture();
			const from = { ...target.node(connection.from.nodeId), socket: connection.from.socketId };
			const to = { ...target.node(connection.to.nodeId), socket: connection.to.socketId };
			return mutations.run(async () => {
				if (!target.owner.current()) throw new Error('Formula owner changed before dispatch.');
				await connectFormulaNodes(target.owner.session, { formula: target.formula, from, to });
			});
		}
	};
}
