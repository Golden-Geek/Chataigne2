import { describe, expect, it } from 'vitest';
import { SpatialIndex, type GraphEdge, type GraphNode } from 'golden_graph_ui';
import { GraphDocumentAdapter } from '../graphDocumentAdapter';

const node = (id: string, x: number, y: number): GraphNode => ({
	id,
	label: id,
	position: { x, y },
	inputs: [],
	outputs: []
});

describe('GraphDocumentAdapter', () => {
	it('keeps a stable document until an input array identity changes', () => {
		const adapter = new GraphDocumentAdapter();
		const nodes = [node('a', 0, 0)];
		const edges: GraphEdge[] = [];
		const first = adapter.update(nodes, edges);
		const stable = adapter.update(nodes, edges);
		const changed = adapter.update([...nodes], edges);

		expect(stable).toBe(first);
		expect(changed.revision).toEqual({
			sequence: 2,
			topology: 2,
			payload: 2,
			presentation: 2
		});
	});
});

describe('SpatialIndex', () => {
	it('returns only nearby entries once and preserves insertion order', () => {
		const index = new SpatialIndex<GraphNode>(10);
		const first = node('first', 0, 0);
		const spanning = node('spanning', 5, 5);
		const distant = node('distant', 100, 100);
		index.insert(first.id, { left: 0, top: 0, right: 2, bottom: 2 }, first);
		index.insert(spanning.id, { left: 5, top: 5, right: 25, bottom: 25 }, spanning);
		index.insert(distant.id, { left: 100, top: 100, right: 102, bottom: 102 }, distant);

		expect(index.query({ left: -1, top: -1, right: 20, bottom: 20 })).toEqual([first, spanning]);
	});
});
