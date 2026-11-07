import type { GraphState } from './graph.svelte';

/** Inspect only changed Parameter paths in persistent indices, never enumerate a document
 * or infer an app's child structure. Owners are the bounded backend-resolved view objects. */
export class EditorParameterObserver {
	private previous: GraphState | null = null;
	private owners = '';
	reset() {
		this.previous = null;
		this.owners = '';
	}
	observe(graph: GraphState, owners: readonly string[], maximumDepth = 256): boolean {
		const roots = new Set(owners),
			key = JSON.stringify([maximumDepth, ...roots]);
		const previous = this.previous;
		this.previous = graph;
		if (!previous || key !== this.owners) {
			this.owners = key;
			return false;
		}
		let changed = false;
		const belongs = (state: GraphState, start: number) => {
			let id: number | undefined = start;
			const seen = new Set<number>();
			while (id !== undefined && !seen.has(id) && seen.size <= maximumDepth) {
				seen.add(id);
				const node = state.nodesById.get(id);
				if (node && roots.has(node.uuid)) return true;
				id = state.parentById.get(id);
			}
			// A malformed/extreme hierarchy cannot make the observer miss an edit.
			return maximumDepth === 256 && id !== undefined && !seen.has(id);
		};
		graph.paramsById.forEachChange(previous.paramsById, (id) => {
			if (changed) return;
			if (Object.is(graph.paramsById.get(id)?.value, previous.paramsById.get(id)?.value)) return;
			changed = belongs(graph, id) || belongs(previous, id);
		});
		return changed;
	}
}
