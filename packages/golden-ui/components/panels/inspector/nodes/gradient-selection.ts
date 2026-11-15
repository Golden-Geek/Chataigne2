import {
	createHierarchySelection,
	selectionPath,
	type SelectionGraph
} from '../../../../store/selection-hierarchy';
import type { NodeId, UiNodeDto } from '../../../../types';

export interface GradientSelection {
	stop: NodeId | null;
}

export function createGradientSelectionProjection() {
	let owner: NodeId;
	const projection = createHierarchySelection((hierarchy, ids): GradientSelection | null => {
		let relevant = false;
		let stop: NodeId | null = null;
		for (const id of ids) {
			const path = selectionPath(hierarchy, id);
			if (!path?.some((node) => node.node_id === owner)) continue;
			relevant = true;
			stop ??= path.find((node) => node.node_type === 'gradient_stop')?.node_id ?? null;
		}
		return relevant ? { stop } : null;
	});
	return (graph: SelectionGraph | null, ids: readonly NodeId[], node: UiNodeDto) => {
		owner = node.node_id;
		return projection.read(graph, ids, node.uuid);
	};
}

export function gradientSelectionNodes(
	graph: SelectionGraph,
	owner: NodeId,
	selection: GradientSelection
): NodeId[] | null {
	if (selection.stop === null) return [owner];
	return graph.parentById.get(selection.stop) === owner &&
		graph.nodesById.get(selection.stop)?.node_type === 'gradient_stop'
		? [selection.stop]
		: null;
}
