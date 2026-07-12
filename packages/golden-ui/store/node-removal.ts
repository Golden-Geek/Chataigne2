import type { GraphState } from './graph.svelte';
import { appState } from './workbench.svelte';
import type { NodeId, UiEditIntent, UiNodeDto } from '../types';

export interface NodeRemovalRequest {
	readonly graph: GraphState;
	readonly nodes: UiNodeDto[];
	sendIntent(intent: UiEditIntent): Promise<void>;
}

export type NodeRemovalGuard = (request: NodeRemovalRequest) => boolean | Promise<boolean>;

const removalGuards = new Set<NodeRemovalGuard>();

export const registerNodeRemovalGuard = (guard: NodeRemovalGuard): (() => void) => {
	removalGuards.add(guard);
	return () => {
		removalGuards.delete(guard);
	};
};

const orderedUniqueNodeIds = (nodeIds: readonly NodeId[]): NodeId[] => {
	const seen = new Set<NodeId>();
	const ordered: NodeId[] = [];
	for (const nodeId of nodeIds) {
		if (seen.has(nodeId)) continue;
		seen.add(nodeId);
		ordered.push(nodeId);
	}
	return ordered;
};

export const requestRemoveNodesById = async (
	nodeIds: readonly NodeId[],
	options?: {
		graph?: GraphState;
		sendIntent?: (intent: UiEditIntent) => Promise<void>;
	}
): Promise<boolean> => {
	const session = appState.session;
	const graph = options?.graph ?? session?.graph.state ?? null;
	const sendIntent =
		options?.sendIntent ??
		((intent: UiEditIntent): Promise<void> => {
			if (!session) return Promise.reject(new Error('no active session'));
			return session.sendIntent(intent);
		});
	if (!graph) return false;

	const ids = orderedUniqueNodeIds(nodeIds);
	const nodes = ids
		.map((nodeId) => graph.nodesById.get(nodeId) ?? null)
		.filter((node): node is UiNodeDto => node !== null);
	if (nodes.length === 0) return false;

	for (const guard of removalGuards) {
		if (!(await guard({ graph, nodes, sendIntent }))) {
			return false;
		}
	}

	await sendIntent(
		nodes.length === 1
			? { kind: 'removeNode', node: nodes[0].node_id }
			: { kind: 'removeNodes', nodes: nodes.map((node) => node.node_id) }
	);
	return true;
};
