import { describe, expect, it } from 'vitest';
import { captureLayoutGesture } from '../gesture-fence';
import { graphFixture, forkGraph, changeParam, node, param } from './fixtures';
import { sample } from './gesture-fixtures';

describe('dashboard gesture source paths', () => {
	const targets = () => sample().previews.map(({ target }) => target);
	it('captures declared layout folders and retires an intermediate folder replacement', () => {
		const graph = graphFixture(1);
		graph.nodesById.set(10, { ...graph.nodesById.get(10)!, children: [90] });
		graph.nodesById.set(90, node(90, 'folder', [11, 12, 13]));
		graph.parentById.set(90, 10);
		for (const id of [11, 12, 13]) graph.parentById.set(id, 90);
		const fence = captureLayoutGesture(graph, targets());
		expect(fence).not.toBeNull();
		expect(
			fence!.isCurrent(changeParam(graph, 12, { kind: 'css_value', value: 14, unit: 'rem' }))
		).toBe(true);
		const next = forkGraph(graph);
		next.nodesById.set(90, { ...next.nodesById.get(90)!, uuid: 'replacement' });
		expect(fence!.isCurrent(next)).toBe(false);
	});
	it('rejects parameter paths that cross another widget or fail to reach the owner', () => {
		const graph = graphFixture(2);
		graph.parentById.set(11, 14);
		expect(captureLayoutGesture(graph, targets())).toBeNull();
		graph.parentById.delete(11);
		expect(captureLayoutGesture(graph, targets())).toBeNull();
	});
	it('accepts value publication and unrelated graph changes without losing a live gesture', () => {
		const graph = graphFixture(2),
			fence = captureLayoutGesture(graph, targets())!;
		const values = changeParam(graph, 11, { kind: 'vec2', value: [7, 8] });
		expect(fence.isCurrent(values)).toBe(true);
		const other = forkGraph(values);
		other.nodesById.set(14, {
			...other.nodesById.get(14)!,
			meta: { ...other.nodesById.get(14)!.meta, label: 'changed' }
		});
		expect(fence.isCurrent(other)).toBe(true);
	});
	it('fences reused widget and parameter identities', () => {
		for (const id of [10, 11, 12, 13]) {
			const graph = graphFixture(1),
				fence = captureLayoutGesture(graph, targets())!,
				next = forkGraph(graph);
			next.nodesById.set(id, { ...graph.nodesById.get(id)!, uuid: 'replacement' });
			expect(fence.isCurrent(next)).toBe(false);
		}
	});
	it('fences an ancestor reparent even if the widget and its direct parent remain identical', () => {
		const graph = graphFixture(1);
		graph.nodesById.set(30, node(30, 'dashboard_widget_container', [10]));
		graph.parentById.set(30, 1);
		graph.parentById.set(10, 30);
		const fence = captureLayoutGesture(graph, targets())!,
			moved = forkGraph(graph);
		moved.parentById.set(30, 0);
		expect(fence.isCurrent(moved)).toBe(false);
	});
	it('fences layout and insertion changes through their published dependencies', () => {
		const graph = graphFixture(1),
			layout = param(99, 'layout_kind', { kind: 'enum', value: 'free' });
		graph.nodesById.set(99, layout);
		graph.parentById.set(99, 1);
		graph.nodesById.set(1, { ...graph.nodesById.get(1)!, children: [99, 10] });
		const fence = captureLayoutGesture(graph, targets())!;
		expect(fence.isCurrent(changeParam(graph, 99, { kind: 'enum', value: 'grid' }))).toBe(false);
		const another = captureLayoutGesture(graph, targets())!,
			reordered = forkGraph(graph);
		reordered.nodesById.set(1, { ...graph.nodesById.get(1)!, children: [10, 99] });
		expect(another.isCurrent(reordered)).toBe(false);
	});
	it('rejects missing parameters and cyclic source paths', () => {
		const graph = graphFixture(1);
		graph.nodesById.delete(11);
		expect(captureLayoutGesture(graph, targets())).toBeNull();
		const cyclic = graphFixture(1);
		cyclic.parentById.set(1, 10);
		expect(captureLayoutGesture(cyclic, targets())).toBeNull();
	});
});
