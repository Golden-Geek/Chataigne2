import type { NodeId } from '../types';
import type { GraphState } from './graph.svelte';

/** Presentation lookup index: local declarations exclude independently owned item subtrees. */
export const createStructuralChildrenWork = (state: GraphState) => {
	const dirty = new Set<NodeId>();
	const appends = new Map<NodeId, NodeId[][]>();
	let parents: SetIterator<NodeId> | undefined;
	let appended: MapIterator<[NodeId, NodeId[][]]> | undefined;
	let parent: NodeId | undefined;
	let children: Iterator<NodeId>;
	let structural: NodeId[] = [];
	let prefix: NodeId[] = [];
	return {
		invalidate(id: NodeId) {
			dirty.add(id);
		},
		append(id: NodeId, children: NodeId[]) {
			const chunks = appends.get(id) ?? [];
			chunks.push(children);
			appends.set(id, chunks);
		},
		advance(): boolean {
			parents ??= dirty.values();
			if (parent === undefined) {
				const entry = parents.next();
				if (entry.done) {
					appended ??= appends.entries();
					const extra = appended.next();
					if (extra.done) return true;
					if (dirty.has(extra.value[0])) return false;
					parent = extra.value[0];
					prefix = state.structuralChildrenById.get(parent) ?? [];
					children = (function* () {
						for (const chunk of extra.value[1]) yield* chunk;
					})();
				} else {
					parent = entry.value;
					const source = state.childrenById.get(parent);
					if (!source) {
						state.structuralChildrenById.delete(parent);
						parent = undefined;
						return false;
					}
					children = source.values();
					prefix = [];
				}
				structural = [];
				return false;
			}
			const child = children.next();
			if (!child.done) {
				const node = state.nodesById.get(child.value);
				if (node && node.user_role !== 'itemRoot') structural.push(child.value);
				return false;
			}
			const result = structural.length ? prefix.concat(structural) : prefix;
			if (result.length) state.structuralChildrenById.set(parent, result);
			else state.structuralChildrenById.delete(parent);
			parent = undefined;
			return false;
		}
	};
};

/** Synchronous reducers visit only changed parents; staged transactions budget each child. */
export const updateStructuralChildren = (before: GraphState, next: GraphState): void => {
	const work = createStructuralChildrenWork(next);
	next.childrenById.forEachChange(before.childrenById, (id) => work.invalidate(id));
	next.nodesById.forEachChange(before.nodesById, (id, node, previous) => {
		if (node?.user_role === previous?.user_role) return;
		for (const parent of [before.parentById.get(id), next.parentById.get(id)])
			if (parent !== undefined) work.invalidate(parent);
	});
	while (!work.advance()) {
		// Ordinary scalar publications never enter a sibling scan.
	}
};
