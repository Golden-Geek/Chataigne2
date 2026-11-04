import { test } from 'node:test';
import assert from 'node:assert/strict';
import { gridSize, projectGrid, selectionScroll } from '../viewport.ts';

test('cells retain equal row and column spans at every viewport scale', () => {
	assert.equal(gridSize.row, gridSize.column);
	for (const rem of [12, 16, 24, 32]) {
		const view = projectGrid({ top: 0, left: 0, width: 1200, height: 800, rem }, 100000, 100000);
		assert.ok(view.viewport.row_count * view.viewport.column_count <= 2048);
	}
});

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

test('Frame selection reveals a distant address through compressed ranges without enlarging the window', () => {
	for (const rem of [12, 16, 24, 32]) {
		const metrics = { top: 0, left: 0, width: 1200, height: 800, rem };
		for (const address of [0, 50000, 99999]) {
			const scroll = selectionScroll(metrics, 100000, 100000, address, address);
			const view = projectGrid({ ...metrics, ...scroll }, 100000, 100000).viewport;
			assert(view.row_start <= address && view.row_start + view.row_count > address);
			assert(view.column_start <= address && view.column_start + view.column_count > address);
			assert(view.row_count * view.column_count <= 2048);
		}
	}
});
