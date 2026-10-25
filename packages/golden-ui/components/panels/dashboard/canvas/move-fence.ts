import type { GraphState } from '../../../../store/graph.svelte';
import type { NodeId } from '../../../../types';
import { isFreeLayoutTransformableWidget } from './geometry';
import { captureSurface } from './surface-fence';

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
	const owners = [...new Set([parent, target])].map((id) => captureSurface(graph, id));
	if (owners.some((owner) => owner === null)) return null;
	return {
		source: { id: source, uuid: item.uuid },
		isCurrent(current: GraphState | null): boolean {
			return (
				!!current &&
				current.nodesById.get(source)?.uuid === item.uuid &&
				current.parentById.get(source) === parent &&
				owners.every((owner) => owner?.isCurrent(current))
			);
		}
	};
}
