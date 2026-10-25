import type { GraphState } from '../../../../store/graph.svelte';
import type { NodeId } from '../../../../types';
import { getDashboardLayoutKind } from '../dashboard-model';
import { isFreeLayoutTransformableWidget, sameNodeIdList } from './geometry';

/** A move preview describes one immutable source/path and both surface layouts. */
export function captureWidgetMove(
	graph: GraphState | null,
	source: NodeId | null,
	parent: NodeId | null,
	target: NodeId | null
) {
	if (!graph || source === null || parent === null || target === null) return null;
	const item = graph.nodesById.get(source);
	if (!item || !isFreeLayoutTransformableWidget(item) || graph.parentById.get(source) !== parent)
		return null;
	const owners = [...new Set([parent, target])].map((id) => {
		const node = graph.nodesById.get(id);
		return node &&
			(node.node_type === 'dashboard_page' || node.node_type === 'dashboard_widget_container')
			? {
					id,
					uuid: node.uuid,
					children: node.children,
					layout: getDashboardLayoutKind(graph, node)
				}
			: null;
	});
	if (owners.some((owner) => owner === null)) return null;
	return {
		source: { id: source, uuid: item.uuid },
		isCurrent(current: GraphState | null): boolean {
			return (
				!!current &&
				current.nodesById.get(source)?.uuid === item.uuid &&
				current.parentById.get(source) === parent &&
				owners.every((owner) => {
					if (!owner) return false;
					const node = current.nodesById.get(owner.id);
					return (
						node?.uuid === owner.uuid &&
						(node.children === owner.children || sameNodeIdList(node.children, owner.children)) &&
						getDashboardLayoutKind(current, node) === owner.layout
					);
				})
			);
		}
	};
}
