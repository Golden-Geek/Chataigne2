import type { Transform2D } from '../media/generated/Transform2D';
import type { Extent2D } from '../media/generated/Extent2D';
import type { Point } from '../media/generated/Point';

/** Presentation only: source extent comes from the renderer, never from a UI domain default. */
export function outline(transform: Transform2D, extent: Extent2D): Point[] {
	const cos = Math.cos(transform.rotation),
		sin = Math.sin(transform.rotation);
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

/** Handle reach depends on the displayed canvas, independently of unavailable source dimensions. */
export function gizmo(transform: Transform2D, canvas: Extent2D, extent?: Extent2D | null) {
	const radius = Math.min(canvas.width, canvas.height) * 0.06;
	const cos = Math.cos(transform.rotation),
		sin = Math.sin(transform.rotation);
	const point = (x: number, y: number) => ({
		x: transform.position[0] + cos * x - sin * y,
		y: transform.position[1] + sin * x + cos * y
	});
	const corner = extent
		? point(
				((transform.pivot[0] < 0.5 ? 1 : 0) - transform.pivot[0]) *
					extent.width *
					transform.scale[0],
				((transform.pivot[1] < 0.5 ? 1 : 0) - transform.pivot[1]) *
					extent.height *
					transform.scale[1]
			)
		: null;
	return {
		radius: radius / 8,
		pivot: point(0, 0),
		rotate: point(0, -radius * 1.8),
		scale: corner ?? point(radius, radius)
	};
}
