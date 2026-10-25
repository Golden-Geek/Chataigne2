import type { GraphState } from '../../../../store/graph.svelte';
import type { NodeId, UiNodeDto } from '../../../../types';

/** One retained authored order; pointer queries address only mounted anchors. */
export class SurfaceItemOrder {
	#graph: GraphState | null = null;
	#children: readonly NodeId[] | null = null;
	#uuid: string | null = null;
	#indices = new Map<NodeId, number>();
	get size() {
		return this.#indices.size;
	}
	clear() {
		this.#graph = null;
		this.#children = null;
		this.#uuid = null;
		this.#indices.clear();
	}
	sync(graph: GraphState, surface: UiNodeDto) {
		let changed = surface.children !== this.#children || surface.uuid !== this.#uuid;
		if (!changed && this.#graph)
			graph.nodesById.forEachChange(this.#graph.nodesById, (id, node, before) => {
				if (
					node?.user_role !== before?.user_role &&
					(graph.parentById.get(id) === surface.node_id ||
						this.#graph?.parentById.get(id) === surface.node_id)
				)
					changed = true;
			});
		if (changed) {
			this.#indices = new Map();
			for (const id of surface.children)
				if (graph.nodesById.get(id)?.user_role === 'itemRoot')
					this.#indices.set(id, this.#indices.size);
		}
		this.#children = surface.children;
		this.#uuid = surface.uuid;
		this.#graph = graph;
	}
	index(id: NodeId, excluded: ReadonlySet<NodeId>): number | undefined {
		if (excluded.has(id)) return undefined;
		const at = this.#indices.get(id);
		if (at === undefined) return undefined;
		let removed = 0;
		for (const other of excluded) {
			const index = this.#indices.get(other);
			if (index !== undefined && index < at) removed++;
		}
		return at - removed;
	}
	count(excluded: ReadonlySet<NodeId>): number {
		let result = this.size;
		for (const id of excluded) if (this.#indices.has(id)) result--;
		return result;
	}
}
