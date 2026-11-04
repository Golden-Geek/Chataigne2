import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cellPreviewGroups } from '../cell-previews.ts';
import type { MatrixViewportSnapshot } from '../../media/generated/MatrixViewportSnapshot';

function scene() {
	const reference = { media: 'shared-media', channel: 'shared-channel' };
	const snapshot = {
		row_total: 1000,
		column_total: 100,
		row_start: 0,
		column_start: 0,
		rows: [{ item: { id: { kind: 'Layer', id: 'row' } } }],
		columns: [{ id: 'one' }, { id: 'two' }, { id: 'three' }],
		cells: ['one', 'two', 'three'].map((column, index) => ({
			cell: {
				layer: 'row',
				definition: { column, source: { media: `owned-${index}`, channel: 'alias' } }
			},
			source_preview: index === 2 ? { media: 'copy', channel: 'independent' } : reference,
			source_missing: false
		}))
	} as unknown as MatrixViewportSnapshot;
	return { snapshot, metrics: { top: 0, left: 0, width: 1200, height: 800, rem: 16 } };
}

test('visible cells share canonical source groups while independent copies keep their own binding', () => {
	const { snapshot, metrics } = scene();
	const groups = cellPreviewGroups(snapshot, metrics);
	assert.equal(groups.length, 2);
	const shared = groups.find(
		(group) => JSON.parse(group.source).reference.media === 'shared-media'
	)!;
	assert.equal(shared.tiles.length, 2);
	assert.equal(shared.capacity, 2);
	assert.deepEqual(shared.tiles[0], { x: 3.2, y: 3.2, width: 113.6, height: 91.2 });
	assert.ok(shared.tiles[1].x > shared.tiles[0].x);
	assert.equal(
		groups.find((group) => JSON.parse(group.source).reference.media === 'copy')!.tiles.length,
		1
	);
});

test('scrolling preserves negative partial rectangles for host cropping and removes offscreen demand', () => {
	const { snapshot, metrics } = scene();
	const partial = cellPreviewGroups(snapshot, { ...metrics, left: 60 });
	const tile = partial.find((group) => JSON.parse(group.source).reference.media === 'shared-media')!
		.tiles[0];
	assert.ok(tile.x < 0 && tile.x + tile.width > 0);
	assert.equal(cellPreviewGroups(snapshot, { ...metrics, left: 500 }).length, 0);
	assert.equal(cellPreviewGroups(snapshot, { ...metrics, width: 100 }).length, 0);
});

test('missing sources and stale windows do not retain native tile demand', () => {
	const { snapshot, metrics } = scene();
	for (const cell of snapshot.cells) cell.source_missing = true;
	assert.deepEqual(cellPreviewGroups(snapshot, metrics), []);
	for (const cell of snapshot.cells) cell.source_missing = false;
	snapshot.rows = [];
	assert.deepEqual(cellPreviewGroups(snapshot, metrics), []);
});
