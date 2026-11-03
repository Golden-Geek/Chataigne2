import type { MatrixItemId } from '../media/generated/MatrixItemId';
import type { MatrixItemDropTarget } from '../media/generated/MatrixItemDropTarget';
import type { MatrixViewRow } from '../media/generated/MatrixViewRow';

export type GridDrag = { kind: 'Item'; item: MatrixItemId } | { kind: 'Column'; column: string };

// Pointer fractions are presentation hints. The backend resolves parents and complete order.
export function rowDropTarget(row: MatrixViewRow, fraction: number): MatrixItemDropTarget {
	if (row.item.id.kind === 'Group' && fraction >= 1 / 3 && fraction <= 2 / 3)
		return { kind: 'Inside', group: row.item.id.id };
	return { kind: fraction < 0.5 ? 'Before' : 'After', item: row.item.id };
}
