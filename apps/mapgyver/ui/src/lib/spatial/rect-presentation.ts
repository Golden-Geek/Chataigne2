import type { Extent2D } from '../media/generated/Extent2D';
import type { MatrixLayoutCell } from '../media/generated/MatrixLayoutCell';
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
