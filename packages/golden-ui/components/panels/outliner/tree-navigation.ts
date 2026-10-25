import type { NodeId } from '../../../types';
import type { OutlinerRow } from './visible-rows';

export interface TreePosition {
	row: OutlinerRow;
	index: number;
	parentId: NodeId | null;
	position: number;
	siblings: number;
}

/** Index the visible projection once, including hoisted rows and filtered sibling groups. */
export function indexTreeRows(rows: readonly OutlinerRow[]): ReadonlyMap<NodeId, TreePosition> {
	const positions = new Map<NodeId, TreePosition>();
	const ancestors: OutlinerRow[] = [];
	const groups = new Map<NodeId | null, TreePosition[]>();
	rows.forEach((row, index) => {
		while (ancestors.length && ancestors.at(-1)!.indent >= row.indent) ancestors.pop();
		const parentId = ancestors.at(-1)?.nodeId ?? null;
		const group = groups.get(parentId) ?? [];
		const entry = { row, index, parentId, position: group.length + 1, siblings: 0 };
		group.push(entry);
		groups.set(parentId, group);
		positions.set(row.nodeId, entry);
		ancestors.push(row);
	});
	for (const group of groups.values()) for (const entry of group) entry.siblings = group.length;
	return positions;
}

export type TreeNavigationIntent =
	| { kind: 'focus'; nodeId: NodeId }
	| { kind: 'expand'; nodeId: NodeId; expanded: boolean }
	| { kind: 'activate'; nodeId: NodeId };

/** Navigation touches the indexed projection, never the full graph or mounted DOM subset. */
export function treeNavigationIntent(
	rows: readonly OutlinerRow[],
	positions: ReadonlyMap<NodeId, TreePosition>,
	nodeId: NodeId | null,
	key: string
): TreeNavigationIntent | null {
	const entry = nodeId === null ? undefined : positions.get(nodeId);
	if (!entry) return rows.length ? { kind: 'focus', nodeId: rows[0].nodeId } : null;
	const { row, index, parentId } = entry;
	const focus = (index: number): TreeNavigationIntent => ({
		kind: 'focus',
		nodeId: rows[Math.max(0, Math.min(index, rows.length - 1))].nodeId
	});
	switch (key) {
		case 'ArrowDown':
			return focus(index + 1);
		case 'ArrowUp':
			return focus(index - 1);
		case 'Home':
			return focus(0);
		case 'End':
			return focus(rows.length - 1);
		case 'ArrowRight':
			if (row.expanded === false) return { kind: 'expand', nodeId: row.nodeId, expanded: true };
			return rows[index + 1]?.indent > row.indent ? focus(index + 1) : null;
		case 'ArrowLeft':
			if (row.expanded === true) return { kind: 'expand', nodeId: row.nodeId, expanded: false };
			return parentId === null ? null : { kind: 'focus', nodeId: parentId };
		case 'Enter':
		case ' ':
			return { kind: 'activate', nodeId: row.nodeId };
		default:
			return null;
	}
}
