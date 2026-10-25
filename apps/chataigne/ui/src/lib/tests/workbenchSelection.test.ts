import { describe, expect, it } from 'vitest';
import { createGraphStore } from 'golden_ui/store/graph.svelte';
import { createWorkbenchSelectionStore } from 'golden_ui/store/session/selection.svelte';
import { node, snapshot } from './fixtures/workbench-graph';

function fixture(count: number) {
	const graph = createGraphStore();
	const value = snapshot(0, 1);
	value.nodes = Array.from({ length: count + 1 }, (_, id) => node(id));
	value.nodes[0].children = value.nodes.slice(1).map((n) => n.node_id);
	for (const n of value.nodes.slice(1)) n.decl_id = `item_${n.node_id}`;
	graph.loadSnapshot(value);
	return { graph, selection: createWorkbenchSelectionStore(graph) };
}
describe('ordered workbench selection membership', () => {
	it('preserves first-occurrence order and validates IDs across all selection modes', () => {
		const { selection: s } = fixture(5);
		s.selectNodes([4, 2, 4, 999, 3]);
		expect(s.selectedNodeIds).toEqual([4, 2, 3]);
		s.selectNodes([3, 1, 1], 'ADD');
		expect(s.selectedNodeIds).toEqual([4, 2, 3, 1]);
		s.selectNodes([2, 2, 999], 'REMOVE');
		expect(s.selectedNodeIds).toEqual([4, 3, 1]);
		s.selectNodes([4, 5, 5], 'TOGGLE');
		expect(s.selectedNodeIds).toEqual([3, 1, 5]);
		expect(s.selectedNodeId).toBe(3);
		expect(s.getSelectedNodes().map((n) => n.node_id)).toEqual([3, 1, 5]);
		expect(s.isNodeSelected(5)).toBe(true);
		expect(s.isNodeSelected(4)).toBe(false);
		s.clearSelection();
		expect(s.isNodeSelected(5)).toBe(false);
	});
	it('handles large duplicate-rich selections while retaining authored occurrence order', () => {
		const { selection: s } = fixture(20000);
		const ids = Array.from({ length: 10000 }, (_, i) => 10000 - i);
		s.selectNodes([...ids, ...ids]);
		expect(s.selectedNodeIds).toEqual(ids);
		s.selectNodes([10001, 10000, 20000, 10001], 'ADD');
		expect(s.selectedNodeIds.slice(-2)).toEqual([10001, 20000]);
		s.selectNodes(
			ids.filter((id) => id % 2 === 0),
			'REMOVE'
		);
		expect(s.selectedNodeIds).toEqual([...ids.filter((id) => id % 2 === 1), 10001, 20000]);
		expect(s.isNodeSelected(9999)).toBe(true);
		expect(s.isNodeSelected(10000)).toBe(false);
		s.reset();
		expect(s.selectedNodeIds).toEqual([]);
		expect(s.isNodeSelected(9999)).toBe(false);
	});
	it('reconciles membership and first-selected identity when nodes retire', () => {
		const { graph, selection: s } = fixture(5);
		s.selectNodes([5, 3, 1]);
		const next = snapshot(0, 2);
		next.nodes.push(node(1), node(3));
		next.nodes[0].children = [1, 3];
		graph.loadSnapshot(next);
		s.reconcileSelection();
		expect(s.selectedNodeIds).toEqual([3, 1]);
		expect(s.selectedNodeId).toBe(3);
		expect(s.isNodeSelected(5)).toBe(false);
	});
});
