import assert from 'node:assert/strict';
import { test } from 'node:test';
import { acceptedWarp, cropCorners, warpDraft, warpOutlines } from '../gesture-presentation.ts';
import type { SurfaceSnapshot } from '../generated/SurfaceSnapshot';
import type { SurfacePresentation, SurfaceHandle } from '../surface-gesture';
import type { ScreenSnapshot } from '../generated/ScreenSnapshot';
import type { ScreenHandleGroup } from '../generated/ScreenHandleGroup';
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
const screen: ScreenSnapshot = {
	id: 'screen',
	label: 'Screen',
	definition: {
		enabled: true,
		target: null,
		anti_aliasing: 'Raw',
		frame_rate: { kind: 'FollowDisplay' }
	},
	surfaces: [surface]
};
test('Screen quad control points and pixel hints advance without writing committed geometry', () => {
	const before = structuredClone(surface);
	const outline = (draft: SurfacePresentation, extent?: { width: number; height: number }) =>
		warpOutlines(screen, warpDraft(draft, extent), []).get(surface.reference.surface)!;
	assert.deepEqual(outline(hint({ kind: 'QuadCorner', corner: 0 }))[0], {
		x: 0.3,
		y: 0.4
	});
	const translation = hint({
		kind: 'QuadTranslation',
		quad: surface.definition.quad,
		origin: { x: 0.1, y: 0.1 }
	});
	assert(Math.abs(outline(translation)[2].x - 1.2) < 1e-9);
	const pixels = {
		...hint({ kind: 'QuadCornerPixels', corner: 0, origin: { x: 0.1, y: 0.1 } }),
		sample: { x: 20, y: 10 }
	};
	const corner = outline(pixels, { width: 100, height: 100 })[0];
	assert(Math.abs(corner.x - 0.3) < 1e-9 && Math.abs(corner.y - 0.2) < 1e-9);
	assert.deepEqual(surface, before);
});
test('Linked outlines share the latest vertices and the final accepted geometry', () => {
	const peer = { ...surface, reference: { ...surface.reference, surface: 'peer' } };
	const another = { ...surface, reference: { ...surface.reference, surface: 'another' } };
	const foreign = { ...surface, reference: { screen: 'elsewhere', surface: 'foreign' } };
	const linked = { ...screen, surfaces: [surface, peer, another, foreign] };
	const group = (corner: number, members: ScreenHandleGroup['members']): ScreenHandleGroup => ({
		corner,
		missing: false,
		members: [{ reference: surface.reference, corner }, ...members]
	});
	const groups = [
		group(0, [
			{ reference: peer.reference, corner: 1 },
			{ reference: another.reference, corner: 2 },
			{ reference: foreign.reference, corner: 0 }
		]),
		group(1, [{ reference: peer.reference, corner: 3 }])
	];
	const draft = hint({
		kind: 'QuadTranslation',
		origin: { x: 0, y: 0 },
		quad: surface.definition.quad
	});
	const outlines = warpOutlines(linked, warpDraft(draft), groups);
	const original = surface.definition.quad.corners;
	assert.deepEqual(outlines.get('peer'), [
		original[0],
		draft.sample,
		original[2],
		{ x: 1.3, y: 0.4 }
	]);
	assert.deepEqual(outlines.get('another')?.[2], draft.sample);
	assert(!outlines.has('foreign'));
	const confirmed = structuredClone(surface);
	confirmed.definition.quad.corners[0] = { x: 0.2, y: 0.3 };
	const acknowledged = warpOutlines(
		linked,
		acceptedWarp(confirmed, { kind: 'QuadCorner', corner: 0 }),
		groups
	);
	assert.deepEqual(acknowledged.get('peer')?.[1], confirmed.definition.quad.corners[0]);
	assert.deepEqual(acknowledged.get('peer')?.[3], original[3]);
	assert.deepEqual(peer.definition.quad.corners, original);
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
