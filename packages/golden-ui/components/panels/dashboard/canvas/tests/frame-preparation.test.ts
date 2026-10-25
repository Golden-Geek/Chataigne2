import { describe, expect, it } from 'vitest';
import { framePreparation } from '../frame-preparation';
import { forkGraph, graphFixture, node } from './fixtures';

describe('dashboard frame paths', () => {
	it('fences reused identities, reparenting and order changes while allowing unchanged structure', () => {
		const graph = graphFixture(2),
			plan = framePreparation(graph, 1, [10, 14]);
		const same = forkGraph(graph);
		same.nodesById.set(1, { ...graph.nodesById.get(1)!, children: [10, 14] });
		expect(plan.isCurrent(same, [10, 14])).toBe(true);
		const moved = forkGraph(graph);
		moved.parentById.set(10, 14);
		expect(plan.isCurrent(moved, [10])).toBe(false);
		const replaced = forkGraph(graph);
		replaced.nodesById.set(10, { ...graph.nodesById.get(10)!, uuid: 'replacement' });
		expect(plan.currentChildren(replaced, 1)).toEqual([14]);
		const reordered = forkGraph(graph);
		reordered.nodesById.set(1, { ...graph.nodesById.get(1)!, children: [14, 10] });
		expect(plan.isCurrent(reordered, [10, 14])).toBe(false);
	});
	it('requests targets and ancestors in root-first order without requesting parameter controls', () => {
		const graph = graphFixture(1),
			container = node(30, 'dashboard_widget_container', [10]);
		container.user_role = 'itemRoot';
		graph.nodesById.set(30, container);
		graph.parentById.set(30, 1);
		graph.parentById.set(10, 30);
		const result = framePreparation(graph, 1, [10, 11]);
		expect(result.paths.get(10)).toEqual([
			{ parent: 1, child: 30 },
			{ parent: 30, child: 10 }
		]);
		expect(result.paths.get(11)).toEqual(result.paths.get(10));
		expect([...result.children.get(30)!]).toEqual([10]);
	});
	it('rejects foreign and cyclic ancestry instead of forcing unrelated views', () => {
		const graph = graphFixture(2);
		graph.parentById.set(10, 14);
		graph.parentById.set(14, 10);
		expect(framePreparation(graph, 1, [10, 14, 999]).children.size).toBe(0);
		expect(framePreparation(null, 1, [10]).paths.size).toBe(0);
	});
	it('deduplicates shared surfaces while preserving requested authored identities', () => {
		const graph = graphFixture(1000),
			ids = graph.nodesById.get(1)!.children;
		const parent = graph.parentById;
		const plan = framePreparation(graph, 1, ids);
		expect(plan.paths.size).toBe(1000);
		expect([...plan.children.get(1)!]).toEqual(ids);
		expect(graph.parentById).toBe(parent);
	});
});
