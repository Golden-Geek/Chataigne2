import { describe, expect, it, vi } from 'vitest';
import type { GraphNode, GraphSocket } from 'golden_graph_ui';
import { node, parameterNode } from '../../../tests/fixtures/workbench-graph';
import { canConnectGraphConnection, graphSocketsByRef, toGraphEdges } from '../alchemistGraph';

const component: GraphSocket = {
	id: 'result.x',
	label: 'X',
	valueType: 'float',
	color: 'red',
	parentId: 'result',
	component: 'x'
};
const input: GraphSocket = { id: 'value1', label: 'Value', valueType: 'float', color: 'blue' };
const graphNodes: GraphNode[] = [
	{
		id: '20',
		label: 'Source',
		position: { x: 0, y: 0 },
		inputs: [],
		outputs: [{ id: 'result', label: 'Result', valueType: 'vec3', children: [component] }]
	},
	{ id: '30', label: 'Target', position: { x: 24, y: 0 }, inputs: [input], outputs: [] }
];
const connection = {
	from: { nodeId: '20', socketId: 'result.x' },
	to: { nodeId: '30', socketId: 'value1' }
};

describe('Formula socket projection reuse', () => {
	it('uses one projected socket index for component connection admission', () => {
		const sockets = graphSocketsByRef(graphNodes);
		expect(sockets.get('20:result.x')).toBe(component);
		expect(canConnectGraphConnection(sockets, connection)).toBe(true);
		expect(
			canConnectGraphConnection(sockets, {
				...connection,
				from: { nodeId: '20', socketId: 'missing' }
			})
		).toBe(false);
		sockets.set('30:value1', { ...input, compatible: false });
		expect(canConnectGraphConnection(sockets, connection)).toBe(false);
	});

	it('reads edge identities from backend records and colors from the existing socket projection', () => {
		const formula = { ...node(0), node_type: 'alchemist_formula', children: [20, 30, 1] };
		const source = { ...node(20), node_type: 'alchemist_anode', children: [10_000] };
		const target = { ...node(30), node_type: 'alchemist_anode', children: [20_000] };
		const edge = { ...node(1), node_type: 'alchemist_connection', children: [2, 3, 4, 5] };
		const records = new Map(
			[
				formula,
				source,
				target,
				edge,
				parameterNode(2, 'source_node', { kind: 'reference', uuid: source.uuid }),
				parameterNode(3, 'target_node', { kind: 'reference', uuid: target.uuid }),
				parameterNode(4, 'source_socket', { kind: 'str', value: 'result.x' }),
				parameterNode(5, 'target_socket', { kind: 'str', value: 'value1' })
			].map((node) => [node.node_id, node])
		);
		const get = vi.spyOn(records, 'get');
		const edges = toGraphEdges(
			formula,
			records,
			graphSocketsByRef(graphNodes),
			new Set(['20:result.x'])
		);
		expect(edges).toEqual([
			{ id: '1', ...connection, color: 'red', targetColor: 'blue', active: true }
		]);
		expect(get.mock.calls.every(([id]) => id < 10_000)).toBe(true);
	});
});
