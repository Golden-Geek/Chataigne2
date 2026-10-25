import { describe, expect, it, vi } from 'vitest';
import { createGraphStore } from 'golden_ui/store/graph.svelte';
import { getDirectParamNode } from 'golden_ui/components/panels/dashboard/dashboard-model.ts';
import type { UiEventDto, UiGraphOp, UiNodeDto } from 'golden_ui/types';
import { node, snapshot } from './fixtures/workbench-graph';

const parameter = (id: number, decl: string): UiNodeDto => ({
	...node(id),
	decl_id: decl,
	data: {
		kind: 'parameter',
		param: {
			value: { kind: 'enum', value: 'free' },
			default_value: { kind: 'enum', value: 'free' },
			event_behaviour: 'Coalesce',
			read_only: false,
			constraints: { enum_options: [], policy: 'ClampAdapt' },
			ui_hints: {},
			control: { mode: 'manual', spec: { mode: 'manual' } },
			reference_allowed_targets: [],
			reference_visible_nodes: []
		}
	}
});
const transaction = (ops: UiGraphOp[]): UiEventDto => ({
	time: { tick: 1, micro: 0, seq: 2 },
	kind: {
		kind: 'graphTransaction',
		tx_id: 1,
		epoch: 1,
		base_graph_version: 0,
		next_graph_version: 1,
		ops
	}
});
function fixture(count = 0) {
	const graph = createGraphStore();
	const value = snapshot(0, 1);
	const items = Array.from({ length: count }, (_, i): UiNodeDto => ({
		...node(i + 100),
		user_role: 'itemRoot',
		children: []
	}));
	value.nodes = [
		{ ...node(0), children: [1, ...items.map((n) => n.node_id)] },
		{ ...node(1), children: [2, 3] },
		parameter(2, 'presentation/layout_kind'),
		parameter(3, 'layout_kind'),
		...items
	];
	graph.loadSnapshot(value);
	return graph;
}
function apply(graph: ReturnType<typeof createGraphStore>, event: UiEventDto, staged: boolean) {
	if (staged) {
		const work = graph.createEventWork(event)!;
		let turns = 0;
		while (!work.advance(7).done) expect(++turns).toBeLessThan(10000);
	}
	graph.applyEvent(event);
}

describe('local structural declaration projection', () => {
	it('bounds repeated local lookup independently of 10k item siblings and preserves scalar identity', () => {
		const graph = fixture(10000),
			before = graph.state;
		expect(before.structuralChildrenById.size).toBe(2);
		const get = vi.spyOn(before.nodesById, 'get');
		for (let i = 0; i < 100; i++) {
			expect(getDirectParamNode(before, before.nodesById.get(0)!, 'layout_kind')?.node_id).toBe(3);
			expect(getDirectParamNode(before, before.nodesById.get(0)!, 'missing')).toBeNull();
		}
		expect(get.mock.calls.length).toBeLessThan(1200);
		get.mockRestore();
		graph.applyEvent(
			transaction([
				{
					kind: 'paramPatched',
					node: 2,
					param: 2,
					patch: { value: { kind: 'enum', value: 'grid' } }
				}
			])
		);
		expect(graph.state.structuralChildrenById.get(0)).toBe(before.structuralChildrenById.get(0));
		expect(graph.state.structuralChildrenById.get(1)).toBe(before.structuralChildrenById.get(1));
		expect(
			getDirectParamNode(graph.state, graph.state.nodesById.get(0)!, 'presentation/layout_kind')
				?.node_id
		).toBe(2);
	});
	it.each([false, true])(
		'updates roles, order, missing-child materialization and retirement (staged=%s)',
		(staged) => {
			const graph = fixture();
			const original = graph.state;
			apply(
				graph,
				transaction([
					{ kind: 'nodeCreated', parent: null, snapshot: { ...node(0), children: [50, 1] } },
					{
						kind: 'nodeCreated',
						parent: 0,
						snapshot: { ...node(1), user_role: 'itemRoot', children: [2, 3] }
					}
				]),
				staged
			);
			expect(graph.state.structuralChildrenById.get(0)).toBeUndefined();
			expect(original.structuralChildrenById.get(0)).toEqual([1]);
			apply(
				graph,
				transaction([{ kind: 'nodeCreated', parent: 0, snapshot: parameter(50, 'layout_kind') }]),
				staged
			);
			expect(graph.state.structuralChildrenById.get(0)).toEqual([50]);
			expect(
				getDirectParamNode(graph.state, graph.state.nodesById.get(0)!, 'layout_kind')?.node_id
			).toBe(50);
			apply(
				graph,
				transaction([
					{ kind: 'nodeCreated', parent: 0, snapshot: { ...node(1), children: [2, 3] } }
				]),
				staged
			);
			apply(
				graph,
				transaction([{ kind: 'childrenReordered', parent: 0, children: [1, 50] }]),
				staged
			);
			expect(graph.state.structuralChildrenById.get(0)).toEqual([1, 50]);
			apply(
				graph,
				transaction([
					{
						kind: 'nodeMoved',
						node: 50,
						old_parent: 0,
						new_parent: 1,
						old_parent_after: { parent: 0, children: [1] },
						new_parent_after: { parent: 1, children: [2, 50, 3] }
					}
				]),
				staged
			);
			expect(graph.state.structuralChildrenById.get(0)).toEqual([1]);
			expect(graph.state.structuralChildrenById.get(1)).toEqual([2, 50, 3]);
			apply(
				graph,
				transaction([
					{
						kind: 'subtreeRemoved',
						root: 50,
						removed_ids: [50],
						parent_after: { parent: 1, children: [2, 3] }
					}
				]),
				staged
			);
			expect(graph.state.structuralChildrenById.get(0)).toEqual([1]);
			expect(graph.state.structuralChildrenById.has(50)).toBe(false);
			graph.reset();
			expect(graph.state.structuralChildrenById.size).toBe(0);
		}
	);
	it('keeps direct batch changes indexed when a previously prepared event becomes stale', () => {
		const graph = fixture();
		const stale = transaction([{ kind: 'childrenReordered', parent: 0, children: [1] }]);
		const work = graph.createEventWork(stale)!;
		while (!work.advance(16).done) {}
		graph.applyBatch({
			events: [
				transaction([
					{
						kind: 'nodeCreated',
						parent: 0,
						snapshot: { ...node(1), user_role: 'itemRoot', children: [2, 3] }
					}
				]),
				stale
			]
		});
		expect(graph.state.requiresResync).toBe(true);
		expect(graph.state.structuralChildrenById.get(0)).toBeUndefined();
	});
	it('budgets sibling classification before a staged projection is published', () => {
		const graph = fixture(10000),
			before = graph.state;
		const event = transaction([
			{
				kind: 'nodeCreated',
				parent: 0,
				snapshot: { ...before.nodesById.get(1)!, user_role: 'itemRoot' }
			}
		]);
		const work = graph.createEventWork(event)!;
		const get = vi.spyOn(before.nodesById.constructor.prototype, 'get');
		let turns = 0,
			done = false;
		while (!done) {
			get.mockClear();
			done = work.advance(7).done;
			expect(get.mock.calls.length).toBeLessThan(40);
			expect(graph.state).toBe(before);
			expect(++turns).toBeLessThan(3000);
		}
		get.mockRestore();
		expect(turns).toBeGreaterThan(1000);
		graph.applyEvent(event);
		expect(graph.state.structuralChildrenById.get(0)).toBeUndefined();
	});
});
