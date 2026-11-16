import type { NodeId } from '../../../../types';
import type { CurveSelection } from '../../../common/curve/types';
import {
	createHierarchySelection,
	selectionPath,
	type SelectionGraph
} from '../../../../store/selection-hierarchy';

export const curveSelectionSignature = (selection: CurveSelection) =>
	JSON.stringify([selection.key, selection.keys, selection.segments]);

export function createCurveSelectionProjection() {
	let owner: NodeId;
	const projection = createHierarchySelection((hierarchy, ids): CurveSelection | null => {
		const keys = new Set<NodeId>(),
			segments = new Set<NodeId>();
		let relevant = false;
		for (const id of ids) {
			const path = selectionPath(hierarchy, id);
			if (!path?.some((node) => node.node_id === owner)) continue;
			relevant = true;
			const key = path.find((node) => node.node_type === 'animation_curve_key');
			if (!key) continue;
			if (path.some((node) => node.node_type === 'animation_curve_easing'))
				segments.add(key.node_id);
			else keys.add(key.node_id);
		}
		return relevant
			? { key: [...keys][0] ?? null, keys: [...keys], segments: [...segments] }
			: null;
	});
	return {
		read(graph: SelectionGraph | null, ids: readonly NodeId[], id: NodeId, identity: string) {
			owner = id;
			return projection.read(graph, ids, identity);
		}
	};
}

/** Resolve only real owned keys/easing nodes; selection never creates or edits document data. */
export function curveSelectionNodes(
	graph: SelectionGraph,
	owner: NodeId,
	selection: CurveSelection
): NodeId[] | null {
	const ids: NodeId[] = [];
	for (const key of selection.keys) {
		if (
			graph.parentById.get(key) !== owner ||
			graph.nodesById.get(key)?.node_type !== 'animation_curve_key'
		)
			return null;
		ids.push(key);
	}
	if (selection.key !== null && ids.includes(selection.key)) {
		ids.splice(ids.indexOf(selection.key), 1);
		ids.unshift(selection.key);
	}
	for (const key of selection.segments) {
		if (graph.parentById.get(key) !== owner) return null;
		const easing = graph.nodesById
			.get(key)
			?.children.map((id) => graph.nodesById.get(id))
			.find((node) => node?.node_type === 'animation_curve_easing');
		if (!easing) return null;
		ids.push(easing.node_id);
	}
	return ids.length ? [...new Set(ids)] : [owner];
}
