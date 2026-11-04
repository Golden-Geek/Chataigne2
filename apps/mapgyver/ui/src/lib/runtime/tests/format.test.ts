import assert from 'node:assert/strict';
import { test } from 'node:test';
import { formatVideoMemory } from '../format.ts';

test('allocation units use decimal MB/GB with matching numerical conversion', () => {
	assert.equal(formatVideoMemory(0), '0.0 MB');
	assert.equal(formatVideoMemory(1_000_000), '1.0 MB');
	assert.equal(formatVideoMemory(512 * 1024 * 1024), '536.9 MB');
	assert.equal(formatVideoMemory(1_000_000_000), '1.00 GB');
	assert.equal(formatVideoMemory(8_500_000_000), '8.50 GB');
});
