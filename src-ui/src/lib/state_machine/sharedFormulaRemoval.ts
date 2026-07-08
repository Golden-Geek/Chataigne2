import {
	registerNodeRemovalGuard,
	requestConfirmation,
	type NodeRemovalRequest,
	type UiNodeDto
} from 'golden_ui';
import { formulaSourceKind } from './formulaSource';

const FORMULA_NODE_TYPE = 'alchemist_formula';
const FORMULA_EXTERNAL_DELETE_FILE_DECL_ID = 'external_formula_delete_file';

let registered = false;

const directChildByDeclId = (
	request: NodeRemovalRequest,
	node: UiNodeDto,
	declId: string
): UiNodeDto | null => {
	for (const childId of node.children) {
		const child = request.graph.nodesById.get(childId);
		if (child?.decl_id === declId) return child;
	}
	return null;
};

const collectSharedFormulas = (
	request: NodeRemovalRequest,
	node: UiNodeDto,
	collected: UiNodeDto[],
	seen: Set<number>
): void => {
	if (seen.has(node.node_id)) return;
	seen.add(node.node_id);
	if (node.node_type === FORMULA_NODE_TYPE && formulaSourceKind(node, request.graph.nodesById) === 'shared') {
		collected.push(node);
	}
	for (const childId of node.children) {
		const child = request.graph.nodesById.get(childId);
		if (child) collectSharedFormulas(request, child, collected, seen);
	}
};

const prepareSharedFormulaFileRemoval = async (
	request: NodeRemovalRequest,
	formulas: UiNodeDto[]
): Promise<boolean> => {
	for (const formula of formulas) {
		const deleteParam = directChildByDeclId(request, formula, FORMULA_EXTERNAL_DELETE_FILE_DECL_ID);
		if (!deleteParam || deleteParam.data.kind !== 'parameter') {
			console.error('shared formula is missing its backend delete marker', formula);
			return false;
		}
		await request.sendIntent({
			kind: 'setParam',
			node: deleteParam.node_id,
			value: { kind: 'bool', value: true },
			behaviour: deleteParam.data.param.event_behaviour
		});
	}
	return true;
};

export const registerSharedFormulaRemovalGuard = (): void => {
	if (registered) return;
	registered = true;
	registerNodeRemovalGuard(async (request) => {
		const sharedFormulas: UiNodeDto[] = [];
		const seen = new Set<number>();
		for (const node of request.nodes) {
			collectSharedFormulas(request, node, sharedFormulas, seen);
		}
		if (sharedFormulas.length === 0) return true;

		const confirmed = await requestConfirmation({
			title:
				sharedFormulas.length === 1 ? 'Remove shared formula?' : 'Remove shared formulas?',
			message:
				sharedFormulas.length === 1
					? 'this will remove the shared formula in the explorer'
					: 'this will remove the shared formulas in the explorer',
			actions: [
				{
					id: 'remove',
					label: 'Remove',
					shortcut: 'Enter',
					tone: 'danger',
					defaultFocus: true
				}
			]
		});
		if (confirmed !== 'remove') return false;
		return prepareSharedFormulaFileRemoval(request, sharedFormulas);
	});
};
