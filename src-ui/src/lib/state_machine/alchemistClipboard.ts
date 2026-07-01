import type { GraphEdge, GraphNode, GraphNodePosition } from 'golden_alchemist_ui';
import type { NodeId, UiCreatableUserItem, UiNodeDto } from 'golden_ui';
import {
	ANODE_CREATE_PREFIX,
	ANODE_NODE_TYPE,
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

export interface AlchemistClipboardTreeNode {
	sourceId: NodeId;
	sourceUuid: string;
	node_type: string;
	user_item_kind: UiNodeDto['user_item_kind'] | null;
	decl_id: string;
	label: string;
	data: UiNodeDto['data'];
	meta: {
		label: string;
		enabled: boolean;
		can_be_disabled: boolean;
		presentation: UiNodeDto['meta']['presentation'];
	};
	children: AlchemistClipboardTreeNode[];
}

export interface AlchemistClipboardNode {
	sourceId: NodeId;
	sourceUuid?: string;
	node_type: string;
	createNodeType: string;
	label: string;
	position: GraphNodePosition;
	size: {
		width: number;
		height: number;
	};
	tree?: AlchemistClipboardTreeNode;
}

export interface AlchemistClipboardEdge {
	sourceNodeId: NodeId;
	sourceSocketId: string;
	targetNodeId: NodeId;
	targetSocketId: string;
}

export interface AlchemistClipboard {
	formulaId: NodeId;
	formulaUuid?: string;
	nodes: AlchemistClipboardNode[];
	edges: AlchemistClipboardEdge[];
}

export interface AlchemistClipboardPayload {
	kind: 'chataigne.alchemist.nodes';
	version: 2;
	clipboard: AlchemistClipboard;
}

const ALCHEMIST_CLIPBOARD_KIND = 'chataigne.alchemist.nodes';
const ALCHEMIST_CLIPBOARD_VERSION = 2;
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

const clipboardTreeFromNode = (
	node: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): AlchemistClipboardTreeNode => ({
	sourceId: node.node_id,
	sourceUuid: node.uuid,
	node_type: node.node_type,
	user_item_kind: node.user_item_kind,
	decl_id: node.decl_id,
	label: node.meta.label,
	data: node.data,
	meta: {
		label: node.meta.label,
		enabled: node.meta.enabled,
		can_be_disabled: node.meta.can_be_disabled,
		presentation: node.meta.presentation
	},
	children: node.children.flatMap((childId): AlchemistClipboardTreeNode[] => {
		const child = nodesById.get(childId);
		return child ? [clipboardTreeFromNode(child, nodesById)] : [];
	})
});

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
				sourceUuid: anode.uuid,
				node_type: anode.node_type,
				createNodeType: `${ANODE_CREATE_PREFIX}${typeId}`,
				label: anode.meta.label.trim().length > 0 ? anode.meta.label.trim() : graphNode.label,
				position: { ...graphNode.position },
				size: graphNodeSize(graphNode),
				tree: clipboardTreeFromNode(anode, nodesById)
			}
		];
	});
	if (nodes.length === 0) return null;

	const copiedNodeIds = new Set(nodes.map((node) => node.sourceId));
	const edges = toGraphEdges(formula, nodesById)
		.map((edge) => clipboardEdgeFromGraphEdge(edge, copiedNodeIds))
		.filter((edge): edge is AlchemistClipboardEdge => edge !== null);
	return { formulaId: formula.node_id, formulaUuid: formula.uuid, nodes, edges };
};

const isRecord = (candidate: unknown): candidate is Record<string, unknown> =>
	typeof candidate === 'object' && candidate !== null;

const numberField = (record: Record<string, unknown>, field: string): number | null => {
	const value = record[field];
	return typeof value === 'number' && Number.isSafeInteger(value) ? value : null;
};

const stringField = (record: Record<string, unknown>, field: string): string | null => {
	const value = record[field];
	return typeof value === 'string' ? value : null;
};

const positionFromJson = (candidate: unknown): GraphNodePosition | null => {
	if (!isRecord(candidate)) return null;
	const { x, y } = candidate;
	return typeof x === 'number' && typeof y === 'number' && Number.isFinite(x) && Number.isFinite(y)
		? { x, y }
		: null;
};

const sizeFromJson = (candidate: unknown): { width: number; height: number } | null => {
	if (!isRecord(candidate)) return null;
	const { width, height } = candidate;
	return typeof width === 'number' &&
		typeof height === 'number' &&
		Number.isFinite(width) &&
		Number.isFinite(height)
		? { width, height }
		: null;
};

const treeFromJson = (candidate: unknown): AlchemistClipboardTreeNode | undefined => {
	if (!isRecord(candidate)) return undefined;
	const sourceId = numberField(candidate, 'sourceId');
	const sourceUuid = stringField(candidate, 'sourceUuid');
	const nodeType = stringField(candidate, 'node_type');
	const declId = stringField(candidate, 'decl_id');
	const label = stringField(candidate, 'label');
	const data = candidate.data;
	const meta = candidate.meta;
	const children = candidate.children;
	if (
		sourceId === null ||
		sourceUuid === null ||
		nodeType === null ||
		declId === null ||
		label === null ||
		!isRecord(data) ||
		!isRecord(meta) ||
		!Array.isArray(children)
	) {
		return undefined;
	}
	return {
		sourceId,
		sourceUuid,
		node_type: nodeType,
		user_item_kind: typeof candidate.user_item_kind === 'string' ? candidate.user_item_kind : null,
		decl_id: declId,
		label,
		data: data as UiNodeDto['data'],
		meta: {
			label: typeof meta.label === 'string' ? meta.label : label,
			enabled: typeof meta.enabled === 'boolean' ? meta.enabled : true,
			can_be_disabled: typeof meta.can_be_disabled === 'boolean' ? meta.can_be_disabled : false,
			presentation: isRecord(meta.presentation)
				? (meta.presentation as UiNodeDto['meta']['presentation'])
				: undefined
		},
		children: children.flatMap((child): AlchemistClipboardTreeNode[] => {
			const tree = treeFromJson(child);
			return tree ? [tree] : [];
		})
	};
};

const clipboardNodeFromJson = (candidate: unknown): AlchemistClipboardNode | null => {
	if (!isRecord(candidate)) return null;
	const sourceId = numberField(candidate, 'sourceId');
	const nodeType = stringField(candidate, 'node_type');
	const createNodeType = stringField(candidate, 'createNodeType');
	const label = stringField(candidate, 'label');
	const position = positionFromJson(candidate.position);
	const size = sizeFromJson(candidate.size);
	if (
		sourceId === null ||
		nodeType === null ||
		createNodeType === null ||
		label === null ||
		position === null ||
		size === null
	) {
		return null;
	}
	return {
		sourceId,
		sourceUuid: stringField(candidate, 'sourceUuid') ?? undefined,
		node_type: nodeType,
		createNodeType,
		label,
		position,
		size,
		tree: treeFromJson(candidate.tree)
	};
};

const clipboardEdgeFromJson = (candidate: unknown): AlchemistClipboardEdge | null => {
	if (!isRecord(candidate)) return null;
	const sourceNodeId = numberField(candidate, 'sourceNodeId');
	const targetNodeId = numberField(candidate, 'targetNodeId');
	const sourceSocketId = stringField(candidate, 'sourceSocketId');
	const targetSocketId = stringField(candidate, 'targetSocketId');
	return sourceNodeId !== null &&
		targetNodeId !== null &&
		sourceSocketId !== null &&
		targetSocketId !== null
		? { sourceNodeId, sourceSocketId, targetNodeId, targetSocketId }
		: null;
};

export const alchemistClipboardJson = (clipboard: AlchemistClipboard): string => {
	const payload: AlchemistClipboardPayload = {
		kind: ALCHEMIST_CLIPBOARD_KIND,
		version: ALCHEMIST_CLIPBOARD_VERSION,
		clipboard
	};
	return JSON.stringify(payload, null, 2);
};

export const alchemistClipboardFromJson = (text: string): AlchemistClipboard | null => {
	try {
		const payload: unknown = JSON.parse(text);
		if (!isRecord(payload) || payload.kind !== ALCHEMIST_CLIPBOARD_KIND) return null;
		if (payload.version !== 1 && payload.version !== ALCHEMIST_CLIPBOARD_VERSION) return null;
		const clipboard = payload.clipboard;
		if (
			!isRecord(clipboard) ||
			!Array.isArray(clipboard.nodes) ||
			!Array.isArray(clipboard.edges)
		) {
			return null;
		}
		const formulaId = numberField(clipboard, 'formulaId');
		if (formulaId === null) return null;
		const nodes = clipboard.nodes
			.map(clipboardNodeFromJson)
			.filter((node): node is AlchemistClipboardNode => node !== null);
		if (nodes.length === 0) return null;
		return {
			formulaId,
			formulaUuid: stringField(clipboard, 'formulaUuid') ?? undefined,
			nodes,
			edges: clipboard.edges
				.map(clipboardEdgeFromJson)
				.filter((edge): edge is AlchemistClipboardEdge => edge !== null)
		};
	} catch {
		return null;
	}
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
