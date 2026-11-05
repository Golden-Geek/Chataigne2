import { test } from 'node:test';
import assert from 'node:assert/strict';
import { gridSize, projectGrid } from '../viewport.ts';

test('large grids retain readable minimum spans and bounded windows at every viewport scale', () => {
	for (const rem of [12, 16, 24, 32]) {
		const view = projectGrid({ top: 0, left: 0, width: 1200, height: 800, rem }, 100000, 100000);
		assert.ok(view.viewport.row_count * view.viewport.axis_count <= 2048);
		assert.equal(view.row, gridSize.row);
		assert.equal(view.column, gridSize.column);
	}
});

test('large Matrix scroll ranges stay finite and mount only a bounded window, including the last address', () => {
	const metrics = { top: 0, left: 0, width: 1200, height: 800, rem: 16 };
	const first = projectGrid(metrics, 100000, 100000);
	assert.ok(first.viewport.row_count <= 64 && first.viewport.axis_count <= 32);
	assert.ok(first.viewport.row_count * first.viewport.axis_count <= 2048);
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
	assert.ok(last.viewport.axis_start + last.viewport.axis_count >= 100000);
	assert.ok(Number.isFinite(last.rowOrigin) && Number.isFinite(last.columnOrigin));
	assert.equal(first.viewport.row_start, 0);
	assert.equal(first.viewport.axis_start, 0);
});

test('empty and oversized viewports keep positive protocol counts at different font scales', () => {
	for (const rem of [12, 16, 32, 48]) {
		const view = projectGrid({ top: 0, left: 0, width: 100000, height: 100000, rem }, 0, 0);
		assert.equal(view.viewport.row_start, 0);
		assert.equal(view.viewport.axis_start, 0);
		assert.ok(view.viewport.row_count >= 1 && view.viewport.row_count <= 64);
		assert.ok(view.viewport.axis_count >= 1 && view.viewport.axis_count <= 32);
	}
});

test('small grids fill available area while trailing creation headers remain inside the scroll extent', () => {
	for (const rem of [12, 16, 24, 32]) {
		const metrics = { top: 0, left: 0, width: 1200, height: 800, rem };
		const view = projectGrid(metrics, 2, 2);
		assert(view.row >= gridSize.row && view.column >= gridSize.column);
		assert(
			Math.abs(view.canvasWidth - (gridSize.label + 2 * view.column + gridSize.creation)) < 1e-8
		);
		assert(Math.abs(view.canvasHeight - (view.header + 2 * view.row + gridSize.creation)) < 1e-8);
		assert(view.canvasWidth * rem >= metrics.width - 1e-6);
		assert(view.canvasHeight * rem >= metrics.height - 1e-6);
	}
});

test('deep group headers keep a bounded scroll area and matching row geometry at every scale', () => {
	for (const rem of [12, 16, 24, 32]) {
		const metrics = { top: 0, left: 0, width: 1200, height: 500, rem };
		const view = projectGrid(metrics, 1000, 1000, 32);
		assert(view.groupHeight * rem <= metrics.height * 0.3);
		assert(view.groupHeight <= 8);
		assert.equal(view.rowOrigin, gridSize.header + view.groupHeight);
		assert(view.viewport.row_count * view.viewport.axis_count <= 2048);
	}
});
