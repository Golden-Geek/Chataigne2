import { describe, expect, it, vi } from 'vitest';
import { SurfaceItemOrder } from '../surface-item-order';
import { graphFixture, changeParam, forkGraph, node } from './fixtures';

describe('retained dashboard item order', () => {
	it('keeps authored indices when only distant anchors are mounted', () => {
		const graph = graphFixture(10000),
			order = new SurfaceItemOrder();
		const parent = graph.nodesById.get(1)!;
		graph.nodesById.set(3, node(3, 'folder'));
		const surface = { ...parent, children: [3, ...parent.children] };
		order.sync(graph, surface);
		expect(order.size).toBe(10000);
		expect(order.index(3, new Set())).toBeUndefined();
		expect(order.index(10 + 4 * 8000, new Set([10, 14]))).toBe(7998);
		expect(order.index(10, new Set([10]))).toBeUndefined();
		expect(order.count(new Set([10, 14, 3]))).toBe(9998);
	});
	it('does not rescan siblings on value publications or repeated point queries', () => {
		const graph = graphFixture(1000),
			order = new SurfaceItemOrder();
		order.sync(graph, graph.nodesById.get(1)!);
		const next = changeParam(graph, 11, { kind: 'vec2', value: [24, 0] });
		const reads = vi.spyOn(next.nodesById, 'get');
		order.sync(next, graph.nodesById.get(1)!);
		for (let i = 0; i < 1000; i++) expect(order.index(4006, new Set([10]))).toBe(998);
		expect(reads.mock.calls.length).toBeLessThanOrEqual(8);
	});
	it('refreshes membership/order and releases retained state', () => {
		const graph = graphFixture(3),
			order = new SurfaceItemOrder();
		order.sync(graph, graph.nodesById.get(1)!);
		const next = forkGraph(graph);
		next.nodesById.set(14, { ...next.nodesById.get(14)!, user_role: 'regular' });
		order.sync(next, next.nodesById.get(1)!);
		expect(order.index(14, new Set())).toBeUndefined();
		expect(order.index(18, new Set())).toBe(1);
		const reversed = { ...next.nodesById.get(1)!, children: [18, 14, 10] };
		order.sync(next, reversed);
		expect(order.index(18, new Set())).toBe(0);
		order.clear();
		expect(order.size).toBe(0);
		expect(order.index(18, new Set())).toBeUndefined();
	});
});
