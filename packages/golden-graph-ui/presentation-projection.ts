import { SpatialIndex, type SpatialBounds } from './spatial-index';
import type { GraphCamera, GraphEdge, GraphNode, GraphNodePosition, GraphNodeSize } from './types';

export interface GraphNodePresentationChanges {
	dragPositions: Record<string, GraphNodePosition>;
	optimisticPositions: Record<string, GraphNodePosition>;
	resizeSizes: Record<string, GraphNodeSize>;
	optimisticSizes: Record<string, GraphNodeSize | null>;
	optimisticLabels: Record<string, string>;
	optimisticCollapsed: Record<string, boolean>;
	optimisticEnabled: Record<string, boolean>;
}

export const projectEffectiveNodes = (
	nodes: readonly GraphNode[],
	changes: GraphNodePresentationChanges
): GraphNode[] =>
	nodes.map((node) => {
		const position = changes.dragPositions[node.id] ?? changes.optimisticPositions[node.id];
		const size =
			node.id in changes.resizeSizes
				? changes.resizeSizes[node.id]
				: changes.optimisticSizes[node.id];
		const label = changes.optimisticLabels[node.id];
		const collapsed = changes.optimisticCollapsed[node.id];
		const enabled = changes.optimisticEnabled[node.id];
		return position !== undefined ||
			size !== undefined ||
			label !== undefined ||
			collapsed !== undefined ||
			enabled !== undefined
			? {
					...node,
					...(position !== undefined ? { position } : {}),
					...(size !== undefined ? { size: size ?? undefined } : {}),
					...(label !== undefined ? { label } : {}),
					...(collapsed !== undefined ? { collapsed } : {}),
					...(enabled !== undefined ? { enabled } : {})
				}
			: node;
	});

export const buildGraphNodeSpatialIndex = (
	nodes: readonly GraphNode[],
	nodeWidth: (node: GraphNode) => number,
	nodeHeight: (node: GraphNode) => number
): SpatialIndex<GraphNode> => {
	const index = new SpatialIndex<GraphNode>(32);
	for (const node of nodes) {
		index.insert(
			node.id,
			{
				left: node.position.x,
				top: node.position.y,
				right: node.position.x + nodeWidth(node),
				bottom: node.position.y + nodeHeight(node)
			},
			node
		);
	}
	return index;
};

export const viewportWorldBounds = (
	camera: GraphCamera,
	viewportWidth: number,
	viewportHeight: number,
	remPx: number,
	margin: number
): SpatialBounds => ({
	left: -camera.x / camera.zoom / remPx - margin,
	top: -camera.y / camera.zoom / remPx - margin,
	right: (viewportWidth - camera.x) / camera.zoom / remPx + margin,
	bottom: (viewportHeight - camera.y) / camera.zoom / remPx + margin
});

export const indexGraphEdgesByNode = (edges: readonly GraphEdge[]): Map<string, number[]> => {
	const indexes = new Map<string, number[]>();
	const append = (nodeId: string, index: number): void => {
		const nodeIndexes = indexes.get(nodeId);
		if (nodeIndexes) {
			nodeIndexes.push(index);
		} else {
			indexes.set(nodeId, [index]);
		}
	};
	for (let index = 0; index < edges.length; index += 1) {
		const edge = edges[index];
		append(edge.from.nodeId, index);
		if (edge.to.nodeId !== edge.from.nodeId) {
			append(edge.to.nodeId, index);
		}
	}
	return indexes;
};

export const projectVisibleEdges = (
	edges: readonly GraphEdge[],
	indexesByNode: Map<string, number[]>,
	visibleNodeIds: ReadonlySet<string>
): GraphEdge[] => {
	const indexes = new Set<number>();
	for (const nodeId of visibleNodeIds) {
		for (const index of indexesByNode.get(nodeId) ?? []) {
			indexes.add(index);
		}
	}
	return [...indexes].sort((left, right) => left - right).map((index) => edges[index]);
};
