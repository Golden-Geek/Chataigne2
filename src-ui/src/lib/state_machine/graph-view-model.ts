import type { GraphEdge, GraphNode, GraphSocket } from 'golden_alchemist_ui';
import type { StateMachineStores } from './stores/stateMachineStores.svelte';

const socketColor = (valueType: string): string => {
	const normalized = valueType.toLowerCase();
	if (normalized.includes('bool') || normalized.includes('trigger')) return '#e4bd67';
	if (normalized.includes('float') || normalized.includes('int')) return '#69b9f2';
	if (normalized.includes('vec') || normalized.includes('color')) return '#b993f2';
	if (normalized.includes('state')) return '#67d69b';
	if (normalized.includes('command')) return '#ef8a72';
	return '#b4c2d8';
};

const toSocket = (
	stores: StateMachineStores,
	nodeId: string,
	socket: { id: string; label: string; value_type: string }
): GraphSocket => {
	const compatibility = stores.types.forSocket(nodeId, socket.id);
	return {
		id: socket.id,
		label: socket.label,
		valueType: socket.value_type,
		compatible: compatibility?.compatible ?? true,
		color: socketColor(socket.value_type)
	};
};

export const graphNodesFor = (stores: StateMachineStores): GraphNode[] => {
	const diagnosticNodes = new Set(
		[...stores.processors.diagnosticsById.values()]
			.map((diagnostic) => diagnostic.node_id)
			.filter((nodeId): nodeId is string => nodeId !== null)
	);
	return [...stores.graph.nodesById.values()].map((node) => ({
		id: node.id,
		label: node.label,
		subtitle: node.type_id,
		x: node.x,
		y: node.y,
		inputs: node.inputs.map((socket) => toSocket(stores, node.id, socket)),
		outputs: node.outputs.map((socket) => toSocket(stores, node.id, socket)),
		active: stores.debug.samplesByNode.has(node.id),
		invalid: diagnosticNodes.has(node.id)
	}));
};

export const graphEdgesFor = (stores: StateMachineStores): GraphEdge[] =>
	stores.graph.edges.map((edge, index) => ({
		id: `${edge.from_node}:${edge.from_socket}:${edge.to_node}:${edge.to_socket}:${index}`,
		from: {
			nodeId: edge.from_node,
			socketId: edge.from_socket
		},
		to: {
			nodeId: edge.to_node,
			socketId: edge.to_socket
		},
		active:
			stores.debug.samplesByNode.has(edge.from_node) ||
			stores.debug.samplesByNode.has(edge.to_node),
		invalid: stores.types.forSocket(edge.to_node, edge.to_socket)?.compatible === false
	}));
