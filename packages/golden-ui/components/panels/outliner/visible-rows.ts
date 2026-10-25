import type { NodeId, UiNodeDto } from '../../../types';
import { VersionedNodeMap } from '../../../store/graph-index';

export interface OutlinerRow {
	nodeId: NodeId;
	parentId: NodeId | null;
	level: number;
	indent: number;
	/** Present on projected rows; absent on traversal roots. */
	expanded?: boolean;
}

export interface OutlinerRowGraph {
	rootId: NodeId | null;
	nodesById: Pick<ReadonlyMap<NodeId, UiNodeDto>, 'get'>;
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

/**
 * Panel-owned lookup projection. Reopening a branch must not repeat persistent-trie lookups
 * for every unchanged sibling. Published changes update only previously visited records.
 * Mutable/non-versioned maps deliberately get no retained cache.
 */
export function createOutlinerProjection() {
	let previous: VersionedNodeMap<UiNodeDto> | null = null;
	let priorOptions: VisibleRowsOptions | null = null;
	let priorRows: OutlinerRow[] = [];
	let positions = new Map<NodeId, number>();
	const cached = new Map<NodeId, UiNodeDto>();
	const lookup = (source: OutlinerRowGraph['nodesById']): OutlinerRowGraph['nodesById'] => {
		if (!(source instanceof VersionedNodeMap)) {
			previous = null;
			cached.clear();
			return source;
		}
		if (previous !== source) {
			if (previous)
				source.forEachChange(previous, (id, value) => {
					if (!value) cached.delete(id);
					else if (cached.has(id)) cached.set(id, value);
				});
			previous = source;
		}
		return {
			get: (id) => {
				let node = cached.get(id);
				if (!node) {
					node = source.get(id);
					if (node) cached.set(id, node);
				}
				return node;
			}
		};
	};
	const project = (options: VisibleRowsOptions) => {
		const readOptions = {
			...options,
			graph: { rootId: options.graph.rootId, nodesById: lookup(options.graph.nodesById) }
		};
		const nextRows = priorOptions
			? updateVisibleRows(priorOptions, options, readOptions, priorRows, positions)
			: undefined;
		const result = nextRows ?? collectVisibleOutlinerRows(readOptions);
		if (result !== priorRows) positions = new Map(result.map((row, index) => [row.nodeId, index]));
		priorOptions = options;
		priorRows = result;
		return result;
	};
	project.lookup = lookup;
	project.clear = () => {
		previous = null;
		cached.clear();
		priorOptions = null;
		priorRows = [];
		positions.clear();
	};
	return project;
}

const sameRow = (left: OutlinerRow, right: OutlinerRow) =>
	left.nodeId === right.nodeId &&
	left.parentId === right.parentId &&
	left.level === right.level &&
	left.indent === right.indent &&
	left.expanded === right.expanded;

/** Reproject changed openness paths; structural, search and policy changes use a full projection. */
function updateVisibleRows(
	before: VisibleRowsOptions,
	next: VisibleRowsOptions,
	read: VisibleRowsOptions,
	rows: OutlinerRow[],
	positions: ReadonlyMap<NodeId, number>
): OutlinerRow[] | undefined {
	const oldSource = before.graph.nodesById,
		source = next.graph.nodesById;
	if (
		!(source instanceof VersionedNodeMap) ||
		!(oldSource instanceof VersionedNodeMap) ||
		before.matchesQuery ||
		next.matchesQuery ||
		before.graph.rootId !== next.graph.rootId ||
		before.initiallyExpandedDepth !== next.initiallyExpandedDepth ||
		before.canRenderChildren !== next.canRenderChildren ||
		before.rowVisible !== next.rowVisible
	)
		return;
	const oldRoots = before.roots,
		roots = next.roots;
	if (
		Boolean(oldRoots) !== Boolean(roots) ||
		oldRoots?.length !== roots?.length ||
		oldRoots?.some((root, index) => !sameRow(root, roots![index]))
	)
		return;
	const dirty = new Set<NodeId>();
	let full = false;
	if (source !== oldSource)
		source.forEachChange(oldSource, (id, node, old) => {
			if (full) return;
			if (
				!node ||
				!old ||
				node.uuid !== old.uuid ||
				node.children !== old.children ||
				(before.rowVisible && before.rowVisible(node) !== before.rowVisible(old)) ||
				(before.canRenderChildren &&
					before.canRenderChildren(node) !== before.canRenderChildren(old))
			) {
				full = true;
				return;
			}
			if (node.meta.presentation?.collapsed !== old.meta.presentation?.collapsed) dirty.add(id);
		});
	if (full) return;
	for (const key of new Set([
		...Object.keys(before.opennessByNodeId),
		...Object.keys(next.opennessByNodeId)
	]))
		if (before.opennessByNodeId[key] !== next.opennessByNodeId[key]) dirty.add(Number(key));
	for (const id of before.autoExpandAncestorNodeIds)
		if (!next.autoExpandAncestorNodeIds.has(id)) dirty.add(id);
	for (const id of next.autoExpandAncestorNodeIds)
		if (!before.autoExpandAncestorNodeIds.has(id)) dirty.add(id);
	if (dirty.size > 64) return;
	const changed = [...dirty]
		.map((id) => positions.get(id))
		.filter((index): index is number => index !== undefined)
		.sort((a, b) => a - b);
	if (!changed.length) return rows;
	const edits: { start: number; end: number; replacement: OutlinerRow[] }[] = [];
	let covered = -1;
	for (const start of changed) {
		if (start < covered) continue;
		const root = rows[start];
		let end = start + 1;
		while (end < rows.length && rows[end].indent > root.indent) end++;
		covered = end;
		const replacement = collectVisibleOutlinerRows({ ...read, roots: [root] });
		if (
			replacement.length === end - start &&
			replacement.every((row, index) => sameRow(row, rows[start + index]))
		)
			continue;
		edits.push({ start, end, replacement });
	}
	if (!edits.length) return rows;
	const result: OutlinerRow[] = [];
	let cursor = 0;
	for (const edit of edits) {
		while (cursor < edit.start) result.push(rows[cursor++]);
		for (const row of edit.replacement) result.push(row);
		cursor = edit.end;
	}
	while (cursor < rows.length) result.push(rows[cursor++]);
	return result;
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
		const explicit = opennessByNodeId[String(row.nodeId)];
		const expanded =
			typeof explicit === 'boolean'
				? explicit
				: autoExpandAncestorNodeIds.has(row.nodeId) ||
					(node.meta.presentation?.collapsed !== true && row.level < initiallyExpandedDepth);
		if (showRow)
			rows.push({
				...row,
				expanded: node.children.length > 0 && canRenderChildren(node) ? expanded : undefined
			});
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
