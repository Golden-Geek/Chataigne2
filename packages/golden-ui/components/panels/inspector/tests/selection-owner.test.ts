import { test } from 'vitest';
import assert from 'node:assert/strict';
import { IdentifiedNodeMap } from '../../../../store/graph-node-index';
import { VersionedNodeMap } from '../../../../store/graph-index';
import { fixtureNode } from '../../../../tests/browser/fixture/session';
import { createInspectorSelectionOwner } from '../selection-owner';
import { createCurveSelectionProjection, curveSelectionNodes } from '../nodes/curve-selection';
import type { SelectionGraph } from '../../../../store/selection-hierarchy';

function scene(): SelectionGraph {
	const nodes = [
		fixtureNode(0, 'Root'),
		fixtureNode(1, 'Curve', [2, 7], 'animation_curve'),
		fixtureNode(2, 'A', [3, 5], 'animation_curve_key'),
		fixtureNode(3, 'Position', [], 'parameter'),
		fixtureNode(5, 'Easing', [6], 'animation_curve_easing'),
		fixtureNode(6, 'Kind', [], 'parameter'),
		fixtureNode(7, 'B', [], 'animation_curve_key'),
		fixtureNode(20, 'Other', [21], 'animation_curve'),
		fixtureNode(21, 'Other key', [], 'animation_curve_key')
	];
	return {
		rootId: 0,
		nodesById: new IdentifiedNodeMap(nodes.map((node) => [node.node_id, node])),
		parentById: new VersionedNodeMap([
			[1, 0],
			[2, 1],
			[3, 2],
			[5, 2],
			[6, 5],
			[7, 1],
			[20, 0],
			[21, 20]
		])
	};
}

test('specialized owners anchor direct, descendant and multi-selection while mixed owners retain ordinary semantics', () => {
	const graph = scene(),
		select = createInspectorSelectionOwner((node) => node.node_type === 'animation_curve');
	for (const ids of [[1], [2], [3], [5], [6], [2, 7], [1, 2], [999, 7]])
		assert.equal(select.read(graph, ids), 1);
	assert.equal(select.read(graph, [2, 21]), null);
	assert.equal(select.read(graph, [0]), 0);
	assert.equal(select.read(graph, []), null);
	assert.equal(select.read(null, [2]), undefined);
});

test('ownership caches ignore unrelated parameter values and invalidate topology, identity and registry revisions', () => {
	let graph = scene(),
		calls = 0;
	const ids = [2, 7],
		select = createInspectorSelectionOwner((node) => {
			calls++;
			return node.node_type === 'animation_curve';
		});
	assert.equal(select.read(graph, ids, 0), 1);
	const before = calls;
	graph = { ...graph, nodesById: graph.nodesById.fork(), parentById: graph.parentById.fork() };
	graph.nodesById.set(3, {
		...graph.nodesById.get(3)!,
		meta: { ...graph.nodesById.get(3)!.meta, label: 'Changed value' }
	});
	assert.equal(select.read(graph, ids, 0), 1);
	assert.equal(calls, before, 'an unselected value update does not rescan all selected keys');
	graph = { ...graph, parentById: graph.parentById.fork() };
	graph.parentById.set(7, 20);
	assert.equal(select.read(graph, ids, 0), null);
	assert(calls > before);
	const after = calls;
	select.read(graph, ids, 1);
	assert(calls > after);
	graph = { ...graph, parentById: graph.parentById.fork() };
	graph.parentById.set(2, 2);
	assert.equal(select.read(graph, [2], 1), null, 'cycles cannot acquire an owner');
});

test('curve focus distinguishes keys, key fields and easing fields without changing another editor', () => {
	const graph = scene(),
		focus = createCurveSelectionProjection();
	assert.deepEqual(focus.read(graph, [3, 7], 1, 'curve'), { key: 2, keys: [2, 7], segments: [] });
	assert.deepEqual(focus.read(graph, [6], 1, 'curve'), { key: null, keys: [], segments: [2] });
	assert.deepEqual(focus.read(graph, [1], 1, 'curve'), { key: null, keys: [], segments: [] });
	assert.equal(focus.read(graph, [21], 1, 'curve'), null);
	assert.deepEqual(curveSelectionNodes(graph, 1, { key: 7, keys: [2, 7], segments: [] }), [7, 2]);
	assert.deepEqual(curveSelectionNodes(graph, 1, { key: null, keys: [], segments: [2] }), [5]);
	assert.deepEqual(curveSelectionNodes(graph, 1, { key: null, keys: [], segments: [] }), [1]);
	assert.equal(curveSelectionNodes(graph, 1, { key: 21, keys: [21], segments: [] }), null);
});

test('large same-owner selection uses graph deltas instead of rescanning on each live parameter edit', () => {
	let graph = scene(),
		calls = 0;
	const ids = Array.from({ length: 20000 }, (_, index) => index + 100);
	for (const id of ids) {
		graph.nodesById.set(id, fixtureNode(id, 'Key', [], 'animation_curve_key'));
		graph.parentById.set(id, 1);
	}
	const select = createInspectorSelectionOwner((node) => {
		calls++;
		return node.node_type === 'animation_curve';
	});
	assert.equal(select.read(graph, ids), 1);
	const first = calls;
	for (let update = 0; update < 20; update++) {
		graph = { ...graph, nodesById: graph.nodesById.fork() };
		graph.nodesById.set(3, { ...graph.nodesById.get(3)!, uuid: `parameter-${update}` });
		assert.equal(select.read(graph, ids), 1);
	}
	assert.equal(calls, first);
});
