import { describe, expect, it } from 'vitest';
import {
	cameraAtZoom,
	cameraForBounds,
	graphBoundsForNodes,
	visibleViewport
} from '../camera-geometry';
import type { GraphNode } from '../types';

const node = (id: number, x: number): GraphNode => ({
	id: String(id),
	label: String(id),
	position: { x, y: -2 },
	inputs: [],
	outputs: []
});

describe('generic graph camera geometry', () => {
	it('frames 150k nodes without spreading them into function arguments', () => {
		const nodes = Array.from({ length: 150_000 }, (_, index) => node(index, index));
		const bounds = graphBoundsForNodes(
			nodes,
			() => 13,
			() => 8
		);
		expect(bounds).toEqual({ left: 0, top: -2, right: 150_012, bottom: 6 });
		const camera = cameraForBounds(bounds!, visibleViewport({}, 1600, 1000), 16, 3, 0.2, 2.5);
		expect(camera).not.toBeNull();
		expect(camera?.zoom).toBe(0.2);
		expect(Number.isFinite(camera?.x)).toBe(true);
		expect(Number.isFinite(camera?.y)).toBe(true);
	});

	it('returns no bounds for an empty graph and rejects nonfinite framing bounds', () => {
		expect(
			graphBoundsForNodes(
				[],
				() => 13,
				() => 8
			)
		).toBeNull();
		expect(
			cameraForBounds(
				{ left: Number.NaN, top: 0, right: 1, bottom: 1 },
				visibleViewport({}, 100, 100),
				16,
				3,
				0.2,
				2.5
			)
		).toBeNull();
	});

	it('scales excessive insets while keeping a nonempty visible viewport', () => {
		expect(visibleViewport({ left: 80, right: 80, top: 30, bottom: 30 }, 100, 50)).toEqual({
			x: 50,
			y: 25,
			width: 1,
			height: 1
		});
	});

	it('keeps the anchored world point fixed during zoom', () => {
		const current = { x: 40, y: -20, zoom: 1 };
		const next = cameraAtZoom(current, 2, 140, 80, 0.2, 2.5);
		expect(next).toEqual({ x: -60, y: -120, zoom: 2 });
		expect(cameraAtZoom(current, 100, 140, 80, 0.2, 2.5).zoom).toBe(2.5);
	});

	it('centers reversed bounds inside the inset viewport', () => {
		const viewport = visibleViewport({ left: 20, right: 0 }, 220, 120);
		const camera = cameraForBounds(
			{ left: 10, top: 5, right: 0, bottom: -5 },
			viewport,
			10,
			1,
			0.2,
			2.5
		);
		expect(camera).toEqual({ x: 70, y: 60, zoom: 1 });
	});
});
