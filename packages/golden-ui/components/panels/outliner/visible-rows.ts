import type { NodeId, UiNodeDto } from '../../../types';

export interface OutlinerRow {
	nodeId: NodeId;
	parentId: NodeId | null;
	level: number;
	indent: number;
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
	roots?: readonly OutlinerRow[];
	canRenderChildren?: (node: UiNodeDto) => boolean;
	rowVisible?: (node: UiNodeDto) => boolean;
}

const matchingSubtrees = (
	graph: OutlinerRowGraph,
	matchesQuery: (node: UiNodeDto) => boolean,
	roots: readonly OutlinerRow[],
	canRenderChildren: (node: UiNodeDto) => boolean
): { matches: ReadonlySet<NodeId>; nodes: ReadonlyMap<NodeId, UiNodeDto> } => {
	const matches = new Set<NodeId>();
	const nodes = new Map<NodeId, UiNodeDto>();
	const pending: Array<{ nodeId: NodeId; postorder: boolean }> = [];
	for (const root of roots) pending.push({ nodeId: root.nodeId, postorder: false });

	while (pending.length > 0) {
		const { nodeId, postorder } = pending.pop()!;
		if (postorder) {
			const node = nodes.get(nodeId)!;
			if (
				matchesQuery(node) ||
				(canRenderChildren(node) && node.children.some((child) => matches.has(child)))
			) {
				matches.add(nodeId);
			}
			continue;
		}
		if (nodes.has(nodeId)) continue;
		const node = graph.nodesById.get(nodeId);
		if (!node) continue;
		nodes.set(nodeId, node);
		pending.push({ nodeId, postorder: true });
		if (canRenderChildren(node))
			for (const child of node.children) pending.push({ nodeId: child, postorder: false });
	}
	return { matches, nodes };
};

/** Project expanded graph paths without mounting a component for every visible node. */
export const collectVisibleOutlinerRows = ({
	graph,
	opennessByNodeId,
	autoExpandAncestorNodeIds,
	initiallyExpandedDepth,
	matchesQuery,
	roots = graph.rootId === null
		? []
		: [{ nodeId: graph.rootId, parentId: null, level: 0, indent: 0 }],
	canRenderChildren = () => true,
	rowVisible = () => true
}: VisibleRowsOptions): OutlinerRow[] => {
	if (!roots.length) return [];
	const filtered = matchesQuery
		? matchingSubtrees(graph, matchesQuery, roots, canRenderChildren)
		: null;
	const rows: OutlinerRow[] = [];
	const visited = new Set<NodeId>();
	const pending: OutlinerRow[] = [...roots].reverse();
	while (pending.length > 0) {
		const row = pending.pop()!;
		if (visited.has(row.nodeId) || (filtered !== null && !filtered.matches.has(row.nodeId)))
			continue;
		visited.add(row.nodeId);
		const node = filtered?.nodes.get(row.nodeId) ?? graph.nodesById.get(row.nodeId);
		if (!node) continue;
		const showRow = rowVisible(node);
		if (showRow) rows.push(row);
		const explicit = opennessByNodeId[String(row.nodeId)];
		const expanded =
			autoExpandAncestorNodeIds.has(row.nodeId) ||
			(typeof explicit === 'boolean'
				? explicit
				: node.meta.presentation?.collapsed !== true && row.level < initiallyExpandedDepth);
		if ((showRow && !expanded) || !canRenderChildren(node)) continue;
		for (let index = node.children.length - 1; index >= 0; index -= 1) {
			pending.push({
				nodeId: node.children[index],
				parentId: row.nodeId,
				level: row.level + 1,
				indent: row.indent + (showRow ? 1 : 0)
			});
		}
	}
	return rows;
};
