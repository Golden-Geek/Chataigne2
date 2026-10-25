import { describe, expect, it } from 'vitest';
import { FreeSurfaceGeometry, type FreeSurfaceMetrics } from '../free-surface-geometry';
import {
	changeParam,
	forkGraph,
	graphFixture,
	node,
	param,
	indexStructuralFixture
} from './fixtures';
import { createEditOperations } from '../../../../../store/session/edit-operations';

const metrics: FreeSurfaceMetrics = {
	rootRemPx: 16,
	emPx: 12,
	surfaceWidthPx: 1200,
	surfaceHeightPx: 850,
	viewportWidthPx: 1680,
	viewportHeightPx: 1200
};

describe('retained free dashboard geometry', () => {
	it('indexes pending presentation without editing the graph and updates only its owner', async () => {
		const graph = graphFixture(1000),
			geometry = new FreeSurfaceGeometry();
		const drafts = createEditOperations({
			uuid: (id) => graph.nodesById.get(id)?.uuid,
			eventTime: () => graph.lastEventTime,
			changed: () => {},
			track: () => {}
		});
		geometry.sync(graph, graph.nodesById.get(1)!, metrics, drafts);
		const scope = drafts.open({
			current: () => true,
			build: () => ({ finish: async () => {}, cancel: async () => {} })
		})!;
		scope.publish([
			{
				kind: 'setParam',
				node: 4007,
				behaviour: 'Coalesce',
				value: { kind: 'vec2', value: [-900, 40] }
			}
		]);
		geometry.sync(graph, graph.nodesById.get(1)!, metrics, drafts);
		expect(geometry.updatedEntries).toBe(1);
		expect(geometry.get(4006)).toEqual({ left: -900, top: 40, width: 192, height: 64 });
		expect(geometry.query({ left: -901, top: 39, width: 10, height: 10 })).toEqual([4006]);
		expect(graph.paramsById.get(4007)?.value).toEqual({ kind: 'vec2', value: [999 * 256, 0] });
		geometry.sync(graph, graph.nodesById.get(1)!, metrics, drafts);
		expect(geometry.updatedEntries).toBe(0);
		await scope.cancel();
		geometry.sync(graph, graph.nodesById.get(1)!, metrics, drafts);
		expect(geometry.updatedEntries).toBe(1);
		expect(geometry.get(4006)?.left).toBe(999 * 256);
		expect(geometry.query({ left: -901, top: 39, width: 10, height: 10 })).toEqual([]);
	});
	it.each([
		['top-left', 100, -20],
		['top-center', 604, -20],
		['top-right', 1108, -20],
		['center-left', 100, 373],
		['center', 604, 373],
		['center-right', 1108, 373],
		['bottom-left', 100, 766],
		['bottom-center', 604, 766],
		['bottom-right', 1108, 766]
	] as const)('resolves the authored %s anchor without a DOM node', (anchor, left, top) => {
		let graph = graphFixture(1);
		graph = changeParam(graph, 11, { kind: 'vec2', value: [100, -20] });
		graph.nodesById.set(10, { ...graph.nodesById.get(10)!, children: [11, 12, 13, 14] });
		graph.nodesById.set(14, param(14, 'anchor', { kind: 'enum', value: anchor }));
		graph.parentById.set(14, 10);
		indexStructuralFixture(graph);
		const geometry = new FreeSurfaceGeometry();
		geometry.sync(graph, graph.nodesById.get(1)!, metrics);
		expect(geometry.get(10)).toEqual({ left, top, width: 192, height: 64 });
	});
	it('queries distant widgets without mounting and updates only a changed owner', () => {
		const geometry = new FreeSurfaceGeometry();
		const before = graphFixture(1000);
		geometry.sync(before, before.nodesById.get(1)!, metrics);
		expect(geometry.size).toBe(1000);
		expect(geometry.updatedEntries).toBe(1000);
		expect(geometry.query({ left: 999 * 256, top: 0, width: 30, height: 30 })).toEqual([4006]);
		const after = changeParam(before, 4007, { kind: 'vec2', value: [-1000, 10] });
		geometry.sync(after, after.nodesById.get(1)!, metrics);
		expect(geometry.updatedEntries).toBe(1);
		expect(geometry.query({ left: -1001, top: 9, width: 30, height: 30 })).toEqual([4006]);
		expect(geometry.query({ left: 999 * 256, top: 0, width: 30, height: 30 })).toEqual([]);
		expect(geometry.get(4006)).toEqual({ left: -1000, top: 10, width: 192, height: 64 });
		geometry.sync(after, after.nodesById.get(1)!, metrics);
		expect(geometry.updatedEntries).toBe(0);
	});
	it('does not rebuild on unrelated changes or authored paint-order changes', () => {
		const geometry = new FreeSurfaceGeometry();
		const before = graphFixture(3);
		geometry.sync(before, before.nodesById.get(1)!, metrics);
		const after = forkGraph(before);
		after.nodesById.set(0, {
			...before.nodesById.get(0)!,
			meta: { ...before.nodesById.get(0)!.meta, label: 'Changed' }
		});
		after.nodesById.set(1, { ...before.nodesById.get(1)!, children: [18, 14, 10] });
		geometry.sync(after, after.nodesById.get(1)!, metrics);
		expect(geometry.updatedEntries).toBe(0);
		expect(geometry.query({ left: -1, top: -1, width: 1000, height: 100 })).toEqual([18, 14, 10]);
		expect(geometry.orderedNodes([10, 18]).map((item) => item.node_id)).toEqual([18, 10]);
	});
	it('removes retired widgets and admits new structural parameters under a reused owner', () => {
		const geometry = new FreeSurfaceGeometry();
		const before = graphFixture(2);
		geometry.sync(before, before.nodesById.get(1)!, metrics);
		const after = forkGraph(before);
		after.nodesById.set(1, { ...before.nodesById.get(1)!, children: [10] });
		after.parentById.delete(14);
		const widget = { ...before.nodesById.get(10)!, children: [11, 12, 13, 30] };
		after.nodesById.set(10, widget);
		const folder = node(30, 'folder', [31]);
		after.nodesById.set(30, folder);
		after.parentById.set(30, 10);
		after.nodesById.set(31, param(31, 'anchor', { kind: 'enum', value: 'bottom-right' }));
		after.parentById.set(31, 30);
		indexStructuralFixture(after);
		geometry.sync(after, after.nodesById.get(1)!, metrics);
		expect(geometry.size).toBe(1);
		expect(geometry.get(14)).toBeUndefined();
		expect(geometry.get(10)).toEqual({ left: 1008, top: 786, width: 192, height: 64 });
		const moved = changeParam(after, 31, { kind: 'enum', value: 'center' });
		geometry.sync(moved, moved.nodesById.get(1)!, metrics);
		expect(geometry.updatedEntries).toBe(1);
		expect(geometry.get(10)?.left).toBe(504);
	});
	it('recomputes unit/anchor geometry for viewport, surface and font changes', () => {
		const geometry = new FreeSurfaceGeometry();
		let graph = graphFixture(1);
		graph = changeParam(graph, 12, { kind: 'css_value', value: 25, unit: 'percent' });
		graph = changeParam(graph, 13, { kind: 'css_value', value: 10, unit: 'vh' });
		geometry.sync(graph, graph.nodesById.get(1)!, metrics);
		expect(geometry.get(10)).toEqual({ left: 0, top: 0, width: 300, height: 120 });
		geometry.sync(graph, graph.nodesById.get(1)!, {
			...metrics,
			surfaceWidthPx: 800,
			viewportHeightPx: 900
		});
		expect(geometry.get(10)).toEqual({ left: 0, top: 0, width: 200, height: 90 });
		graph = changeParam(graph, 12, { kind: 'css_value', value: -1, unit: 'rem' });
		geometry.sync(graph, graph.nodesById.get(1)!, { ...metrics, rootRemPx: 20 });
		expect(geometry.get(10)?.width).toBe(10);
	});
	it('keeps strict rectangle intersection at shared edges', () => {
		const geometry = new FreeSurfaceGeometry();
		const graph = graphFixture(1);
		geometry.sync(graph, graph.nodesById.get(1)!, metrics);
		expect(geometry.query({ left: 192, top: 0, width: 10, height: 10 })).toEqual([]);
		expect(geometry.query({ left: 191, top: 0, width: 10, height: 10 })).toEqual([10]);
		geometry.clear();
		expect(geometry.size).toBe(0);
		expect(geometry.query({ left: -1, top: -1, width: 1000, height: 1000 })).toEqual([]);
		geometry.sync(graph, graph.nodesById.get(1)!, metrics);
		expect(geometry.get(10)?.width).toBe(192);
	});
	it('separates local em sizing from root rem sizing', () => {
		let graph = graphFixture(1);
		graph = changeParam(graph, 12, { kind: 'css_value', value: 10, unit: 'em' });
		graph = changeParam(graph, 13, { kind: 'css_value', value: 2, unit: 'rem' });
		const geometry = new FreeSurfaceGeometry();
		geometry.sync(graph, graph.nodesById.get(1)!, metrics);
		expect(geometry.get(10)).toEqual({ left: 0, top: 0, width: 120, height: 32 });
		geometry.sync(graph, graph.nodesById.get(1)!, { ...metrics, emPx: 20 });
		expect(geometry.get(10)).toEqual({ left: 0, top: 0, width: 200, height: 32 });
		geometry.sync(graph, graph.nodesById.get(1)!, { ...metrics, rootRemPx: 20 });
		expect(geometry.get(10)).toEqual({ left: 0, top: 0, width: 120, height: 40 });
	});
});
