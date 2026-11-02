import { outline } from '../composition/presentation.ts';
import type { SurfaceSnapshot } from '../screens/generated/SurfaceSnapshot';
import type { ChannelReference } from './generated/ChannelReference';
import type { CompositionSnapshot } from './generated/CompositionSnapshot';
import type { Extent2D } from './generated/Extent2D';
import type { Point } from './generated/Point';

const epsilon = 32 * Number.EPSILON;

function validExtent(extent: Extent2D): boolean {
	return (
		Number.isFinite(extent.width) &&
		Number.isFinite(extent.height) &&
		extent.width > 0 &&
		extent.height > 0
	);
}

function insideBounds(point: Point, width: number, height: number): boolean {
	return (
		Number.isFinite(point.x) &&
		Number.isFinite(point.y) &&
		point.x >= 0 &&
		point.y >= 0 &&
		point.x <= width &&
		point.y <= height
	);
}

/** Convex outline picking is presentation only. Normalize locally so both unit-screen
 * coordinates and large transformed canvas coordinates use the same edge tolerance. */
function insideQuad(point: Point, corners: readonly Point[]): boolean {
	if (
		corners.length !== 4 ||
		corners.some((corner) => !Number.isFinite(corner.x) || !Number.isFinite(corner.y))
	)
		return false;
	let minX = Infinity,
		minY = Infinity,
		maxX = -Infinity,
		maxY = -Infinity;
	for (const corner of corners) {
		minX = Math.min(minX, corner.x);
		minY = Math.min(minY, corner.y);
		maxX = Math.max(maxX, corner.x);
		maxY = Math.max(maxY, corner.y);
	}
	const scale = Math.max(maxX - minX, maxY - minY);
	if (!Number.isFinite(scale) || scale <= 0) return false;
	const slack = epsilon * scale;
	if (
		point.x < minX - slack ||
		point.x > maxX + slack ||
		point.y < minY - slack ||
		point.y > maxY + slack
	)
		return false;
	let winding = 0;
	for (let index = 0; index < corners.length; index++) {
		const a = corners[index],
			b = corners[(index + 1) % 4],
			c = corners[(index + 2) % 4];
		const edgeX = (b.x - a.x) / scale,
			edgeY = (b.y - a.y) / scale;
		const turn = edgeX * ((c.y - b.y) / scale) - edgeY * ((c.x - b.x) / scale);
		if (!Number.isFinite(turn) || Math.abs(turn) <= epsilon) return false;
		const direction = Math.sign(turn);
		if (winding !== 0 && winding !== direction) return false;
		winding = direction;
		const side = edgeX * ((point.y - a.y) / scale) - edgeY * ((point.x - a.x) / scale);
		if (!Number.isFinite(side) || side * direction < -epsilon) return false;
	}
	return true;
}

/** Pick the top editor outline at a normalized Screen point, including objects whose
 * interactive DOM outlines were omitted by the overlay's bounded presentation window. */
export function pickSurfaceSource(
	point: Point,
	surfaces: readonly SurfaceSnapshot[]
): string | null {
	if (!insideBounds(point, 1, 1)) return null;
	for (let index = surfaces.length - 1; index >= 0; index--) {
		const surface = surfaces[index];
		if (insideQuad(point, surface.definition.quad.corners)) return surface.reference.surface;
	}
	return null;
}

/** Source dimensions are observed render metadata. No extent means there is no known body;
 * explicit DOM handles may still provide a replacement target independently. */
export function pickCompositionSource(
	point: Point,
	composition: Pick<CompositionSnapshot, 'canvas' | 'instances'>,
	sourceExtent: (source: ChannelReference) => Extent2D | null
): string | null {
	if (
		!validExtent(composition.canvas) ||
		!insideBounds(point, composition.canvas.width, composition.canvas.height)
	)
		return null;
	const extents = new Map<string, Extent2D | null>();
	for (let index = composition.instances.length - 1; index >= 0; index--) {
		const instance = composition.instances[index];
		const source = instance.definition.source;
		if (!source) continue;
		let extent = extents.get(source.channel);
		if (extent === undefined) {
			extent = sourceExtent(source);
			extents.set(source.channel, extent);
		}
		if (
			extent &&
			validExtent(extent) &&
			insideQuad(point, outline(instance.definition.transform, extent))
		)
			return instance.reference.instance;
	}
	return null;
}
