import type { Extent2D } from '../media/generated/Extent2D';
import type { MatrixLayoutCell } from '../media/generated/MatrixLayoutCell';
import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
export interface LayoutRect {
	id: string;
	label: string;
	position: [number, number];
	size: Extent2D | null;
	enabled: boolean;
}

/** Renderer observations can advance after the bounded document projection was read. */
export const cellExtent = (cell: MatrixLayoutCell, observed?: Extent2D | null) =>
	cell.view.cell.definition.settings.size ?? observed ?? cell.extent;

/** SVG paints last on top; use backend ordinals even for independently read selections. */
export function matrixLayerPaintRows(
	snapshot: MatrixViewportSnapshot,
	selected: MatrixViewRow | null
) {
	const indexed = snapshot.rows.map((row, offset) => ({ row, index: snapshot.row_start + offset }));
	const independent = snapshot.selected;
	if (
		selected &&
		independent?.row_index != null &&
		independent.row?.item.id.id === selected.item.id.id &&
		!indexed.some(({ row }) => row.item.id.id === selected.item.id.id)
	) {
		indexed.push({ row: selected, index: independent.row_index });
	}
	return indexed.sort((a, b) => b.index - a.index).map(({ row }) => row);
}
