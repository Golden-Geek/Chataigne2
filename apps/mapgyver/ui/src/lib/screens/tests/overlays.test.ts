import assert from 'node:assert/strict';
import test from 'node:test';
import { quadOverlay } from '../overlays.ts';

const quad = [
	{ x: 0, y: 0 },
	{ x: 1, y: 0 },
	{ x: 1, y: 1 },
	{ x: 0, y: 1 }
];
test('native Surface paint keeps unselected quads transparent, selected borders stronger and invalid outlines visible', () => {
	const plain = quadOverlay(quad, 16),
		selected = quadOverlay(quad, 16, { selected: true });
	assert.equal(plain.fill[3], 0);
	assert(selected.fill[3] > 0 && selected.fill[3] < 0.15);
	assert(selected.width > plain.width);
	const invalid = quadOverlay(quad, 16, { valid: false });
	assert.equal(invalid.fill[3], 0);
	assert.notDeepEqual(invalid.stroke, plain.stroke);
});
