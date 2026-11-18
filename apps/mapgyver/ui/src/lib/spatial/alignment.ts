import { ViewportAlignment, type AlignmentGuide, type ViewBounds } from 'golden_ui';
import type { Point } from '../media/generated/Point';

/** One bounded candidate page, frozen at pointer-down; final geometry still goes through Rust. */
export function createGeometryAlignment(
	element: () => Element | undefined,
	enabled: () => boolean,
	changed: (guides: AlignmentGuide[]) => void
) {
	let session: ViewportAlignment | null = null;
	let extent = { width: 1, height: 1 },
		rem = 16;
	return {
		begin(
			candidates: { id: string; bounds: ViewBounds | null }[],
			excluded: string,
			canvas: { width: number; height: number },
			project: (point: Point) => ViewBounds | null,
			options: { centers?: boolean } = {}
		) {
			if (!enabled()) {
				session = null;
				changed([]);
				return;
			}
			extent = canvas;
			rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
			session = new ViewportAlignment(
				[
					{ x: 0, y: 0, width: canvas.width, height: canvas.height },
					...candidates.flatMap((entry) =>
						entry.id !== excluded && entry.bounds ? [entry.bounds] : []
					)
				],
				project,
				options
			);
			changed([]);
		},
		move(point: Point) {
			const rect = element()?.getBoundingClientRect();
			if (!enabled() || !session || !rect || rect.width <= 0 || rect.height <= 0) {
				session?.clear();
				changed([]);
				return point;
			}
			const result = session.snap(point, {
				x: (0.5 * rem * extent.width) / rect.width,
				y: (0.5 * rem * extent.height) / rect.height
			});
			changed(result.guides);
			return result.point;
		},
		clear() {
			session = null;
			changed([]);
		}
	};
}
