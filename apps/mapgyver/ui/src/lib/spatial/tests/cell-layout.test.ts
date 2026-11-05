import assert from 'node:assert/strict';
import { test } from 'node:test';
import { CellLayoutReader, cellLayoutState } from '../cell-layout.ts';
import type { MatrixCellLayoutPage } from '../../media/generated/MatrixCellLayoutPage';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const query = { media: 'matrix', layer: 'layer', selected: 'cell', node: null, start: 0, count: 4 };
const page: MatrixCellLayoutPage = {
	media: 'matrix',
	layer: 'layer',
	row: null,
	context: null,
	cells: [],
	selected_id: 'cell',
	selected: null,
	total: 0,
	start: 0
};
const host = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	projectGeneration: 1 as number | null,
	currentHistoryStateId: 1,
	hasActiveEditSession: false,
	loading: { visible: false }
});

test('Cell layout serializes one active/latest read and preserves missing identity without crossing project/history fences', async () => {
	const session = host(),
		state = cellLayoutState(),
		requests: unknown[] = [];
	let resolve!: (value: MediaEditResult) => void;
	const first = new Promise<MediaEditResult>((yes) => {
		resolve = yes;
	});
	const reader = new CellLayoutReader(
		() => session,
		state,
		async (_, edit) => {
			requests.push(edit);
			return requests.length === 1 ? first : { kind: 'MatrixCellLayout', page };
		}
	);
	reader.refresh(query);
	for (let start = 1; start <= 100; start++) reader.refresh({ ...query, start });
	assert.equal(requests.length, 1);
	resolve({ kind: 'MatrixCellLayout', page: { ...page, selected_id: 'stale' } });
	await flush();
	await flush();
	assert.equal(requests.length, 2);
	assert.equal(reader.page?.selected_id, 'cell');
	assert.equal(reader.busy, false);
	session.currentHistoryStateId++;
	assert.equal(reader.busy, true);
	reader.refresh(query);
	await flush();
	assert.equal(reader.busy, false);
	session.projectGeneration = null;
	reader.refresh(query);
	assert.equal(reader.page, null);
	assert.equal(state.page, null);
	reader.dispose();
});

test('Cell layout retains same-project presentation on reconnect while edits await a fresh snapshot and disposal fences replies', async () => {
	const session = host(),
		state = cellLayoutState();
	const reader = new CellLayoutReader(
		() => session,
		state,
		async () => ({ kind: 'MatrixCellLayout', page })
	);
	reader.refresh(query);
	await flush();
	session.status = 'disconnected';
	reader.refresh(query);
	assert.equal(reader.page?.selected_id, 'cell');
	assert.equal(reader.busy, true);
	session.status = 'connected';
	session.snapshotGeneration++;
	assert.equal(reader.busy, true);
	reader.refresh(query);
	await flush();
	assert.equal(reader.busy, false);
	reader.dispose();
	assert.equal(reader.page, null);
});
