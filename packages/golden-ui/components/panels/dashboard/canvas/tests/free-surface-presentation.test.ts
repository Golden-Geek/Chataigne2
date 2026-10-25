import { describe, expect, it } from 'vitest';
import { FreeSurfaceGeometry, freeSurfaceDetailLimit } from '../free-surface-geometry';
import { changeParam, forkGraph, graphFixture } from './fixtures';

const metrics = {
	rootRemPx: 16,
	emPx: 16,
	surfaceWidthPx: 1200,
	surfaceHeightPx: 850,
	viewportWidthPx: 1680,
	viewportHeightPx: 1200
};
const viewport = { left: -1, top: -1, width: 1_000_000, height: 100 };

describe('bounded free dashboard presentation', () => {
	it('keeps complete detail at the limit and uses authored groups above it', () => {
		for (const count of [freeSurfaceDetailLimit, freeSurfaceDetailLimit + 1]) {
			const graph = graphFixture(count);
			const geometry = new FreeSurfaceGeometry();
			geometry.sync(graph, graph.nodesById.get(1)!, metrics);
			const presentation = geometry.presentation(viewport);
			expect(presentation.overview).toBe(count > freeSurfaceDetailLimit);
			if (presentation.overview) {
				expect(presentation.ids).toEqual([]);
				expect(presentation.groups.reduce((sum, group) => sum + group.count, 0)).toBe(count);
			} else expect(presentation.ids).toEqual(graph.nodesById.get(1)!.children);
		}
	});
	it('never exposes a truncated dense candidate sample as complete controls', () => {
		const graph = graphFixture(5000);
		for (let index = 0; index < 5000; index++) {
			const id = 11 + index * 4;
			const current = graph.nodesById.get(id)!;
			if (current.data.kind !== 'parameter') throw new Error('Expected position parameter');
			graph.nodesById.set(id, {
				...current,
				data: {
					kind: 'parameter',
					param: { ...current.data.param, value: { kind: 'vec2', value: [0, 0] } }
				}
			});
		}
		const geometry = new FreeSurfaceGeometry();
		geometry.sync(graph, graph.nodesById.get(1)!, metrics);
		const visible = geometry.presentation({ left: 0, top: 0, width: 100, height: 50 });
		expect(visible.overview).toBe(true);
		expect(visible.ids).toEqual([]);
		expect(visible.groups.length).toBeLessThanOrEqual(256);
		expect(visible.groups.reduce((sum, group) => sum + group.count, 0)).toBe(5000);
		expect(geometry.hitTest(1, 1)).toBe(10 + 4999 * 4);
		const reordered = forkGraph(graph);
		reordered.nodesById.set(1, {
			...graph.nodesById.get(1)!,
			children: [...graph.nodesById.get(1)!.children].reverse()
		});
		geometry.sync(reordered, reordered.nodesById.get(1)!, metrics);
		expect(geometry.hitTest(1, 1)).toBe(10);
		const moved = changeParam(reordered, 11, { kind: 'vec2', value: [-1000, 20] });
		geometry.sync(moved, moved.nodesById.get(1)!, metrics);
		expect(geometry.hitTest(-999, 21)).toBe(10);
		expect(geometry.hitTest(1, 1)).toBe(14);
		expect(geometry.presentation({ left: -1001, top: 19, width: 30, height: 30 }).ids).toEqual([
			10
		]);
		geometry.clear();
		expect(geometry.presentation(viewport)).toEqual({ overview: false, ids: [], groups: [] });
	});
	it('keeps exact hit edges and retires removed identity from dense groups', () => {
		const before = graphFixture(2);
		const geometry = new FreeSurfaceGeometry();
		geometry.sync(before, before.nodesById.get(1)!, metrics);
		expect(geometry.hitTest(192, 1)).toBe(null);
		expect(geometry.hitTest(0, 0)).toBe(10);
		const after = forkGraph(before);
		after.nodesById.set(1, { ...before.nodesById.get(1)!, children: [14] });
		geometry.sync(after, after.nodesById.get(1)!, metrics);
		expect(geometry.hitTest(1, 1)).toBe(null);
		expect(geometry.presentation(viewport).ids).toEqual([14]);
	});
});
