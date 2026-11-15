import { createHierarchySelection, selectionPath } from '../../../store/selection-hierarchy';
import type { NodeId, UiNodeDto } from '../../../types';

/** Keep one specialized owner mounted when all selected descendants belong to that owner. */
export function createInspectorSelectionOwner(anchors: (node: UiNodeDto) => boolean) {
	return createHierarchySelection((hierarchy, ids): NodeId | null => {
		let owner: NodeId | undefined;
		for (const id of ids) {
			const path = selectionPath(hierarchy, id);
			if (!path) continue;
			const candidate = path.find(anchors)?.node_id ?? id;
			if (owner !== undefined && owner !== candidate) return null;
			owner = candidate;
		}
		return owner ?? null;
	});
}
