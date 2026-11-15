import { test } from 'vitest';
import assert from 'node:assert/strict';
import { fixtureNode } from '../../../../tests/browser/fixture/session';
import { IdentifiedNodeMap } from '../../../../store/graph-node-index';
import { VersionedNodeMap } from '../../../../store/graph-index';
import {
	createGradientSelectionProjection,
	gradientSelectionNodes
} from '../nodes/gradient-selection';

const scene = () => ({
	rootId: 0,
	nodesById: new IdentifiedNodeMap([
		[0, fixtureNode(0, 'Project', [1, 10])],
		[1, fixtureNode(1, 'Gradient', [2, 5], 'gradient')],
		[2, fixtureNode(2, 'Stop', [3], 'gradient_stop')],
		[3, fixtureNode(3, 'Position', [], 'parameter')],
		[5, fixtureNode(5, 'Other stop', [], 'gradient_stop')],
		[10, fixtureNode(10, 'Other Gradient', [11], 'gradient')],
		[11, fixtureNode(11, 'Foreign stop', [], 'gradient_stop')]
	]),
	parentById: new VersionedNodeMap([
		[1, 0],
		[2, 1],
		[3, 2],
		[5, 1],
		[10, 0],
		[11, 10]
	])
});

test('Gradient owner, stop and typed field project real identities without acquiring foreign children', () => {
	const graph = scene(),
		project = createGradientSelectionProjection(),
		owner = graph.nodesById.get(1)!;
	assert.deepEqual(project(graph, [1], owner), { stop: null });
	assert.deepEqual(project(graph, [2], owner), { stop: 2 });
	assert.deepEqual(project(graph, [3], owner), { stop: 2 });
	assert.deepEqual(project(graph, [5, 2], owner), { stop: 5 });
	assert.equal(project(graph, [11], owner), null);
	assert.deepEqual(gradientSelectionNodes(graph, 1, { stop: 2 }), [2]);
	assert.deepEqual(gradientSelectionNodes(graph, 1, { stop: null }), [1]);
	assert.equal(gradientSelectionNodes(graph, 1, { stop: 11 }), null);
	assert.equal(gradientSelectionNodes(graph, 1, { stop: 3 }), null);
});

test('Gradient focus retains unchanged projections and invalidates moved ancestry and owner scope', () => {
	let graph = scene();
	const project = createGradientSelectionProjection(),
		owner = graph.nodesById.get(1)!,
		ids = [3];
	const first = project(graph, ids, owner);
	graph = { ...graph, nodesById: graph.nodesById.fork() };
	graph.nodesById.set(11, {
		...graph.nodesById.get(11)!,
		meta: { ...graph.nodesById.get(11)!.meta, label: 'Changed' }
	});
	assert.equal(project(graph, ids, owner), first);
	graph = { ...graph, parentById: graph.parentById.fork() };
	graph.parentById.set(2, 10);
	assert.equal(project(graph, ids, owner), null);
	assert.deepEqual(project(graph, ids, graph.nodesById.get(10)!), { stop: 2 });
});
