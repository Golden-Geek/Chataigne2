import type { GraphState } from '../../../../store/graph.svelte';
import type { NodeId, UiNodeDto } from '../../../../types';
import type { FreeLayoutTarget } from './contracts';
import { getDirectParamNode } from '../dashboard-model';
import { captureSurface } from './surface-fence';

/** Capture only edited targets/parameters and their unique ancestry; inspect later map deltas. */
export function captureLayoutGesture(
	graph: GraphState | null,
	targets: readonly FreeLayoutTarget[]
) {
	if (!graph || !targets.length) return null;
	const nodes = new Map<
		NodeId,
		{ uuid: string; parent: NodeId | undefined; type: string; declaration: string }
	>();
	const surfaces = new Map<NodeId, NonNullable<ReturnType<typeof captureSurface>>>();
	const layouts = new Map<NodeId, NodeId>();
	const capture = (id: NodeId) => {
		const node = graph.nodesById.get(id);
		if (!node) return false;
		nodes.set(id, {
			uuid: node.uuid,
			parent: graph.parentById.get(id),
			type: node.node_type,
			declaration: node.decl_id
		});
		return true;
	};
	const expanded = new Set<NodeId>();
	for (const target of targets) {
		for (const id of [
			target.positionParamNodeId,
			target.widthParamNodeId,
			target.heightParamNodeId
		])
			if (id !== null) {
				const path = new Set<NodeId>();
				let field: NodeId | undefined = id;
				while (field !== target.nodeId) {
					if (field === undefined || path.has(field) || !capture(field)) return null;
					if (graph.nodesById.get(field)?.user_role === 'itemRoot') return null;
					path.add(field);
					field = graph.parentById.get(field);
				}
			}
		const path = new Set<NodeId>();
		let id: NodeId | undefined = target.nodeId;
		while (id !== undefined) {
			if (path.has(id)) return null;
			if (expanded.has(id)) break;
			path.add(id);
			expanded.add(id);
			if (!capture(id)) return null;
			const surface = captureSurface(graph, id);
			if (surface) {
				surfaces.set(id, surface);
				const layout = getDirectParamNode(graph, graph.nodesById.get(id)!, 'layout_kind');
				if (layout) {
					if (!capture(layout.node_id)) return null;
					layouts.set(layout.node_id, id);
				}
			}
			id = graph.parentById.get(id);
		}
	}
	let previous = graph,
		valid = true;
	const checkNode = (current: GraphState, id: NodeId, node: UiNodeDto | undefined) => {
		const expected = nodes.get(id);
		if (
			expected &&
			(node?.uuid !== expected.uuid ||
				node.node_type !== expected.type ||
				node.decl_id !== expected.declaration)
		)
			valid = false;
		const surface = surfaces.get(layouts.get(id) ?? id);
		if (surface && !surface.isCurrent(current)) valid = false;
	};
	return {
		isCurrent(current: GraphState | null): boolean {
			if (!valid || !current) return false;
			current.nodesById.forEachChange(previous.nodesById, (id, node) =>
				checkNode(current, id, node)
			);
			current.parentById.forEachChange(previous.parentById, (id, parent) => {
				const expected = nodes.get(id);
				if (expected && parent !== expected.parent) valid = false;
			});
			previous = current;
			return valid;
		}
	};
}
