import { describe, expect, it } from 'vitest';
import type { UiNodeDto } from '../../../../types';
import { collectVisibleOutlinerRows, outlinerWindow } from '../visible-rows';

const node = (
	nodeId: number,
	children: number[] = [],
	label = String(nodeId),
	collapsed = false
): UiNodeDto =>
	({
		node_id: nodeId,
		children,
		meta: { label, short_name: '', presentation: { collapsed } },
		node_type: 'Folder'
	}) as UiNodeDto;

const graph = (...nodes: UiNodeDto[]) => ({
	rootId: nodes[0]?.node_id ?? null,
	nodesById: new Map(nodes.map((entry) => [entry.node_id, entry]))
});

const project = (
	nodes: UiNodeDto[],
	options: {
		opennessByNodeId?: Record<string, boolean>;
		autoExpandAncestorNodeIds?: Set<number>;
		matchesQuery?: (candidate: UiNodeDto) => boolean;
	} = {}
) =>
	collectVisibleOutlinerRows({
		graph: graph(...nodes),
		opennessByNodeId: options.opennessByNodeId ?? {},
		autoExpandAncestorNodeIds: options.autoExpandAncestorNodeIds ?? new Set(),
		initiallyExpandedDepth: 2,
		matchesQuery: options.matchesQuery
	});

describe('outliner visible row projection', () => {
	it('preserves tree order, depth, and sibling parent IDs without mounting descendants', () => {
		const rows = project([
			node(0, [1, 4]),
			node(1, [2, 3]),
			node(2, [5]),
			node(3),
			node(4),
			node(5)
		]);
		expect(rows.map(({ nodeId, parentId, level }) => [nodeId, parentId, level])).toEqual([
			[0, null, 0],
			[1, 0, 1],
			[2, 1, 2],
			[3, 1, 2],
			[4, 0, 1]
		]);
	});

	it('honors persisted openness and reveals selected ancestors even when explicitly closed', () => {
		const nodes = [node(0, [1]), node(1, [2], 'Folder', true), node(2)];
		expect(project(nodes).map((row) => row.nodeId)).toEqual([0, 1]);
		expect(project(nodes, { opennessByNodeId: { '1': true } }).map((row) => row.nodeId)).toEqual([
			0, 1, 2
		]);
		expect(
			project(nodes, {
				opennessByNodeId: { '1': false },
				autoExpandAncestorNodeIds: new Set([1])
			}).map((row) => row.nodeId)
		).toEqual([0, 1, 2]);
	});

	it('does not visit descendants of a closed branch without a search query', () => {
		const nodes = [node(0, [1, 2]), node(1, [3], 'Closed', true), node(2), node(3)];
		const indexed = graph(...nodes);
		const get = indexed.nodesById.get.bind(indexed.nodesById);
		indexed.nodesById.get = (nodeId) => {
			if (nodeId === 3) throw new Error('closed descendant was visited');
			return get(nodeId);
		};
		expect(
			collectVisibleOutlinerRows({
				graph: indexed,
				opennessByNodeId: {},
				autoExpandAncestorNodeIds: new Set(),
				initiallyExpandedDepth: 2
			}).map((row) => row.nodeId)
		).toEqual([0, 1, 2]);
	});

	it('retains matching ancestors for search without forcing closed branches open', () => {
		const nodes = [node(0, [1, 3]), node(1, [2]), node(2, [], 'Target'), node(3, [], 'Other')];
		const matchesQuery = (candidate: UiNodeDto) => candidate.meta.label === 'Target';
		expect(project(nodes, { matchesQuery }).map((row) => row.nodeId)).toEqual([0, 1, 2]);
		expect(
			project(nodes, { matchesQuery, opennessByNodeId: { '1': false } }).map((row) => row.nodeId)
		).toEqual([0, 1]);
	});

	it('uses an iterative traversal for deep trees', () => {
		const nodes = Array.from({ length: 10_001 }, (_, index) =>
			node(index, index < 10_000 ? [index + 1] : [])
		);
		const opennessByNodeId = Object.fromEntries(nodes.map((entry) => [entry.node_id, true]));
		expect(project(nodes, { opennessByNodeId })).toHaveLength(10_001);
	});

	it('clamps the window and renders a bounded overscan around the viewport', () => {
		expect(outlinerWindow(0, 0, 100, 20)).toEqual({ start: 0, end: 0 });
		expect(outlinerWindow(1000, 200, 100, 20)).toEqual({ start: 2, end: 23 });
		expect(outlinerWindow(1000, 50_000, 100, 20)).toEqual({ start: 999, end: 1000 });
	});
});
