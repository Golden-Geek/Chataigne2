import { test } from 'vitest';
import assert from 'node:assert/strict';
import { buildCurveSamples } from '../canvas-samples';
import { compile_curve, sample_compiled_curve } from '../evaluation';
import { initialCurveCanvasTheme, readCurveCanvasTheme } from '../canvas-theme';
import { formatCurveAxisValue } from '../canvas-grid';
import type { CurveEasingKind, ParsedCurveKey } from '../types';

function samples(kind: CurveEasingKind, min = 0, max = 1) {
	const keys: ParsedCurveKey[] = [
		{
			node_id: 4,
			position: 0,
			value: 0,
			source_index: 0,
			easing: { node_id: 6, kind, params: {} }
		},
		{ node_id: 8, position: 1, value: 1, source_index: 1 }
	];
	const curve = compile_curve(keys);
	return buildCurveSamples(
		min,
		max,
		600,
		curve,
		(x) => x * 600,
		(y) => 300 - y * 300,
		(x) => sample_compiled_curve(curve, x) ?? 0
	);
}

test('visible curve sampling keeps rendered coordinates and hit owners aligned', () => {
	const result = samples('bezier', 0.25, 0.75);
	assert.equal(result.positions[0], 0.25);
	assert.equal(result.positions.at(-1), 0.75);
	assert.equal(result.edge_owner_key_ids.length, result.positions.length - 1);
	assert(result.edge_owner_key_ids.every((id) => id === 4));
	assert(result.positions.length <= 4096);
	for (let index = 0; index < result.positions.length; index++) {
		assert.equal(result.screen_x[index], result.positions[index] * 600);
		assert.equal(result.screen_y[index], 300 - result.values[index] * 300);
		assert(Math.abs(result.values[index] - result.positions[index]) < 1e-6);
	}
});

test('extension lines have no segment owner and retain endpoint values', () => {
	const result = samples('linear', -1, 2);
	assert.equal(result.positions[0], -1);
	assert.equal(result.values[0], 0);
	assert.equal(result.positions.at(-1), 2);
	assert.equal(result.values.at(-1), 1);
	assert.equal(result.edge_owner_key_ids[0], null);
	assert.equal(result.edge_owner_key_ids.at(-1), null);
	assert(result.edge_owner_key_ids.includes(4));
});

test('step/hold discontinuities retain vertical edges and their selectable owner', () => {
	for (const kind of ['steps', 'hold'] as const) {
		const result = samples(kind);
		let vertical = 0;
		for (let index = 1; index < result.positions.length; index++) {
			if (
				result.positions[index] === result.positions[index - 1] &&
				result.values[index] !== result.values[index - 1]
			) {
				vertical++;
				assert.equal(result.edge_owner_key_ids[index - 1], 4);
			}
		}
		assert(vertical >= 1, `${kind} must preserve its discontinuity`);
		assert.equal(result.values[0], 0);
		assert.equal(result.values.at(-1), 1);
	}
});

test('axis formatting retains compact bounds and scientific small/large values', () => {
	assert.equal(formatCurveAxisValue(1.25), '1.25');
	assert.equal(formatCurveAxisValue(1234), '1.23e+3');
	assert.equal(formatCurveAxisValue(0.0001), '1.00e-4');
	assert.equal(formatCurveAxisValue(Number.NaN), '0');
});

test('CSS theme resolution consumes overrides and fallback values without mutating defaults', () => {
	const before = { ...initialCurveCanvasTheme };
	const theme = readCurveCanvasTheme({
		getPropertyValue: (name) => (name === '--gc-color-curve-handle-out' ? ' #fd00fd ' : '')
	});
	assert.equal(theme.handle_out, '#fd00fd');
	assert.equal(theme.line_idle, '#9aa1ac');
	assert.deepEqual(initialCurveCanvasTheme, before);
	assert.notEqual(theme, initialCurveCanvasTheme);
});
