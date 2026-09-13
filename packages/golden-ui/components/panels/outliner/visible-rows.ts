import type { NodeId, UiNodeDto } from '../../../types';

export interface OutlinerRow {
	nodeId: NodeId;
	parentId: NodeId | null;
	level: number;
}

export interface OutlinerRowGraph {
	rootId: NodeId | null;
	nodesById: ReadonlyMap<NodeId, UiNodeDto>;
}

interface VisibleRowsOptions {
	graph: OutlinerRowGraph;
	opennessByNodeId: Readonly<Record<string, boolean>>;
	autoExpandAncestorNodeIds: ReadonlySet<NodeId>;
	initiallyExpandedDepth: number;
	matchesQuery?: (node: UiNodeDto) => boolean;
}

const matchingSubtrees = (
	graph: OutlinerRowGraph,
	matchesQuery: (node: UiNodeDto) => boolean
): ReadonlySet<NodeId> => {
	const matches = new Set<NodeId>();
	const visited = new Set<NodeId>();
	const pending: Array<{ nodeId: NodeId; postorder: boolean }> = [];
	if (graph.rootId !== null) pending.push({ nodeId: graph.rootId, postorder: false });

	while (pending.length > 0) {
		const { nodeId, postorder } = pending.pop()!;
		const node = graph.nodesById.get(nodeId);
		if (!node) continue;
		if (postorder) {
			if (matchesQuery(node) || node.children.some((child) => matches.has(child))) {
				matches.add(nodeId);
			}
			continue;
		}
		if (visited.has(nodeId)) continue;
		visited.add(nodeId);
		pending.push({ nodeId, postorder: true });
		for (const child of node.children) pending.push({ nodeId: child, postorder: false });
	}
	return matches;
};

/** Project expanded graph paths without mounting a component for every visible node. */
export const collectVisibleOutlinerRows = ({
	graph,
	opennessByNodeId,
	autoExpandAncestorNodeIds,
	initiallyExpandedDepth,
	matchesQuery
}: VisibleRowsOptions): OutlinerRow[] => {
	if (graph.rootId === null) return [];
	const matches = matchesQuery ? matchingSubtrees(graph, matchesQuery) : null;
	const rows: OutlinerRow[] = [];
	const visited = new Set<NodeId>();
	const pending: OutlinerRow[] = [{ nodeId: graph.rootId, parentId: null, level: 0 }];
	while (pending.length > 0) {
		const row = pending.pop()!;
		if (visited.has(row.nodeId) || (matches !== null && !matches.has(row.nodeId))) continue;
		visited.add(row.nodeId);
		const node = graph.nodesById.get(row.nodeId);
		if (!node) continue;
		rows.push(row);
		const explicit = opennessByNodeId[String(row.nodeId)];
		const expanded =
			autoExpandAncestorNodeIds.has(row.nodeId) ||
			(typeof explicit === 'boolean'
				? explicit
				: node.meta.presentation?.collapsed !== true && row.level < initiallyExpandedDepth);
		if (!expanded) continue;
		for (let index = node.children.length - 1; index >= 0; index -= 1) {
			pending.push({ nodeId: node.children[index], parentId: row.nodeId, level: row.level + 1 });
		}
	}
	return rows;
};

export interface OutlinerWindow {
	start: number;
	end: number;
}

/** Geometry is measured in CSS pixels because scrollTop/clientHeight are pixel-valued DOM APIs. */
export const outlinerWindow = (
	rowCount: number,
	scrollTop: number,
	viewportHeight: number,
	rowHeight: number,
	overscan = 8
): OutlinerWindow => {
	if (rowCount === 0) return { start: 0, end: 0 };
	const height = Math.max(1, rowHeight);
	const start = Math.min(
		rowCount - 1,
		Math.max(0, Math.floor(Math.max(0, scrollTop) / height) - overscan)
	);
	const end = Math.min(
		rowCount,
		Math.ceil((Math.max(0, scrollTop) + Math.max(0, viewportHeight)) / height) + overscan
	);
	return { start, end: Math.max(start + 1, end) };
};
