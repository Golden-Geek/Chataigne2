import type { GraphState } from '../../../store/graph.svelte';
import type { NodeId, UiNodeDto } from '../../../types';

export type OutlinerDropZone = 'before' | 'inside' | 'after';

export interface OutlinerDropTarget {
	hoverNodeId: NodeId;
	zone: OutlinerDropZone;
	newParentId: NodeId;
	newPrevSiblingId: NodeId | null;
}

const getNode = (graphState: GraphState | null, nodeId: NodeId): UiNodeDto | null =>
	graphState?.nodesById.get(nodeId) ?? null;

const getChildIds = (
	graphState: GraphState | null,
	parentId: NodeId,
	excludedNodeId?: NodeId
): NodeId[] => {
	const parent = getNode(graphState, parentId);
	if (!parent) {
		return [];
	}
	return parent.children.filter((childId) => childId !== excludedNodeId);
};

const isDescendantOf = (
	graphState: GraphState | null,
	nodeId: NodeId,
	ancestorNodeId: NodeId
): boolean => {
	let current: NodeId | undefined = nodeId;
	while (current !== undefined) {
		if (current === ancestorNodeId) {
			return true;
		}
		current = graphState?.parentById.get(current);
	}
	return false;
};

const currentParentIdForNode = (graphState: GraphState | null, nodeId: NodeId): NodeId | null => {
	const parentId = graphState?.parentById.get(nodeId);
	return parentId === undefined ? null : parentId;
};

const currentPrevSiblingIdForNode = (
	graphState: GraphState | null,
	nodeId: NodeId
): NodeId | null => {
	const parentId = currentParentIdForNode(graphState, nodeId);
	if (parentId === null) {
		return null;
	}
	const siblings = getChildIds(graphState, parentId);
	const nodeIndex = siblings.findIndex((candidateId) => candidateId === nodeId);
	if (nodeIndex <= 0) {
		return null;
	}
	return siblings[nodeIndex - 1] ?? null;
};

interface MovedItemRootDescriptor {
	nodeId: NodeId;
	nodeType: string;
	itemKind: string;
}

interface MovedSubtreeDescriptor {
	nodeIds: Set<NodeId>;
	itemRoots: MovedItemRootDescriptor[];
}

const collectMovedSubtree = (
	graphState: GraphState | null,
	rootNodeId: NodeId
): MovedSubtreeDescriptor => {
	const rootNode = getNode(graphState, rootNodeId);
	if (!graphState || !rootNode) {
		return {
			nodeIds: new Set<NodeId>(),
			itemRoots: []
		};
	}

	const nodeIds = new Set<NodeId>();
	const movedItemRoots: MovedItemRootDescriptor[] = [];
	const pendingNodeIds: NodeId[] = [rootNodeId];
	while (pendingNodeIds.length > 0) {
		const nodeId = pendingNodeIds.pop();
		if (nodeId === undefined) {
			continue;
		}
		const node = getNode(graphState, nodeId);
		if (!node) {
			continue;
		}
		nodeIds.add(nodeId);
		if (node.user_role === 'itemRoot') {
			movedItemRoots.push({
				nodeId,
				nodeType: node.node_type,
				itemKind: node.user_item_kind.trim()
			});
		}
		for (const childId of node.children) {
			pendingNodeIds.push(childId);
		}
	}

	return {
		nodeIds,
		itemRoots: movedItemRoots
	};
};

const isContainerNode = (node: UiNodeDto | null): node is UiNodeDto => {
	if (!node) {
		return false;
	}
	return node.accepted_user_item_kinds.length > 0 || node.creatable_user_items.length > 0;
};

const nearestContainerAncestor = (
	graphState: GraphState | null,
	startNodeId: NodeId
): NodeId | null => {
	let current: NodeId | undefined = startNodeId;
	while (current !== undefined) {
		const node = getNode(graphState, current);
		if (!node) {
			return null;
		}
		if (isContainerNode(node)) {
			return current;
		}
		current = graphState?.parentById.get(current);
	}
	return null;
};

const nearestContainerAncestorOfParent = (
	graphState: GraphState | null,
	nodeId: NodeId
): NodeId | null => {
	const parentId = currentParentIdForNode(graphState, nodeId);
	return parentId === null ? null : nearestContainerAncestor(graphState, parentId);
};

const containerAcceptsMovedRoot = (
	container: UiNodeDto,
	movedRoot: MovedItemRootDescriptor
): boolean => {
	if (
		container.accepted_user_item_kinds.some(
			(acceptedKind) => acceptedKind === '*' || acceptedKind === movedRoot.itemKind
		)
	) {
		return true;
	}

	return container.creatable_user_items.some(
		(candidate) =>
			candidate.node_type === movedRoot.nodeType || candidate.item_kind === movedRoot.itemKind
	);
};

const parentAcceptsMovedNode = (
	graphState: GraphState | null,
	sourceNodeId: NodeId,
	newParentId: NodeId
): boolean => {
	const movedSubtree = collectMovedSubtree(graphState, sourceNodeId);
	if (movedSubtree.itemRoots.length === 0) {
		return false;
	}

	const targetContainer = getNode(graphState, newParentId);
	if (!isContainerNode(targetContainer)) {
		return false;
	}

	return movedSubtree.itemRoots.every((movedRoot) => {
		const oldContainerId = nearestContainerAncestorOfParent(graphState, movedRoot.nodeId);
		const containerChanges = oldContainerId === null || !movedSubtree.nodeIds.has(oldContainerId);
		if (!containerChanges) {
			return true;
		}

		return containerAcceptsMovedRoot(targetContainer, movedRoot);
	});
};

const prevSiblingIdBeforeNode = (
	graphState: GraphState | null,
	parentId: NodeId,
	targetNodeId: NodeId,
	excludedNodeId: NodeId
): NodeId | null => {
	const siblingIds = getChildIds(graphState, parentId, excludedNodeId);
	const targetIndex = siblingIds.findIndex((candidateId) => candidateId === targetNodeId);
	if (targetIndex < 0) {
		return null;
	}
	return targetIndex === 0 ? null : (siblingIds[targetIndex - 1] ?? null);
};

const prevSiblingIdAfterNode = (
	graphState: GraphState | null,
	parentId: NodeId,
	targetNodeId: NodeId,
	excludedNodeId: NodeId
): NodeId | null => {
	const siblingIds = getChildIds(graphState, parentId, excludedNodeId);
	const targetIndex = siblingIds.findIndex((candidateId) => candidateId === targetNodeId);
	if (targetIndex < 0) {
		return null;
	}
	return siblingIds[targetIndex] ?? null;
};

const lastChildIdForParent = (
	graphState: GraphState | null,
	parentId: NodeId,
	excludedNodeId: NodeId
): NodeId | null => {
	const siblingIds = getChildIds(graphState, parentId, excludedNodeId);
	return siblingIds.at(-1) ?? null;
};

export const canDragOutlinerNode = (
	graphState: GraphState | null,
	node: UiNodeDto | null
): boolean => {
	if (!graphState || !node) {
		return false;
	}
	if (graphState.rootId === node.node_id) {
		return false;
	}
	return node.user_role === 'itemRoot' && node.meta.user_permissions.can_remove_and_duplicate;
};

export const resolveOutlinerDropTarget = (
	graphState: GraphState | null,
	sourceNodeId: NodeId,
	hoverNodeId: NodeId,
	zone: OutlinerDropZone
): OutlinerDropTarget | null => {
	const sourceNode = getNode(graphState, sourceNodeId);
	const hoverNode = getNode(graphState, hoverNodeId);
	if (!graphState || !sourceNode || !hoverNode) {
		return null;
	}
	if (!canDragOutlinerNode(graphState, sourceNode)) {
		return null;
	}

	const effectiveZone =
		graphState.rootId !== null && hoverNodeId === graphState.rootId ? 'inside' : zone;

	let newParentId: NodeId;
	let newPrevSiblingId: NodeId | null;

	if (effectiveZone === 'inside') {
		if (hoverNodeId === sourceNodeId) {
			return null;
		}
		newParentId = hoverNodeId;
		if (!parentAcceptsMovedNode(graphState, sourceNodeId, newParentId)) {
			return null;
		}
		newPrevSiblingId = lastChildIdForParent(graphState, newParentId, sourceNodeId);
	} else {
		if (hoverNodeId === sourceNodeId) {
			return null;
		}
		const parentId = currentParentIdForNode(graphState, hoverNodeId);
		if (parentId === null) {
			return null;
		}
		newParentId = parentId;
		if (!parentAcceptsMovedNode(graphState, sourceNodeId, newParentId)) {
			return null;
		}
		newPrevSiblingId =
			effectiveZone === 'before'
				? prevSiblingIdBeforeNode(graphState, newParentId, hoverNodeId, sourceNodeId)
				: prevSiblingIdAfterNode(graphState, newParentId, hoverNodeId, sourceNodeId);
	}

	if (newParentId === sourceNodeId || isDescendantOf(graphState, newParentId, sourceNodeId)) {
		return null;
	}

	if (
		currentParentIdForNode(graphState, sourceNodeId) === newParentId &&
		currentPrevSiblingIdForNode(graphState, sourceNodeId) === newPrevSiblingId
	) {
		return null;
	}

	return {
		hoverNodeId,
		zone: effectiveZone,
		newParentId,
		newPrevSiblingId
	};
};
