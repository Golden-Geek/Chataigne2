import { test } from 'node:test';
import assert from 'node:assert/strict';
import { gridSize, projectGrid } from '../viewport.ts';

test('large Matrix scroll ranges stay finite and mount only a bounded window, including the last address', () => {
	const metrics = { top: 0, left: 0, width: 1200, height: 800, rem: 16 };
	const first = projectGrid(metrics, 100000, 100000);
	assert.ok(first.viewport.row_count <= 64 && first.viewport.column_count <= 32);
	assert.ok(first.viewport.row_count * first.viewport.column_count <= 2048);
	assert.ok(first.canvasHeight <= gridSize.maximum && first.canvasWidth <= gridSize.maximum);
	const last = projectGrid(
		{
			...metrics,
			top: first.canvasHeight * 16 - metrics.height,
			left: first.canvasWidth * 16 - metrics.width
		},
		100000,
		100000
	);
	assert.ok(last.viewport.row_start + last.viewport.row_count >= 100000);
	assert.ok(last.viewport.column_start + last.viewport.column_count >= 100000);
	assert.ok(Number.isFinite(last.rowOrigin) && Number.isFinite(last.columnOrigin));
	assert.equal(first.viewport.row_start, 0);
	assert.equal(first.viewport.column_start, 0);
});

test('empty and oversized viewports keep positive protocol counts at different font scales', () => {
	for (const rem of [12, 16, 32, 48]) {
		const view = projectGrid({ top: 0, left: 0, width: 100000, height: 100000, rem }, 0, 0);
		assert.equal(view.viewport.row_start, 0);
		assert.equal(view.viewport.column_start, 0);
		assert.ok(view.viewport.row_count >= 1 && view.viewport.row_count <= 64);
		assert.ok(view.viewport.column_count >= 1 && view.viewport.column_count <= 32);
	}
});
