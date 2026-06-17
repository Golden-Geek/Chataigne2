import type { GraphEdge, GraphNode, GraphNodePosition } from 'golden_alchemist_ui';
import type {
	NodeId,
	UiCreateUserItemInitialParam,
	UiCreatableUserItem,
	UiEditIntent,
	UiNodeDto
} from 'golden_ui';
import {
	ANODE_CREATE_PREFIX,
	ANODE_NODE_TYPE,
	CONNECTION_NODE_TYPE,
	anodeType,
	toGraphEdges,
	toGraphNodes
} from './alchemistGraph';

interface GraphBounds {
	left: number;
	top: number;
	right: number;
	bottom: number;
}

export interface AlchemistClipboardNode {
	sourceId: NodeId;
	node_type: string;
	createNodeType: string;
	label: string;
	position: GraphNodePosition;
	size: {
		width: number;
		height: number;
	};
}

export interface AlchemistClipboardEdge {
	sourceNodeId: NodeId;
	sourceSocketId: string;
	targetNodeId: NodeId;
	targetSocketId: string;
}

export interface AlchemistClipboard {
	formulaId: NodeId;
	nodes: AlchemistClipboardNode[];
	edges: AlchemistClipboardEdge[];
}

const GRAPH_NODE_DEFAULT_WIDTH = 13;
const GRAPH_NODE_DEFAULT_HEIGHT = 4.5;
const GRAPH_NODE_MIN_WIDTH = 8;
const GRAPH_DUPLICATE_GAP = 2;
const GRAPH_DUPLICATE_SCAN_RINGS = 12;

const graphNodeSize = (node: GraphNode): { width: number; height: number } => {
	const automaticSize = node.automaticSize;
	const width =
		node.size?.width ??
		(automaticSize && automaticSize.width > 0 ? automaticSize.width : undefined) ??
		GRAPH_NODE_DEFAULT_WIDTH;
	const height =
		node.size?.height ??
		(automaticSize && automaticSize.height > 0 ? automaticSize.height : undefined) ??
		GRAPH_NODE_DEFAULT_HEIGHT;
	return {
		width: Math.max(GRAPH_NODE_MIN_WIDTH, width),
		height: Math.max(1, height)
	};
};

const boundsForClipboardNode = (node: AlchemistClipboardNode): GraphBounds => ({
	left: node.position.x,
	top: node.position.y,
	right: node.position.x + node.size.width,
	bottom: node.position.y + node.size.height
});

const boundsForGraphNode = (node: GraphNode): GraphBounds => {
	const size = graphNodeSize(node);
	return {
		left: node.position.x,
		top: node.position.y,
		right: node.position.x + size.width,
		bottom: node.position.y + size.height
	};
};

const groupBounds = (nodes: readonly AlchemistClipboardNode[]): GraphBounds | null => {
	if (nodes.length === 0) return null;
	const bounds = nodes.map(boundsForClipboardNode);
	return {
		left: Math.min(...bounds.map((rect) => rect.left)),
		top: Math.min(...bounds.map((rect) => rect.top)),
		right: Math.max(...bounds.map((rect) => rect.right)),
		bottom: Math.max(...bounds.map((rect) => rect.bottom))
	};
};

const offsetBounds = (bounds: GraphBounds, offset: GraphNodePosition): GraphBounds => ({
	left: bounds.left + offset.x,
	top: bounds.top + offset.y,
	right: bounds.right + offset.x,
	bottom: bounds.bottom + offset.y
});

const boundsOverlap = (left: GraphBounds, right: GraphBounds, padding: number): boolean =>
	left.left < right.right + padding &&
	left.right > right.left - padding &&
	left.top < right.bottom + padding &&
	left.bottom > right.top - padding;

const clipboardEdgeFromGraphEdge = (
	edge: GraphEdge,
	selectedNodeIds: ReadonlySet<NodeId>
): AlchemistClipboardEdge | null => {
	const sourceNodeId = Number(edge.from.nodeId);
	const targetNodeId = Number(edge.to.nodeId);
	if (
		!Number.isSafeInteger(sourceNodeId) ||
		!Number.isSafeInteger(targetNodeId) ||
		!selectedNodeIds.has(sourceNodeId) ||
		!selectedNodeIds.has(targetNodeId)
	) {
		return null;
	}
	return {
		sourceNodeId,
		sourceSocketId: edge.from.socketId,
		targetNodeId,
		targetSocketId: edge.to.socketId
	};
};

export const buildAlchemistClipboard = ({
	formula,
	nodesById,
	selectedNodeIds,
	anodeNodeIds,
	anodeItems
}: {
	formula: UiNodeDto;
	nodesById: ReadonlyMap<NodeId, UiNodeDto>;
	selectedNodeIds: readonly NodeId[];
	anodeNodeIds: ReadonlySet<NodeId>;
	anodeItems: readonly UiCreatableUserItem[];
}): AlchemistClipboard | null => {
	const selectedSet = new Set(selectedNodeIds.filter((nodeId) => anodeNodeIds.has(nodeId)));
	if (selectedSet.size === 0) return null;

	const graphNodesById = new Map(
		toGraphNodes(formula, nodesById, anodeItems).map((node) => [Number(node.id), node])
	);
	const nodes = formula.children.flatMap((childId): AlchemistClipboardNode[] => {
		if (!selectedSet.has(childId)) return [];
		const anode = nodesById.get(childId);
		const graphNode = graphNodesById.get(childId);
		if (
			anode?.node_type !== ANODE_NODE_TYPE ||
			!graphNode ||
			!anode.meta.user_permissions.can_remove_and_duplicate
		) {
			return [];
		}
		const typeId = anodeType(anode);
		return [
			{
				sourceId: anode.node_id,
				node_type: anode.node_type,
				createNodeType: `${ANODE_CREATE_PREFIX}${typeId}`,
				label: anode.meta.label.trim().length > 0 ? anode.meta.label.trim() : graphNode.label,
				position: { ...graphNode.position },
				size: graphNodeSize(graphNode)
			}
		];
	});
	if (nodes.length === 0) return null;

	const copiedNodeIds = new Set(nodes.map((node) => node.sourceId));
	const edges = toGraphEdges(formula, nodesById)
		.map((edge) => clipboardEdgeFromGraphEdge(edge, copiedNodeIds))
		.filter((edge): edge is AlchemistClipboardEdge => edge !== null);
	return { formulaId: formula.node_id, nodes, edges };
};

export const findEmptyAlchemistDuplicateOffset = ({
	nodes,
	formula,
	nodesById,
	anodeItems,
	viewportCenter,
	preferSourcePosition
}: {
	nodes: readonly AlchemistClipboardNode[];
	formula: UiNodeDto;
	nodesById: ReadonlyMap<NodeId, UiNodeDto>;
	anodeItems: readonly UiCreatableUserItem[];
	viewportCenter: GraphNodePosition | null;
	preferSourcePosition: boolean;
}): GraphNodePosition => {
	const bounds = groupBounds(nodes);
	if (!bounds) return { x: 0, y: 0 };

	const occupied = toGraphNodes(formula, nodesById, anodeItems).map(boundsForGraphNode);
	const width = bounds.right - bounds.left;
	const height = bounds.bottom - bounds.top;
	const stepX = Math.max(
		width + GRAPH_DUPLICATE_GAP,
		GRAPH_NODE_DEFAULT_WIDTH + GRAPH_DUPLICATE_GAP
	);
	const stepY = Math.max(
		height + GRAPH_DUPLICATE_GAP,
		GRAPH_NODE_DEFAULT_HEIGHT + GRAPH_DUPLICATE_GAP
	);
	const center = viewportCenter ?? {
		x: (bounds.left + bounds.right) * 0.5,
		y: (bounds.top + bounds.bottom) * 0.5
	};
	const baseOffset = preferSourcePosition
		? { x: 0, y: 0 }
		: {
				x: center.x - (bounds.left + bounds.right) * 0.5,
				y: center.y - (bounds.top + bounds.bottom) * 0.5
			};

	const isClear = (offset: GraphNodePosition): boolean => {
		const candidate = offsetBounds(bounds, offset);
		return occupied.every((rect) => !boundsOverlap(candidate, rect, GRAPH_DUPLICATE_GAP));
	};
	const withBase = (extra: GraphNodePosition): GraphNodePosition => ({
		x: baseOffset.x + extra.x,
		y: baseOffset.y + extra.y
	});
	const preferred = preferSourcePosition
		? [
				{ x: stepX, y: 0 },
				{ x: 0, y: stepY },
				{ x: stepX, y: stepY },
				{ x: -stepX, y: 0 },
				{ x: 0, y: -stepY }
			]
		: [
				{ x: 0, y: 0 },
				{ x: stepX, y: 0 },
				{ x: 0, y: stepY },
				{ x: stepX, y: stepY }
			];
	for (const extra of preferred) {
		const offset = withBase(extra);
		if (isClear(offset)) return offset;
	}

	for (let ring = 1; ring <= GRAPH_DUPLICATE_SCAN_RINGS; ring++) {
		for (let gridY = -ring; gridY <= ring; gridY++) {
			for (let gridX = -ring; gridX <= ring; gridX++) {
				if (Math.max(Math.abs(gridX), Math.abs(gridY)) !== ring) continue;
				const offset = withBase({ x: gridX * stepX, y: gridY * stepY });
				if (isClear(offset)) return offset;
			}
		}
	}

	return withBase({ x: stepX, y: stepY });
};

export const nextAlchemistCopyLabel = (
	entry: AlchemistClipboardNode,
	usedLabels: Set<string>
): string => {
	const label = entry.label.trim();
	const baseLabel = label.length > 0 ? `${label} Copy` : `${entry.node_type} Copy`;
	if (!usedLabels.has(baseLabel)) {
		usedLabels.add(baseLabel);
		return baseLabel;
	}
	let suffix = 2;
	while (usedLabels.has(`${baseLabel} ${suffix}`)) {
		suffix += 1;
	}
	const next = `${baseLabel} ${suffix}`;
	usedLabels.add(next);
	return next;
};

export const formulaChildLabels = (
	formula: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): Set<string> =>
	new Set(
		formula.children
			.map((childId) => nodesById.get(childId)?.meta.label.trim())
			.filter((label): label is string => label !== undefined && label.length > 0)
	);

export const createCopiedConnectionIntent = (
	edge: AlchemistClipboardEdge,
	createdBySource: ReadonlyMap<NodeId, UiNodeDto>,
	formula: UiNodeDto,
	initialParam: (
		decl_id: string,
		value: UiCreateUserItemInitialParam['value']
	) => UiCreateUserItemInitialParam
): UiEditIntent | null => {
	const source = createdBySource.get(edge.sourceNodeId);
	const target = createdBySource.get(edge.targetNodeId);
	if (!source || !target) return null;
	return {
		kind: 'createUserItem',
		parent: formula.node_id,
		node_type: CONNECTION_NODE_TYPE,
		label: 'Connection',
		initial_params: [
			initialParam('source_node', {
				kind: 'reference',
				uuid: source.uuid,
				cached_id: source.node_id,
				cached_name: source.meta.label,
				relative_path_from_root: []
			}),
			initialParam('source_socket', { kind: 'str', value: edge.sourceSocketId }),
			initialParam('target_node', {
				kind: 'reference',
				uuid: target.uuid,
				cached_id: target.node_id,
				cached_name: target.meta.label,
				relative_path_from_root: []
			}),
			initialParam('target_socket', { kind: 'str', value: edge.targetSocketId })
		]
	};
};
