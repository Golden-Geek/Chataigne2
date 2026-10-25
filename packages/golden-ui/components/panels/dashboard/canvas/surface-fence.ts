import type { GraphState } from '../../../../store/graph.svelte';
import type { NodeId, UiNodeDto } from '../../../../types';
import { getDashboardLayoutKind } from '../dashboard-model';
import { sameNodeIdList } from './geometry';

export function captureSurface(graph: GraphState | null, id: NodeId) {
	const node = graph?.nodesById.get(id);
	if (
		!graph ||
		!node ||
		(node.node_type !== 'dashboard_page' && node.node_type !== 'dashboard_widget_container')
	)
		return null;
	const { uuid, children } = node;
	const layout = getDashboardLayoutKind(graph, node);
	return {
		isCurrent(current: GraphState | null): boolean {
			const owner = current?.nodesById.get(id);
			return (
				!!current &&
				owner?.uuid === uuid &&
				(owner.children === children || sameNodeIdList(owner.children, children)) &&
				getDashboardLayoutKind(current, owner) === layout
			);
		}
	};
}

/** Retire a drop when its source identity or its destination presentation changes. */
export function captureSurfaceDrop(graph: GraphState | null, owner: NodeId, source: UiNodeDto) {
	const surface = captureSurface(graph, owner);
	if (!surface || graph?.nodesById.get(source.node_id)?.uuid !== source.uuid) return null;
	return {
		isCurrent(current: GraphState | null) {
			return (
				current?.nodesById.get(source.node_id)?.uuid === source.uuid && surface.isCurrent(current)
			);
		}
	};
}
