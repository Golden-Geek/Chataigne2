import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
import type { Point } from './generated/Point';
import type { ScreenHandleGroup } from './generated/ScreenHandleGroup';
import type { ScreenSnapshot } from './generated/ScreenSnapshot';
import type { SurfaceId } from './generated/SurfaceId';
import type { SurfaceReference } from './generated/SurfaceReference';
import type { SurfaceHandle, SurfacePresentation } from './surface-gesture';

/** Pending control vertices only; persistence, crop bounds and mapping validity stay authoritative. */
export function warpDraft(
	draft: SurfacePresentation,
	extent?: { width: number; height: number } | null
): WarpVertices | null {
	const { handle, sample, reference } = draft;
	switch (handle.kind) {
		case 'QuadTranslation':
			return {
				reference,
				vertices: handle.quad.corners.map((point, index) => ({
					index,
					point: {
						x: point.x + sample.x - handle.origin.x,
						y: point.y + sample.y - handle.origin.y
					}
				}))
			};
		case 'QuadCorner':
			return { reference, vertices: [{ index: handle.corner, point: sample }] };
		case 'QuadCornerPixels':
			return extent
				? {
						reference,
						vertices: [
							{
								index: handle.corner,
								point: {
									x: handle.origin.x + sample.x / extent.width,
									y: handle.origin.y + sample.y / extent.height
								}
							}
						]
					}
				: null;
		default:
			return null;
	}
}

type WarpVertex = { index: number; point: Point };
export type WarpVertices = { reference: SurfaceReference; vertices: readonly WarpVertex[] };
export type WarpOutlines = ReadonlyMap<SurfaceId, readonly Point[]>;

/** The accepted geometry covers the gap between the gesture and the refreshed editor page. */
export function acceptedWarp(surface: SurfaceSnapshot, handle: SurfaceHandle): WarpVertices | null {
	const {
		reference,
		definition: {
			quad: { corners }
		}
	} = surface;
	if (handle.kind === 'QuadTranslation')
		return { reference, vertices: corners.map((point, index) => ({ index, point })) };
	if (handle.kind === 'QuadCorner' || handle.kind === 'QuadCornerPixels') {
		const point = corners[handle.corner];
		return point ? { reference, vertices: [{ index: handle.corner, point }] } : null;
	}
	return null;
}

/** Each affected outline uses the same vertices and the backend's admitted corner groups. */
export function warpOutlines(
	screen: ScreenSnapshot,
	warp: WarpVertices | null,
	groups: readonly ScreenHandleGroup[]
): WarpOutlines {
	const outlines = new Map<SurfaceId, Point[]>();
	if (!warp || warp.reference.screen !== screen.id) return outlines;
	const surfaces = new Map(screen.surfaces.map((surface) => [surface.reference.surface, surface]));
	function project(reference: SurfaceReference, { index, point }: WarpVertex) {
		if (reference.screen !== screen.id) return;
		const existing = surfaces.get(reference.surface)?.definition.quad.corners;
		if (!existing?.[index]) return;
		const corners = outlines.get(reference.surface) ?? [...existing];
		corners[index] = point;
		outlines.set(reference.surface, corners);
	}
	for (const vertex of warp.vertices) {
		project(warp.reference, vertex);
		const group = groups.find(
			({ corner, members, missing }) =>
				!missing &&
				corner === vertex.index &&
				members.some(
					({ reference, corner }) =>
						reference.screen === warp.reference.screen &&
						reference.surface === warp.reference.surface &&
						corner === vertex.index
				)
		);
		for (const member of group?.members ?? []) {
			if (member.reference.surface !== warp.reference.surface)
				project(member.reference, { index: member.corner, point: vertex.point });
		}
	}
	return outlines;
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
