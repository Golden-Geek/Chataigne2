import type { GraphState } from '../../../store/graph.svelte';
import type { NodeId } from '../../../types';

const EMPTY_NODE_ID_SET: ReadonlySet<NodeId> = new Set<NodeId>();

export const collectOutlinerAncestorNodeIds = (
	graphState: GraphState | null,
	nodeId: NodeId | null
): ReadonlySet<NodeId> => {
	if (graphState === null || nodeId === null || !graphState.nodesById.has(nodeId)) {
		return EMPTY_NODE_ID_SET;
	}

	const ancestorNodeIds = new Set<NodeId>();
	let current = graphState.parentById.get(nodeId);
	while (current !== undefined) {
		ancestorNodeIds.add(current);
		current = graphState.parentById.get(current);
	}
	return ancestorNodeIds;
};

export const scrollOutlinerNodeIntoView = (
	container: ParentNode | null | undefined,
	nodeId: NodeId | null
): boolean => {
	if (!container || nodeId === null) {
		return false;
	}

	const target = container.querySelector<HTMLElement>(
		`.outliner-item-content[data-node-id="${nodeId}"]`
	);
	if (!target) {
		return false;
	}

	target.scrollIntoView({ block: 'center', inline: 'nearest' });
	return true;
};
