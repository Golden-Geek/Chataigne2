import assert from 'node:assert/strict';
import { test } from 'node:test';
import { nearestHandle } from '../handle-snap.ts';
import { linkOverlays } from '../link-overlays.ts';
import { screenPanelState } from '../panel-state.ts';
import type { ScreenHandleCandidate } from '../generated/ScreenHandleCandidate';

const candidate = (screen: string, x: number, y: number): ScreenHandleCandidate => ({
	member: { reference: { screen: 'screen', surface: screen }, corner: 0 },
	parameter: screen,
	point: { x, y },
	label: screen,
	linkable: [true, false, false, false]
});

test('proximity uses displayed screen distance, zoom/aspect and backend eligibility', () => {
	const target = candidate('target', 0.5, 0.5);
	assert.equal(
		nearestHandle({ x: 0.495, y: 0.5 }, 0, [target], { width: 1000, height: 500 }, 10),
		target
	);
	assert.equal(
		nearestHandle({ x: 0.495, y: 0.5 }, 0, [target], { width: 3000, height: 1500 }, 10),
		null
	);
	assert.equal(
		nearestHandle({ x: 0.5, y: 0.48 }, 0, [target], { width: 1000, height: 1000 }, 10),
		null
	);
	assert.equal(
		nearestHandle({ x: 0.5, y: 0.5 }, 1, [target], { width: 1000, height: 500 }, 10),
		null
	);
	assert.equal(nearestHandle({ x: 0.5, y: 0.5 }, 0, [target], { width: 0, height: 500 }, 10), null);
	assert.equal(
		nearestHandle({ x: 0.5, y: 0.5 }, 0, [target], { width: 1000, height: 500 }, 0),
		null
	);
});

test('overlapping targets use stable identity ties and the nearest eligible point', () => {
	const a = candidate('a', 0.5, 0.5),
		b = candidate('b', 0.5, 0.5),
		far = candidate('far', 0.505, 0.5);
	for (const values of [
		[b, a, far],
		[far, a, b]
	])
		assert.equal(nearestHandle({ x: 0.5, y: 0.5 }, 0, values, { width: 1000, height: 500 }, 10), a);
});

test('link glyph keeps the DOM button offset and screen size at different zoom levels', () => {
	for (const width of [400, 1200, 3200]) {
		const overlays = linkOverlays(
			{ x: 0.5, y: 0.25 },
			{ width, height: width / 2 },
			16,
			'group',
			false
		);
		assert.equal(overlays.length, 3);
		const shape = overlays[0].shape;
		assert.equal(shape.kind, 'Circle');
		if (shape.kind === 'Circle') {
			assert(Math.abs((shape.center[0] - 0.5) * width - 19.2) < 1e-9);
			assert.equal(shape.radius, 9.6);
		}
		assert(overlays.every((entry) => entry.hit === 'group'));
	}
});

test('magnet defaults off and persists independently for Screen panels only', () => {
	assert.equal(screenPanelState({}, 'Screen').snap, false);
	assert.equal(screenPanelState({ snap: true }, 'Screen').snap, true);
	assert.equal(screenPanelState({ snap: true }, 'Surface').snap, false);
});
