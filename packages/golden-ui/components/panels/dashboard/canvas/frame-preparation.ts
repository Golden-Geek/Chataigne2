import type { GraphState } from '../../../../store/graph.svelte';
import type { NodeId } from '../../../../types';

export interface FrameStep {
	readonly parent: NodeId;
	readonly child: NodeId;
}

/** Plan only known dashboard paths; materialization never edits the authored graph. */
export function framePreparation(graph: GraphState | null, page: NodeId, ids: readonly NodeId[]) {
	const paths = new Map<NodeId, readonly FrameStep[]>();
	const children = new Map<NodeId, Set<NodeId>>();
	const identities = new Map<NodeId, { uuid: string; children: readonly NodeId[] }>();
	let validatedVersion: GraphState['nodesById'] | null = null;
	const validated = new Map<NodeId, boolean>();
	const capture = (id: NodeId) => {
		const node = graph?.nodesById.get(id);
		if (node) identities.set(id, { uuid: node.uuid, children: node.children });
	};
	const currentNode = (source: GraphState | null, id: NodeId): boolean => {
		// Published graph versions are immutable; shared ancestor checks need only one validation.
		const version = source?.nodesById ?? null;
		if (version !== validatedVersion) {
			validatedVersion = version;
			validated.clear();
		}
		const cached = validated.get(id);
		if (cached !== undefined) return cached;
		const expected = identities.get(id),
			node = source?.nodesById.get(id);
		const result =
			!!expected &&
			!!node &&
			node.uuid === expected.uuid &&
			(node.children === expected.children ||
				(node.children.length === expected.children.length &&
					node.children.every((child, index) => child === expected.children[index])));
		validated.set(id, result);
		return result;
	};
	const isCurrent = (source: GraphState | null, targets: readonly NodeId[]) =>
		targets.every((id) => {
			const path = paths.get(id);
			if (!path) return true;
			return (
				currentNode(source, id) &&
				path.every(
					({ parent, child }) =>
						currentNode(source, parent) &&
						currentNode(source, child) &&
						source?.parentById.get(child) === parent
				)
			);
		});
	const currentChildren = (source: GraphState | null, parent: NodeId): NodeId[] =>
		currentNode(source, parent)
			? [...(children.get(parent) ?? [])].filter(
					(child) => currentNode(source, child) && source?.parentById.get(child) === parent
				)
			: [];
	if (!graph) return { paths, children, isCurrent, currentChildren };
	for (const id of ids) {
		const steps: FrameStep[] = [],
			seen = new Set<NodeId>();
		let child: NodeId | undefined = id;
		while (child !== undefined && child !== page && !seen.has(child)) {
			seen.add(child);
			const parent = graph.parentById.get(child);
			if (parent === undefined) break;
			if (graph.nodesById.get(child)?.user_role === 'itemRoot') steps.push({ parent, child });
			child = parent;
		}
		if (child !== page) continue;
		capture(id);
		capture(page);
		steps.reverse();
		paths.set(id, steps);
		for (const { parent, child } of steps) {
			capture(parent);
			capture(child);
			let members = children.get(parent);
			if (!members) children.set(parent, (members = new Set()));
			members.add(child);
		}
	}
	return { paths, children, isCurrent, currentChildren };
}
