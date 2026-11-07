import type { Extent2D } from '../media/generated/Extent2D';
import type { MatrixLayoutCell } from '../media/generated/MatrixLayoutCell';
import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
import type { LayerPresentation } from './layer-gesture';
import type { CellPresentation } from './cell-gesture';
import { SPATIAL_RESIZE_HANDLES } from './handles.ts';
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

/** Unrounded control-space hint only. Never use this rectangle as an authored edit basis. */
export function displayRect(
	object: LayoutRect,
	draft: LayerPresentation | CellPresentation | null
): LayoutRect {
	if (
		!draft ||
		('layer' in draft.reference ? draft.reference.layer : draft.reference.cell) !== object.id
	)
		return object;
	const handle = draft.handle;
	const mode =
		'mode' in handle
			? handle.mode
			: handle.kind === 'Translate'
				? 'Translate'
				: { Resize: { handle: handle.handle } };
	const origin =
		'kind' in handle && handle.kind === 'Translate'
			? { position: handle.origin, size: object.size }
			: handle.origin;
	const dx = draft.sample.x - handle.start.x,
		dy = draft.sample.y - handle.start.y;
	if (mode === 'Translate')
		return {
			...object,
			position: [origin.position[0] + dx, origin.position[1] + dy],
			size: origin.size
		};
	const anchor = SPATIAL_RESIZE_HANDLES.find((entry) => entry.name === mode.Resize.handle);
	if (!origin.size || !anchor) return object;
	const width = origin.size.width + (anchor.x === 0.5 ? 0 : dx * (2 * anchor.x - 1));
	const height = origin.size.height + (anchor.y === 0.5 ? 0 : dy * (2 * anchor.y - 1));
	// SVG rectangles require positive extents. Domain limits/rounding/rejection stay in Rust.
	if (width <= 0 || height <= 0 || !Number.isFinite(width + height)) return object;
	return {
		...object,
		position: [
			origin.position[0] + (anchor.x === 0 ? dx : 0),
			origin.position[1] + (anchor.y === 0 ? dy : 0)
		],
		size: { width, height }
	};
}

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
