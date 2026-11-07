import assert from 'node:assert/strict';
import { test } from 'node:test';
import { CellLayoutReader, cellLayoutState } from '../cell-layout.ts';
import type { MatrixCellLayoutPage } from '../../media/generated/MatrixCellLayoutPage';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
import type { MatrixLayoutCell } from '../../media/generated/MatrixLayoutCell';
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const query = { media: 'matrix', layer: 'layer', selected: 'cell', node: null, start: 0, count: 4 };
const page: MatrixCellLayoutPage = {
	at: { tick: 0, micro: 0, seq: 0 },
	parameters: [],
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

test('held Cell geometry patches bounded and independent selection while queries resume only after the gesture', async () => {
	const session = host(),
		state = cellLayoutState(),
		requests: unknown[] = [];
	const cell = (id: string): MatrixLayoutCell => ({
		label: id,
		extent: null,
		preview: { kind: 'MatrixCell', media: 'matrix', cell: id },
		view: {
			source_label: null,
			source_preview: null,
			source_missing: false,
			cell: {
				id,
				media: null,
				layer: 'layer',
				definition: {
					column: id,
					source: null,
					application: id,
					transition: null,
					settings: { position: [0, 0], size: null, fit_mode: 'Fit', opacity: 1, enabled: false }
				}
			}
		}
	});
	const projected = {
		...page,
		cells: [cell('visible')],
		selected: cell('offpage'),
		selected_id: 'offpage'
	};
	const reader = new CellLayoutReader(
		() => session,
		state,
		async (_, edit) => {
			requests.push(edit);
			return { kind: 'MatrixCellLayout', page: projected };
		}
	);
	reader.refresh(query);
	await flush();
	reader.setInteracting(true);
	reader.refresh({ ...query, start: 128 });
	assert.equal(requests.length, 1);
	assert.equal(reader.busy, true);
	const settings = {
		position: [4, 3] as [number, number],
		size: null,
		fit_mode: 'Fill' as const,
		opacity: 0.5,
		enabled: false
	};
	for (const id of ['visible', 'offpage'])
		reader.acceptCell({ kind: 'MatrixCellGeometryChanged', media: 'matrix', cell: id, settings });
	assert.deepEqual(reader.page?.cells[0].view.cell.definition.settings, settings);
	assert.deepEqual(reader.page?.selected?.view.cell.definition.settings, settings);
	assert.equal(reader.page?.selected_id, 'offpage');
	reader.acceptCell({
		kind: 'MatrixCellGeometryChanged',
		media: 'foreign',
		cell: 'offpage',
		settings: { ...settings, position: [99, 99] }
	});
	assert.deepEqual(reader.page?.selected?.view.cell.definition.settings.position, [4, 3]);
	reader.setInteracting(false);
	reader.refresh();
	await flush();
	assert.equal(requests.length, 2);
	assert.deepEqual(requests[1], { kind: 'InspectMatrixCells', query: { ...query, start: 128 } });
	reader.dispose();
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
