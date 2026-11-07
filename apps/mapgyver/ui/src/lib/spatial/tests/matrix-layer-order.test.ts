import assert from 'node:assert/strict';
import { test } from 'node:test';
import { matrixLayerPaintRows } from '../rect-presentation.ts';
import { viewportResult } from '../../matrix/tests/fixture.ts';
import type { MatrixViewRow } from '../../media/generated/MatrixViewRow';

const row = (id: string): MatrixViewRow => ({
	depth: 0,
	item: {
		id: { kind: 'Layer', id },
		parent: null,
		label: id,
		channel: { media: 'matrix', channel: id },
		settings: { enabled: true, opacity: 1, blend: 'Normal', expose_channel: false },
		layer: { position: [0, 0], size: { width: 16, height: 16 }, duration_ms: 250 }
	}
});

test('Layer SVG paint/hit order follows backend front-first ordinals without changing hierarchy', () => {
	const result = viewportResult('matrix', 10);
	assert.equal(result.kind, 'MatrixViewport');
	if (result.kind !== 'MatrixViewport') throw Error('fixture');
	const snapshot = result.snapshot;
	snapshot.rows = [row('front'), row('back')];
	const authored = [...snapshot.rows];
	assert.deepEqual(
		matrixLayerPaintRows(snapshot, null).map((value) => value.item.id.id),
		['back', 'front']
	);
	assert.deepEqual(snapshot.rows, authored);
	assert.deepEqual(
		matrixLayerPaintRows(snapshot, snapshot.rows[0]).map((value) => value.item.id.id),
		['back', 'front']
	);
});

test('independent offscreen Layer selection paints at its real ordinal, never automatically on top', () => {
	const result = viewportResult('matrix', 10);
	if (result.kind !== 'MatrixViewport') throw Error('fixture');
	const snapshot = result.snapshot,
		independent = row('selected');
	snapshot.rows = [row('front'), row('back')];
	for (const [index, expected] of [
		[0, ['back', 'front', 'selected']],
		[100, ['selected', 'back', 'front']]
	] as const) {
		snapshot.selected = {
			selection: { item: independent.item.id, column: null },
			row_index: index,
			column_index: null,
			row: independent,
			column: null,
			cell: null
		};
		assert.deepEqual(
			matrixLayerPaintRows(snapshot, independent).map((value) => value.item.id.id),
			expected
		);
	}
});
