import { getContext, setContext } from 'svelte';
import type { AlignmentGuide } from 'golden_ui';
import type { OverlayPrimitive } from 'golden_video_ui';
import type { Point } from '../media/generated/Point';
import { handleOverlays, imageFrameOverlay, overlayRem, quadOverlay } from '../preview/overlays';

const spatialPaint = Symbol('spatial viewport paint');
type Paint = (overlays: OverlayPrimitive[]) => void;

export function provideSpatialPaint(write: Paint | null) {
	setContext(spatialPaint, write);
}

export function useSpatialPaint(): Paint | null {
	return getContext<Paint | null>(spatialPaint) ?? null;
}

// The contour ID also keeps its paint history stable when selection changes stacking order.
type Contour = NonNullable<Parameters<typeof quadOverlay>[2]> & {
	points: readonly Point[];
	hit: string;
};

export function spatialOverlays(
	canvas: { width: number; height: number },
	contours: readonly Contour[],
	handles: readonly Point[],
	ghost: Point | null,
	guides: readonly AlignmentGuide[]
): OverlayPrimitive[] {
	const rem = overlayRem();
	const normalized = ({ x, y }: Point): Point => ({ x: x / canvas.width, y: y / canvas.height });
	return [
		imageFrameOverlay(rem),
		...contours.map(({ points, ...appearance }) =>
			quadOverlay(points.map(normalized), rem, appearance)
		),
		...guides.map(({ axis, value }) => {
			const start = normalized(axis === 'x' ? { x: value, y: 0 } : { x: 0, y: value });
			const end = axis === 'x' ? { x: start.x, y: 1 } : { x: 1, y: start.y };
			return {
				...quadOverlay([start, end, end, start], rem, { color: '70aaffcc' }),
				dash: [0.25 * rem, 0.2 * rem] as [number, number]
			};
		}),
		...handles.flatMap((point) => handleOverlays(normalized(point), rem)),
		...(ghost ? handleOverlays(normalized(ghost), rem, false, true) : [])
	];
}
