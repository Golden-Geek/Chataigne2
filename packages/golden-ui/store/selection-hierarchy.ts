import type { GraphState } from './graph.svelte';
import type { NodeId, UiNodeDto } from '../types';

export type SelectionGraph = Pick<GraphState, 'rootId' | 'nodesById' | 'parentById'>;
export interface SelectionHierarchy {
	node(id: NodeId): UiNodeDto | undefined;
	parent(id: NodeId): NodeId | undefined;
}

/** Cache ownership against immutable selection arrays and published copy-on-write graph deltas. */
export function createHierarchySelection<T>(
	project: (hierarchy: SelectionHierarchy, ids: readonly NodeId[]) => T
) {
	let previous: SelectionGraph | null = null;
	let selected: readonly NodeId[] | null = null;
	let scope: unknown;
	let captured = new Set<NodeId>();
	let result: T;
	return {
		read(graph: SelectionGraph | null, ids: readonly NodeId[], identity?: unknown): T | undefined {
			if (!graph) {
				previous = null;
				selected = null;
				return undefined;
			}
			let changed =
				previous === null ||
				selected !== ids ||
				identity !== scope ||
				previous.rootId !== graph.rootId;
			if (!changed && previous) {
				graph.nodesById.forEachChange(previous.nodesById, (id, next, before) => {
					if (captured.has(id) && next !== before) changed = true;
				});
				graph.parentById.forEachChange(previous.parentById, (id) => {
					if (captured.has(id)) changed = true;
				});
			}
			if (changed) {
				captured = new Set();
				result = project(
					{
						node(id) {
							captured.add(id);
							return graph.nodesById.get(id);
						},
						parent(id) {
							captured.add(id);
							return graph.parentById.get(id);
						}
					},
					ids
				);
			}
			previous = graph;
			selected = ids;
			scope = identity;
			return result;
		}
	};
}

/** Return a bounded ancestry path; cycles and missing nodes do not acquire an owner. */
export function selectionPath(hierarchy: SelectionHierarchy, id: NodeId): UiNodeDto[] | null {
	const path: UiNodeDto[] = [],
		seen = new Set<NodeId>();
	let current: NodeId | undefined = id;
	while (current !== undefined && path.length < 128) {
		if (seen.has(current)) return null;
		seen.add(current);
		const node = hierarchy.node(current);
		if (!node) return null;
		path.push(node);
		current = hierarchy.parent(current);
	}
	return current === undefined ? path : null;
}
