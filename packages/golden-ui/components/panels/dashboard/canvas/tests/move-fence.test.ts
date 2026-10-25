import { describe, expect, it } from 'vitest';
import { captureWidgetMove } from '../move-fence';
import { changeParam, forkGraph, graphFixture, node, param } from './fixtures';

describe('dashboard move lifetime', () => {
	it('rejects absent sources, wrong parents and non-surface targets', () => {
		const graph = graphFixture(2);
		expect(captureWidgetMove(null, 10, 1, 1)).toBeNull();
		expect(captureWidgetMove(graph, 999, 1, 1)).toBeNull();
		expect(captureWidgetMove(graph, 10, 14, 1)).toBeNull();
		expect(captureWidgetMove(graph, 10, 1, 14)).toBeNull();
		expect(captureWidgetMove(graph, 11, 10, 1)).toBeNull();
	});
	it('fences source and both owner identities and source reparenting', () => {
		const graph = graphFixture(2);
		graph.nodesById.set(30, node(30, 'dashboard_widget_container'));
		const fence = captureWidgetMove(graph, 10, 1, 30)!;
		for (const id of [10, 1, 30]) {
			const next = forkGraph(graph);
			next.nodesById.set(id, { ...graph.nodesById.get(id)!, uuid: 'replacement' });
			expect(fence.isCurrent(next)).toBe(false);
		}
		const moved = forkGraph(graph);
		moved.parentById.set(10, 30);
		expect(fence.isCurrent(moved)).toBe(false);
		const removed = forkGraph(graph);
		removed.nodesById.delete(10);
		expect(fence.isCurrent(removed)).toBe(false);
	});
	it('retires stale insertion order and layout while accepting ordinary value publications', () => {
		const graph = graphFixture(2),
			layout = param(99, 'layout_kind', { kind: 'enum', value: 'free' });
		if (layout.data.kind !== 'parameter') throw new Error('Expected layout parameter');
		graph.nodesById.set(99, layout);
		graph.paramsById.set(99, layout.data.param);
		graph.nodesById.set(1, { ...graph.nodesById.get(1)!, children: [99, 10, 14] });
		const fence = captureWidgetMove(graph, 10, 1, 1)!;
		expect(fence.isCurrent(changeParam(graph, 11, { kind: 'vec2', value: [99, 101] }))).toBe(true);
		expect(fence.isCurrent(changeParam(graph, 99, { kind: 'enum', value: 'grid' }))).toBe(false);
		const reordered = forkGraph(graph);
		reordered.nodesById.set(1, { ...graph.nodesById.get(1)!, children: [99, 14, 10] });
		expect(fence.isCurrent(reordered)).toBe(false);
	});
});
