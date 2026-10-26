import { expect, it } from 'vitest';
import { createGraphStore } from 'golden_ui/store/graph.svelte';
import {
	node,
	parameterNode as field,
	snapshot
} from '../../../../src/lib/tests/fixtures/workbench-graph';
import { createNativeFormulaConnections } from '../formula-connections';

const fixture = () => [
	{ ...node(0), children: [1, 20, 30] },
	{ ...node(1), node_type: 'alchemist_connection', children: [2, 3, 4, 5] },
	field(2, 'source_node', { kind: 'reference', uuid: node(20).uuid }),
	field(3, 'target_node', { kind: 'reference', uuid: node(30).uuid }),
	field(4, 'source_socket', { kind: 'str', value: 'result' }),
	field(5, 'target_socket', { kind: 'str', value: 'value1' }),
	node(20),
	node(30)
];

it('observes parameter-only changes, removals and reused IDs without stale connection reads', () => {
	const store = createGraphStore(),
		read = createNativeFormulaConnections();
	const nodes = fixture();
	store.loadSnapshot({ ...snapshot(0, 0), nodes });
	const original = read(store.state, node(20).uuid)[0];
	expect(original).toMatchObject({ id: 1, target: node(30).uuid, targetSocket: 'value1' });
	const changed = {
		...store.state,
		nodesById: store.state.nodesById.fork(),
		paramsById: store.state.paramsById.fork()
	};
	for (const fieldNode of [
		field(2, 'source_node', { kind: 'reference', uuid: node(30).uuid }),
		field(5, 'target_socket', { kind: 'str', value: 'value2' })
	]) {
		changed.nodesById.set(fieldNode.node_id, fieldNode);
		if (fieldNode.data.kind === 'parameter')
			changed.paramsById.set(fieldNode.node_id, fieldNode.data.param);
	}
	expect(read(changed, node(20).uuid)).toEqual([]);
	expect(read(changed, node(30).uuid)[0]).toMatchObject({ targetSocket: 'value2' });
	const edge = read(changed, node(30).uuid)[0];
	const unrelated = { ...changed, nodesById: changed.nodesById.fork() };
	unrelated.nodesById.set(20, { ...node(20), meta: { ...node(20).meta, label: 'Renamed' } });
	expect(read(unrelated, node(30).uuid)[0]).toBe(edge);

	store.loadSnapshot({
		...snapshot(0, 1),
		nodes: [{ ...node(0), children: [20, 30] }, node(20), node(30)]
	});
	expect(read(store.state, node(30).uuid)).toEqual([]);

	const replacement = fixture();
	replacement[1] = { ...replacement[1], uuid: '10000000-0000-0000-0000-000000000001' };
	store.loadSnapshot({ ...snapshot(0, 2), project_generation: 2, nodes: replacement });
	expect(read(store.state, node(30).uuid)).toEqual([]);
	expect(read(store.state, node(20).uuid)[0]).toMatchObject({
		uuid: replacement[1].uuid,
		targetSocket: 'value1'
	});
	expect(original.uuid).toBe(node(1).uuid);
});
