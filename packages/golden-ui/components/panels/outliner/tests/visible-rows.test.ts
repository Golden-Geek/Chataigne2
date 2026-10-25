import { describe, expect, it } from 'vitest';
import type { UiNodeDto } from '../../../../types';
import { collectVisibleOutlinerRows } from '../visible-rows';

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
	it('reads each graph record once while filtering and reuses it for row projection', () => {
		const nodes = [
			node(
				0,
				Array.from({ length: 1000 }, (_, index) => index + 1)
			),
			...Array.from({ length: 1000 }, (_, index) => node(index + 1))
		];
		const indexed = graph(...nodes),
			get = indexed.nodesById.get.bind(indexed.nodesById);
		let reads = 0;
		indexed.nodesById.get = (id) => {
			reads++;
			return get(id);
		};
		const rows = collectVisibleOutlinerRows({
			graph: indexed,
			opennessByNodeId: {},
			autoExpandAncestorNodeIds: new Set(),
			initiallyExpandedDepth: 5,
			matchesQuery: (candidate) => candidate.node_id === 1000
		});
		expect(rows.map((row) => row.nodeId)).toEqual([0, 1000]);
		expect(reads).toBe(1001);
	});
	it('projects manager roots without entering domain-hidden child trees', () => {
		const nodes = [node(0, [1, 2]), node(1, [3]), node(2, [4]), node(3), node(4, [], 'Target')];
		const rows = collectVisibleOutlinerRows({
			graph: graph(...nodes),
			roots: [
				{ nodeId: 1, parentId: 0, level: 0, indent: 0 },
				{ nodeId: 2, parentId: 0, level: 0, indent: 0 }
			],
			opennessByNodeId: {},
			autoExpandAncestorNodeIds: new Set(),
			initiallyExpandedDepth: 5,
			canRenderChildren: (candidate) => candidate.node_id !== 2,
			matchesQuery: (candidate) => candidate.meta.label === 'Target'
		});
		expect(rows).toEqual([]);
	});
	it('hoists hidden picker rows without adding indentation or requiring expansion', () => {
		const rows = collectVisibleOutlinerRows({
			graph: graph(node(0, [1], 'Hidden', true), node(1, [2]), node(2)),
			opennessByNodeId: {},
			autoExpandAncestorNodeIds: new Set(),
			initiallyExpandedDepth: 5,
			rowVisible: (candidate) => candidate.node_id !== 0
		});
		expect(rows.map(({ nodeId, level, indent }) => [nodeId, level, indent])).toEqual([
			[1, 1, 0],
			[2, 2, 1]
		]);
	});
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

	it('reveals selected ancestors by default but honors an explicit collapse', () => {
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
		).toEqual([0, 1]);
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
});
