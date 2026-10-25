import { describe, expect, it } from 'vitest';
import { captureSurface, captureSurfaceDrop } from '../surface-fence';
import { changeParam, forkGraph, graphFixture, param, indexStructuralFixture } from './fixtures';

describe('dashboard drop presentation identity', () => {
	it('requires a current source and a real layout surface', () => {
		const graph = graphFixture(2),
			source = graph.nodesById.get(10)!;
		expect(captureSurface(null, 1)).toBeNull();
		expect(captureSurface(graph, 10)).toBeNull();
		expect(captureSurfaceDrop(graph, 1, { ...source, uuid: 'retired' })).toBeNull();
		expect(captureSurfaceDrop(graph, 1, source)?.isCurrent(graph)).toBe(true);
	});
	it('retires either replaced identity, missing source and reordered insertion context', () => {
		const graph = graphFixture(2),
			fence = captureSurfaceDrop(graph, 1, graph.nodesById.get(10)!)!;
		for (const id of [1, 10]) {
			const next = forkGraph(graph);
			next.nodesById.set(id, { ...graph.nodesById.get(id)!, uuid: 'replacement' });
			expect(fence.isCurrent(next)).toBe(false);
		}
		const removed = forkGraph(graph);
		removed.nodesById.delete(10);
		expect(fence.isCurrent(removed)).toBe(false);
		const reordered = forkGraph(graph);
		reordered.nodesById.set(1, { ...graph.nodesById.get(1)!, children: [14, 10] });
		expect(fence.isCurrent(reordered)).toBe(false);
	});
	it('accepts immutable value publications but retires a changed destination layout', () => {
		const graph = graphFixture(1),
			layout = param(99, 'layout_kind', { kind: 'enum', value: 'free' });
		graph.nodesById.set(99, layout);
		graph.nodesById.set(1, { ...graph.nodesById.get(1)!, children: [99, 10] });
		indexStructuralFixture(graph);
		const fence = captureSurfaceDrop(graph, 1, graph.nodesById.get(10)!)!;
		expect(fence.isCurrent(changeParam(graph, 11, { kind: 'vec2', value: [42, 42] }))).toBe(true);
		expect(fence.isCurrent(changeParam(graph, 99, { kind: 'enum', value: 'tabs' }))).toBe(false);
	});
});
