import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { MatrixPlaybackMix } from '../../preview/generated/MatrixPlaybackMix';
import { lineWeight, playbackLines } from '../playback-mix.ts';

test('Cell and heading lines follow the actual Layer envelopes after an interruption', () => {
	const mix: MatrixPlaybackMix = {
		base_in_use: false,
		base: {
			interval: null,
			weights: [{ column: 'new', from: 1, to: 1, individual: false }]
		},
		layers: [
			{
				layer: 'slow',
				mix: {
					interval: { start_ms: 200, duration_ms: 1000, curve: 'Smooth' },
					weights: [
						{ column: 'old', from: 0.4, to: 0, individual: false },
						{ column: 'old', from: 0.2, to: 0, individual: false },
						{ column: 'new', from: 0, to: 1, individual: false }
					]
				}
			},
			{
				layer: 'fast',
				mix: {
					interval: { start_ms: 200, duration_ms: 500, curve: 'Linear' },
					weights: [{ column: 'new', from: 0, to: 1, individual: false }]
				}
			},
			{
				layer: 'independent',
				mix: {
					interval: null,
					weights: [{ column: 'solo', from: 1, to: 1, individual: true }]
				}
			}
		]
	};
	const lines = playbackLines(mix);
	assert.equal(lines.until, 1200);
	assert.equal(lineWeight(lines.cell('slow', 'new')!.incoming, 450), 0.15625);
	assert.equal(lineWeight(lines.columns.get('new')!.incoming, 450), 0.5);
	assert.ok(Math.abs(lineWeight(lines.cell('slow', 'old')!.outgoing, 700) - 0.3) < 1e-9);
	assert.equal(lineWeight(lines.columns.get('old')!.outgoing, 1200), 0);
	assert.equal(lineWeight(lines.cell('independent', 'solo')!.incoming, 100), 1);
	assert.equal(lines.columns.has('solo'), false);
});
