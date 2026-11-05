import type { Transform2D } from '../media/generated/Transform2D';
import type { Extent2D } from '../media/generated/Extent2D';
import type { Point } from '../media/generated/Point';
import type { SpatialResizeHandle } from '../media/generated/SpatialResizeHandle';
import { SPATIAL_RESIZE_HANDLES } from '../spatial/handles.ts';

/** Presentation only: source extent comes from the renderer, never from a UI domain default. */
export function outline(transform: Transform2D, extent: Extent2D): Point[] {
	const angle = (transform.rotation_degrees * Math.PI) / 180;
	const cos = Math.cos(angle),
		sin = Math.sin(angle);
	return [
		[0, 0],
		[extent.width, 0],
		[extent.width, extent.height],
		[0, extent.height]
	].map(([x, y]) => {
		const localX = (x - transform.pivot[0] * extent.width) * transform.scale[0];
		const localY = (y - transform.pivot[1] * extent.height) * transform.scale[1];
		return {
			x: transform.position[0] + cos * localX - sin * localY,
			y: transform.position[1] + sin * localX + cos * localY
		};
	});
}

/** Project real source bounds; hit-target reach is applied in rem by the view. */
export function gizmo(transform: Transform2D, extent?: Extent2D | null) {
	const pivot = { x: transform.position[0], y: transform.position[1] };
	const resize: { name: SpatialResizeHandle; point: Point; cursor: string }[] = [];
	const rotate: { name: SpatialResizeHandle; point: Point; offset: Point }[] = [];
	if (!extent) return { pivot, resize, rotate };
	const corners = outline(transform, extent);
	const center = { x: (corners[0].x + corners[2].x) / 2, y: (corners[0].y + corners[2].y) / 2 };
	const names = SPATIAL_RESIZE_HANDLES.map((handle) => handle.name);
	for (let index = 0; index < 8; index++) {
		const first = corners[Math.floor(index / 2)];
		const next = corners[(Math.floor(index / 2) + 1) % 4];
		const point = index % 2 ? { x: (first.x + next.x) / 2, y: (first.y + next.y) / 2 } : first;
		const dx = point.x - center.x,
			dy = point.y - center.y;
		const direction = ((Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) % 4) + 4) % 4;
		resize.push({
			name: names[index],
			point,
			cursor: ['ew-resize', 'nwse-resize', 'ns-resize', 'nesw-resize'][direction]
		});
		if (index % 2 === 0) {
			const length = Math.hypot(dx, dy);
			rotate.push({ name: names[index], point, offset: { x: dx / length, y: dy / length } });
		}
	}
	return { pivot, resize, rotate };
}
