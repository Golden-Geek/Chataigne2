import assert from 'node:assert/strict';
import { test } from 'node:test';
import { surfaceQuad, cropCorners } from '../gesture-presentation.ts';
import type { SurfaceSnapshot } from '../generated/SurfaceSnapshot';
import type { SurfacePresentation, SurfaceHandle } from '../surface-gesture';
const surface: SurfaceSnapshot = {
	reference: { screen: 'screen', surface: 'surface' },
	label: 'Surface',
	mapping_valid: true,
	definition: {
		source: null,
		crop: { x: 0.1, y: 0.2, width: 0.5, height: 0.4 },
		quad: {
			corners: [
				{ x: 0, y: 0 },
				{ x: 1, y: 0 },
				{ x: 1, y: 1 },
				{ x: 0, y: 1 }
			]
		},
		opacity: 1,
		enabled: true,
		calibration: 'Off'
	}
};
const hint = (handle: SurfaceHandle): SurfacePresentation => ({
	reference: surface.reference,
	handle,
	sample: { x: 0.3, y: 0.4 }
});
test('Screen quad control points and pixel hints advance without writing committed geometry', () => {
	const before = structuredClone(surface);
	assert.deepEqual(surfaceQuad(surface, hint({ kind: 'QuadCorner', corner: 0 }))[0], {
		x: 0.3,
		y: 0.4
	});
	const translation = hint({
		kind: 'QuadTranslation',
		quad: surface.definition.quad,
		origin: { x: 0.1, y: 0.1 }
	});
	assert(Math.abs(surfaceQuad(surface, translation)[2].x - 1.2) < 1e-9);
	const pixels = {
		...hint({ kind: 'QuadCornerPixels', corner: 0, origin: { x: 0.1, y: 0.1 } }),
		sample: { x: 20, y: 10 }
	};
	const corner = surfaceQuad(surface, pixels, { width: 100, height: 100 })[0];
	assert(Math.abs(corner.x - 0.3) < 1e-9 && Math.abs(corner.y - 0.2) < 1e-9);
	assert.deepEqual(surface, before);
});
test('Source crop hints preserve the opposite edge and leave clamping/validation in the backend', () => {
	const a = cropCorners(surface, hint({ kind: 'CropCorner', corner: 0 }));
	assert.deepEqual(a[0], { x: 0.3, y: 0.4 });
	assert.deepEqual(a[2], { x: 0.6, y: 0.6000000000000001 });
	const move = { ...hint({ kind: 'CropOrigin' }), sample: { x: 0.9, y: 0.8 } };
	assert(
		Math.abs(cropCorners(surface, move)[2].x - 1.4) < 1e-9,
		'visual hints do not own crop constraints'
	);
	const late = structuredClone(surface);
	late.definition.crop.x = 0.2;
	assert.deepEqual(cropCorners(surface, move), cropCorners(late, move));
});
