import type { NodeId, UiNodeDto } from '../../types';

export type NodeSelectionIdentity = Pick<UiNodeDto, 'node_id' | 'uuid'>;

/** One authoritative selection request, resolved by projection events rather than a timer. */
export const createPendingNodeSelection = () => {
	let pending: { rootUuid: string; nodes: NodeSelectionIdentity[] } | null = null;

	return {
		request(nodes: readonly NodeSelectionIdentity[], rootUuid: string): void {
			pending = { rootUuid, nodes: nodes.map(({ node_id, uuid }) => ({ node_id, uuid })) };
		},
		cancel(): void {
			pending = null;
		},
		takeIfReady(
			nodes: ReadonlyMap<NodeId, NodeSelectionIdentity>,
			rootUuid: string | undefined
		): NodeId[] | undefined {
			if (!pending) return undefined;
			if (pending.rootUuid !== rootUuid) {
				pending = null;
				return undefined;
			}
			let missing = false;
			for (const identity of pending.nodes) {
				const projected = nodes.get(identity.node_id);
				if (!projected) {
					missing = true;
				} else if (projected.uuid !== identity.uuid) {
					pending = null;
					return undefined;
				}
			}
			if (missing) return undefined;
			const selected = [...new Set(pending.nodes.map((node) => node.node_id))];
			pending = null;
			return selected;
		}
	};
};
