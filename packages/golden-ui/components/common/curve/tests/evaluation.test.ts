import { test } from 'vitest';
import assert from 'node:assert/strict';
import { compile_curve, sample_compiled_curve } from '../evaluation';
import type { CurveEasingKind, ParsedCurveKey } from '../types';

const keys = (kind: CurveEasingKind): ParsedCurveKey[] => [
	{ node_id: 1, position: 0, value: 0, source_index: 0, easing: { node_id: 3, kind, params: {} } },
	{ node_id: 2, position: 1, value: 1, source_index: 1 }
];
for (const kind of [
	'linear',
	'bezier',
	'hold',
	'steps',
	'shape',
	'perlinNoise',
	'elastic',
	'bounce',
	'random',
	'script'
] as const) {
	test(`${kind} evaluation retains endpoints, finite samples and deterministic results`, () => {
		const compiled = compile_curve(keys(kind));
		assert.equal(sample_compiled_curve(compiled, -1), 0);
		assert.equal(sample_compiled_curve(compiled, 2), 1);
		for (let index = 1; index < 32; index++) {
			const value = sample_compiled_curve(compiled, index / 32);
			assert(value !== null && Number.isFinite(value));
			assert.equal(sample_compiled_curve(compiled, index / 32), value);
		}
		if (kind === 'linear' || kind === 'bezier' || kind === 'script')
			assert(Math.abs(sample_compiled_curve(compiled, 0.25)! - 0.25) < 1e-6);
		if (kind === 'hold') assert.equal(sample_compiled_curve(compiled, 0.75), 0);
		if (kind === 'steps') assert.equal(sample_compiled_curve(compiled, 0.3), 0.25);
	});
}

test('empty/nonfinite inputs and duplicate positions retain the existing sampling contract', () => {
	assert.equal(sample_compiled_curve(compile_curve([]), 0), null);
	assert.equal(sample_compiled_curve(compile_curve(keys('linear')), Number.NaN), null);
	const input = keys('linear');
	input.push({ node_id: 4, position: 0, value: 0.5, source_index: 2 });
	input.push({ node_id: 5, position: Number.NaN, value: 3, source_index: 3 });
	const compiled = compile_curve(input);
	assert.equal(compiled.keys.length, 2);
	assert.equal(sample_compiled_curve(compiled, 0), 0.5);
	assert.equal(input.length, 4, 'compilation does not mutate the authored projection');
});
