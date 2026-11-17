import assert from 'node:assert/strict';
import { test } from 'node:test';
import { previewKey } from '../identity.ts';

test('preview kinds have separate identities and channel property order is irrelevant', () => {
	const first = { kind: 'Channel', reference: { media: 'same', channel: 'channel' } } as const;
	const reordered = { kind: 'Channel', reference: { channel: 'channel', media: 'same' } } as const;
	assert.equal(previewKey(first), previewKey(reordered));
	assert.notEqual(previewKey(first), previewKey({ kind: 'Screen', screen: 'same' }));
	assert.notEqual(
		previewKey({ kind: 'Screen', screen: 'same' }),
		previewKey({ kind: 'Screen', screen: 'other' })
	);
});

test('calibration identity shares matching images and fences mode and dimensions', () => {
	const first = { kind: 'Calibration', mode: 'Grid', extent: { width: 96, height: 64 } } as const;
	assert.equal(previewKey(first), previewKey({ ...first, extent: { height: 64, width: 96 } }));
	assert.notEqual(previewKey(first), previewKey({ ...first, mode: 'ColorBars' }));
	assert.notEqual(previewKey(first), previewKey({ ...first, extent: { width: 64, height: 96 } }));
});
