import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
import type { Point } from './generated/Point';
import type { SurfacePresentation } from './surface-gesture';

/** Pending control vertices only; persistence, crop bounds and mapping validity stay authoritative. */
export function surfaceQuad(
	surface: SurfaceSnapshot,
	draft: SurfacePresentation | null,
	extent?: { width: number; height: number } | null
): readonly Point[] {
	const committed = surface.definition.quad.corners;
	if (!draft || draft.reference.surface !== surface.reference.surface) return committed;
	const handle = draft.handle,
		point = draft.sample;
	if (handle.kind === 'QuadTranslation')
		return handle.quad.corners.map((corner) => ({
			x: corner.x + point.x - handle.origin.x,
			y: corner.y + point.y - handle.origin.y
		}));
	if (handle.kind === 'QuadCorner')
		return committed.map((corner, index) => (index === handle.corner ? point : corner));
	if (handle.kind === 'QuadCornerPixels' && extent)
		return committed.map((corner, index) =>
			index === handle.corner
				? {
						x: handle.origin.x + point.x / extent.width,
						y: handle.origin.y + point.y / extent.height
					}
				: corner
		);
	return committed;
}

export function cropCorners(surface: SurfaceSnapshot, draft: SurfacePresentation | null): Point[] {
	const crop = surface.definition.crop;
	const corners = [
		{ x: crop.x, y: crop.y },
		{ x: crop.x + crop.width, y: crop.y },
		{ x: crop.x + crop.width, y: crop.y + crop.height },
		{ x: crop.x, y: crop.y + crop.height }
	];
	if (!draft || draft.reference.surface !== surface.reference.surface) return corners;
	const { handle, sample } = draft;
	if (handle.kind === 'CropOrigin')
		return corners.map((_point, index) => ({
			x: sample.x + (index === 1 || index === 2 ? crop.width : 0),
			y: sample.y + (index >= 2 ? crop.height : 0)
		}));
	if (handle.kind !== 'CropCorner') return corners;
	const left = handle.corner === 0 || handle.corner === 3;
	const top = handle.corner === 0 || handle.corner === 1;
	return corners.map((point, index) => ({
		x: (index === 0 || index === 3) === left ? sample.x : point.x,
		y: (index === 0 || index === 1) === top ? sample.y : point.y
	}));
}
