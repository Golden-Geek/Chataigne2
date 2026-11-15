import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cropOverlays } from '../crop-overlays.ts';
const corners = [
	{ x: 0.2, y: 0.2 },
	{ x: 0.8, y: 0.2 },
	{ x: 0.8, y: 0.8 },
	{ x: 0.2, y: 0.8 }
];

test('clearing crop selection withdraws native handle paint and hit IDs while retaining its selectable contour', () => {
	const selected = cropOverlays(corners, true, corners[0], 16);
	assert.deepEqual(
		selected.flatMap((item) => item.hit ?? []),
		['crop', 'crop:0', 'crop:1', 'crop:2', 'crop:3']
	);
	const cleared = cropOverlays(corners, false, corners[0], 16);
	assert.equal(cleared.length, 2, 'neither stale corner paint nor a pending ghost remains');
	assert.deepEqual(
		cleared.flatMap((item) => item.hit ?? []),
		['crop']
	);
	assert.equal(cleared[1].fill[3], 0);
	assert.deepEqual(cleared[1].shape, selected[1].shape, 'selection cannot change crop geometry');
});
