import { describe, expect, it } from 'vitest';
import { buildFreeLayoutResizePreview } from '../geometry';
import type { FreeLayoutInteraction, FreeLayoutResizeEdges } from '../contracts';
import { sample } from './gesture-fixtures';

function interaction(edge: keyof FreeLayoutResizeEdges): FreeLayoutInteraction {
	const target = sample().previews[0].target;
	target.startRect = { left: 2, top: 3, width: 100, height: 80 };
	target.startPreview = {
		position: { x: 2, y: 3 },
		size: { width: { value: 100, unit: 'px' }, height: { value: 80, unit: 'px' } }
	};
	return {
		mode: 'resize',
		resizeEdges: { left: false, right: false, top: false, bottom: false, [edge]: true },
		primaryNodeId: 10,
		sourceSurfaceNodeId: 1,
		selectionRect: target.startRect,
		targets: [target],
		startClient: [0, 0],
		rootRemPx: 16,
		emPx: 16,
		surfaceWidthPx: 1200,
		surfaceHeightPx: 850,
		surfaceScaleX: 1,
		surfaceScaleY: 1,
		viewportWidthPx: 1680,
		viewportHeightPx: 1200
	};
}
describe('free resize edge snapping', () => {
	it.each([
		['right', 2, 3, 110, 80],
		['bottom', 2, 3, 100, 93],
		['left', 16, 3, 86, 80],
		['top', 2, 16, 100, 67]
	] as const)('snaps the %s edge without moving an unedited edge', (edge, x, y, width, height) => {
		const preview = buildFreeLayoutResizePreview(interaction(edge), 10, 10, 8)!.previews[0].preview;
		expect(preview.position).toEqual({ x, y });
		expect(preview.size.width.unit).toBe('px');
		expect(preview.size.height.unit).toBe('px');
		expect(preview.size.width.value).toBeCloseTo(width, 10);
		expect(preview.size.height.value).toBeCloseTo(height, 10);
	});
	it('preserves the fixed off-grid edge when the minimum size wins over snapping', () => {
		const preview = buildFreeLayoutResizePreview(interaction('left'), 200, 0, 8)!.previews[0]
			.preview;
		expect(preview.position).toEqual({ x: 94, y: 3 });
		expect(preview.size.width).toEqual({ value: 8, unit: 'px' });
		expect(preview.position.x + preview.size.width.value).toBe(102);
	});
});
