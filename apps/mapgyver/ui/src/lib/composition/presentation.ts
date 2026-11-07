import type { Transform2D } from '../media/generated/Transform2D';
import type { Extent2D } from '../media/generated/Extent2D';
import type { Point } from '../media/generated/Point';
import type { SpatialResizeHandle } from '../media/generated/SpatialResizeHandle';
import { SPATIAL_RESIZE_HANDLES } from '../spatial/handles.ts';
import type { InstanceSnapshot } from '../media/generated/InstanceSnapshot';
import type { InstancePresentation } from './instance-gesture';

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
	return gizmoFromOutline(pivot, extent ? outline(transform, extent) : []);
}

function gizmoFromOutline(pivot: Point, corners: Point[]) {
	const resize: { name: SpatialResizeHandle; point: Point; cursor: string }[] = [];
	const rotate: { name: SpatialResizeHandle; point: Point; offset: Point }[] = [];
	if (!corners.length) return { pivot, resize, rotate };
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
		if (index % 2 === 0 && Math.hypot(dx, dy) > 0) {
			const length = Math.hypot(dx, dy);
			rotate.push({ name: names[index], point, offset: { x: dx / length, y: dy / length } });
		}
	}
	return { pivot, resize, rotate };
}

/** Transform only viewport points. No authored Transform2D is produced or validated here. */
export function instanceDisplay(
	instance: InstanceSnapshot,
	extent: Extent2D | null | undefined,
	draft: InstancePresentation | null
) {
	if (!draft || draft.reference.instance !== instance.reference.instance) {
		const transform = instance.definition.transform;
		return { points: extent ? outline(transform, extent) : [], handles: gizmo(transform, extent) };
	}
	const { origin, start, mode } = draft.handle;
	const target = draft.sample;
	const dx = target.x - start.x,
		dy = target.y - start.y;
	const pivot = { x: origin.position[0], y: origin.position[1] };
	const basis = typeof mode === 'object' ? mode.Resize.extent : extent;
	let project = (point: Point): Point => ({ x: point.x + dx, y: point.y + dy });
	if (mode === 'Rotate') {
		const a = { x: start.x - pivot.x, y: start.y - pivot.y };
		const b = { x: target.x - pivot.x, y: target.y - pivot.y };
		const angle = Math.atan2(a.x * b.y - a.y * b.x, a.x * b.x + a.y * b.y);
		project = (point) => ({
			x: pivot.x + Math.cos(angle) * (point.x - pivot.x) - Math.sin(angle) * (point.y - pivot.y),
			y: pivot.y + Math.sin(angle) * (point.x - pivot.x) + Math.cos(angle) * (point.y - pivot.y)
		});
	} else if (typeof mode === 'object' && basis) {
		const anchor = SPATIAL_RESIZE_HANDLES.find((entry) => entry.name === mode.Resize.handle)!;
		const angle = (origin.rotation_degrees * Math.PI) / 180;
		const c = Math.cos(angle),
			s = Math.sin(angle);
		const width = basis.width * origin.scale[0],
			height = basis.height * origin.scale[1];
		const x = (1 - anchor.x - origin.pivot[0]) * width,
			y = (1 - anchor.y - origin.pivot[1]) * height;
		const fixed = { x: pivot.x + c * x - s * y, y: pivot.y + s * x + c * y };
		const sx = anchor.x === 0.5 ? 1 : 1 + ((c * dx + s * dy) * (2 * anchor.x - 1)) / width;
		const sy = anchor.y === 0.5 ? 1 : 1 + ((-s * dx + c * dy) * (2 * anchor.y - 1)) / height;
		project = (point) => {
			const x = point.x - fixed.x,
				y = point.y - fixed.y;
			const a = (c * x + s * y) * sx,
				b = (-s * x + c * y) * sy;
			return { x: fixed.x + c * a - s * b, y: fixed.y + s * a + c * b };
		};
	}
	const points = basis ? outline(origin, basis).map(project) : [];
	return { points, handles: gizmoFromOutline(project(pivot), points) };
}
