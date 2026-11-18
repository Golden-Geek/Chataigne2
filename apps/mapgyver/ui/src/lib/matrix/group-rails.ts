import type { MatrixGroupSpan } from '../media/generated/MatrixGroupSpan';
import { gridSize, rowOffset, type projectGrid } from './viewport.ts';

/** Leave ancestor wraps outside the actual row card, including its selection contour. */
export function rowInset(depth: number) {
	return 0.25 + Math.min(depth * 0.8, gridSize.label * 0.4);
}

/** Decorative viewport geometry only; Group membership and intervals come from the backend. */
export function groupRail(
	span: MatrixGroupSpan,
	projection: ReturnType<typeof projectGrid>,
	height: number,
	depth: number
) {
	const origin = projection.rowOrigin - projection.top;
	const start =
		origin + rowOffset({ visible_index: span.start, groups_before: span.start_groups }, projection);
	const end =
		origin + rowOffset({ visible_index: span.end, groups_before: span.end_groups }, projection);
	const empty = span.end === span.start + 1;
	const first = start + (empty ? 0.35 : projection.groupRow - 0.1);
	const last = end - (empty ? 0.3 : 0.08);
	const top = Math.max(projection.header, first),
		bottom = Math.min(height, last);
	if (bottom <= top) return null;
	const startClipped = first < projection.header,
		endClipped = last > height;
	const spacing = Math.min(0.8, (gridSize.label * 0.4 - 0.4) / Math.max(1, depth + 1));
	const x = 0.35 + span.depth * spacing,
		right = gridSize.label - 0.2,
		radius = Math.min(0.45, (bottom - top) / 2);
	// Like Inspector containers: a tinted body with a left edge and a quiet lower return.
	const path =
		`M${x},${top}` +
		(endClipped
			? ` V${bottom}`
			: ` V${bottom - radius} Q${x},${bottom} ${x + radius},${bottom} H${right}`);
	const fill = `${path} H${right} V${top} Z`;
	return { path, fill, top, bottom, x, startClipped, endClipped };
}
