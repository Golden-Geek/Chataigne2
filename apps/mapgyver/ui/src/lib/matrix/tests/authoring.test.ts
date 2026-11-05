import { test } from 'node:test';
import assert from 'node:assert/strict';
import { MatrixController, matrixState } from '../authoring.ts';
import { matrixPanelState } from '../panel-state.ts';
import { viewportResult } from './fixture.ts';
import type { MediaEdit } from '../../media/generated/MediaEdit';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
import type { MatrixEditorQuery } from '../../media/generated/MatrixEditorQuery';
import type { MatrixSelection } from '../../media/generated/MatrixSelection';

const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const connection = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	projectGeneration: 1 as number | null,
	currentHistoryStateId: 1,
	loading: { visible: false },
	hasActiveEditSession: false
});
const query = (media = 'matrix'): MatrixEditorQuery => ({
	selected: media,
	document: null,
	locked: null,
	retained: null,
	viewport: matrixState().viewport,
	selection: null,
	node: null
});
function snapshot(
	media = 'matrix',
	row = 0,
	selection: MatrixSelection | null = null
): MediaEditResult {
	const value = viewportResult(media, row);
	if (value.kind !== 'MatrixViewport') throw Error();
	return {
		kind: 'MatrixEditor',
		page: {
			document: 'document',
			document_matches: true,
			media,
			node: null,
			selection,
			snapshot: value.snapshot
		}
	};
}
const missing = (media = 'matrix'): MediaEditResult => ({
	kind: 'MatrixEditor',
	page: {
		document: 'document',
		document_matches: true,
		media,
		node: null,
		selection: null,
		snapshot: null
	}
});
function deferred<T>() {
	let resolve!: (value: T) => void, reject!: (error: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}

test('authoritative absence clears the window and Undo restores the retained identity without a catalog default', async () => {
	const host = connection();
	let absent = false;
	const controller = new MatrixController(
		() => host,
		matrixState(),
		async (_, edit) => {
			assert.equal(edit.kind, 'InspectMatrixEditor');
			return absent ? missing() : snapshot();
		}
	);
	assert.equal(controller.state.window, null);
	controller.observe(query());
	await flush();
	assert.equal(controller.window?.media, 'matrix');
	absent = true;
	host.currentHistoryStateId++;
	controller.refresh();
	await flush();
	assert.equal(controller.state.window, null);
	absent = false;
	host.currentHistoryStateId++;
	controller.refresh();
	await flush();
	assert.equal(controller.window?.media, 'matrix');
});
test('late absence cannot clear a newer selected Matrix', async () => {
	const host = connection(),
		old = deferred<MediaEditResult>();
	let calls = 0;
	const controller = new MatrixController(
		() => host,
		matrixState(),
		async () => (++calls === 1 ? old.promise : snapshot('new'))
	);
	controller.observe(query('old'));
	controller.observe(query('new'));
	old.resolve(missing('old'));
	await flush();
	assert.equal(controller.window?.media, 'new');
});
test('scroll bursts retain one active and one latest bounded read without selection changes or launches', async () => {
	const host = connection(),
		pending = deferred<MediaEditResult>(),
		edits: MediaEdit[] = [];
	let calls = 0;
	const controller = new MatrixController(
		() => host,
		matrixState(),
		async (_, edit) => {
			edits.push(edit);
			if (++calls === 1) return snapshot();
			if (calls === 2) return pending.promise;
			if (edit.kind !== 'InspectMatrixEditor') throw Error();
			return snapshot('matrix', edit.query.viewport.row_start, edit.query.selection);
		}
	);
	controller.observe(query());
	await flush();
	controller.select({ kind: 'Layer', id: 'layer' }, 'column');
	for (let row = 1; row < 100; row++)
		controller.setViewport({ row_start: row, row_count: 8, axis_start: row, axis_count: 8 });
	assert.equal(edits.length, 2);
	pending.resolve(snapshot());
	await flush();
	assert.equal(edits.length, 3);
	assert.equal(controller.window?.row_start, 99);
	assert.deepEqual(controller.state.selected, {
		item: { kind: 'Layer', id: 'layer' },
		column: 'column'
	});
	assert(edits.every((edit) => edit.kind === 'InspectMatrixEditor'));
});
test('session/project/snapshot/disconnect/disposal fence stale windows and errors', async () => {
	for (const reason of ['session', 'project', 'snapshot', 'disconnect', 'dispose']) {
		let host = connection();
		const pending = deferred<MediaEditResult>(),
			state = matrixState();
		const controller = new MatrixController(
			() => host,
			state,
			async () => pending.promise
		);
		controller.observe(query());
		if (reason === 'session') host = connection();
		if (reason === 'project') host.projectGeneration = 2;
		if (reason === 'snapshot') host.snapshotGeneration++;
		if (reason === 'disconnect') host.status = 'disconnected';
		if (reason === 'dispose') controller.dispose();
		pending.reject(Error('old scope'));
		await flush();
		assert.equal(controller.window, null);
		assert.equal(state.readError, '');
	}
});
test('same-project reconnect retains presentation but unknown/new projects withdraw it immediately', async () => {
	const host = connection(),
		controller = new MatrixController(
			() => host,
			matrixState(),
			async () => snapshot()
		);
	controller.observe(query());
	await flush();
	host.status = 'disconnected';
	controller.refresh();
	assert(controller.window);
	host.status = 'connected';
	host.snapshotGeneration++;
	controller.refresh();
	assert(controller.window);
	await flush();
	host.projectGeneration = null;
	assert.equal(controller.window, null);
	host.projectGeneration = 2;
	assert.equal(controller.window, null);
});
test('history replacement disables mutation until the exact current projection arrives', async () => {
	const host = connection(),
		pending = deferred<MediaEditResult>();
	let calls = 0;
	const controller = new MatrixController(
		() => host,
		matrixState(),
		async () => (++calls === 1 ? snapshot() : pending.promise)
	);
	controller.observe(query());
	await flush();
	host.currentHistoryStateId++;
	assert.equal(controller.busy, true);
	assert.equal(await controller.edit({ kind: 'ClearPlayback' }), undefined);
	controller.refresh();
	pending.resolve(snapshot());
	await flush();
	assert.equal(controller.busy, false);
});
test('assignment and launch remain separate explicit intents, with persistent rejection feedback', async () => {
	const host = connection(),
		state = matrixState(),
		edits: MediaEdit[] = [];
	const controller = new MatrixController(
		() => host,
		state,
		async (_, edit) => {
			edits.push(edit);
			if (edit.kind === 'InspectMatrixEditor') return snapshot();
			throw Error('rejected mutation');
		}
	);
	controller.observe(query());
	await flush();
	await controller.edit({ kind: 'SetCells', assignments: [] });
	await flush();
	assert.match(state.error, /rejected mutation/);
	assert.equal(edits.filter((edit) => edit.kind === 'Matrix').length, 1);
	await controller.edit({ kind: 'LaunchColumn', column: 'column', mode: 'Cut' });
	assert.equal(edits.filter((edit) => edit.kind === 'Matrix').length, 2);
});
test('stale structural receipts cannot adopt created identities in a replaced project', async () => {
	const host = connection(),
		pending = deferred<MediaEditResult>();
	const controller = new MatrixController(
		() => host,
		matrixState(),
		async (_, edit) => (edit.kind === 'InspectMatrixEditor' ? snapshot() : pending.promise)
	);
	controller.observe(query());
	await flush();
	const mutation = controller.edit({ kind: 'EnsureCell', layer: 'layer', column: 'column' });
	host.projectGeneration = 2;
	controller.observe(query('new'));
	pending.resolve({ kind: 'MatrixCellPrepared', cell: 'old-cell', application: 'old-application' });
	assert.equal(await mutation, undefined);
});
test('backend selection replies coalesce with viewport reads and suppress superseded Inspector definitions', async () => {
	const host = connection(),
		pending = deferred<MediaEditResult>();
	let calls = 0;
	const controller = new MatrixController(
		() => host,
		matrixState(),
		async (_, edit) => {
			if (++calls === 1) return snapshot();
			if (calls === 2) return pending.promise;
			if (edit.kind !== 'InspectMatrixEditor') throw Error();
			return snapshot('matrix', 0, edit.query.selection);
		}
	);
	controller.observe(query());
	await flush();
	controller.select({ kind: 'Layer', id: 'a' }, 'first');
	controller.select({ kind: 'Layer', id: 'b' }, 'last');
	pending.resolve(snapshot());
	await flush();
	assert.deepEqual(controller.state.selected, { item: { kind: 'Layer', id: 'b' }, column: 'last' });
});
test('current structural receipts return committed identities to the caller without implicit selection', async () => {
	const host = connection(),
		pending = deferred<MediaEditResult>();
	let mutations = 0;
	const controller = new MatrixController(
		() => host,
		matrixState(),
		async (_, edit) => {
			if (edit.kind === 'InspectMatrixEditor') return snapshot();
			mutations++;
			return pending.promise;
		}
	);
	controller.observe(query());
	await flush();
	const result = controller.edit({ kind: 'EnsureCell', layer: 'layer', column: 'column' });
	assert.equal(
		await controller.edit({ kind: 'EnsureCell', layer: 'layer', column: 'column' }),
		undefined
	);
	pending.resolve({
		kind: 'MatrixCellPrepared',
		cell: 'committed-cell',
		application: 'application'
	});
	assert.deepEqual(await result, {
		kind: 'MatrixCellPrepared',
		cell: 'committed-cell',
		application: 'application'
	});
	assert.equal(mutations, 1);
	assert.equal(controller.state.node, null);
});
test('address, viewport and pending selection reads keep displayed-target controls usable', async () => {
	const host = connection(),
		pending = deferred<MediaEditResult>();
	let calls = 0;
	const controller = new MatrixController(
		() => host,
		matrixState(),
		async () => (++calls === 1 ? snapshot() : pending.promise)
	);
	controller.observe(query());
	await flush();
	controller.select({ kind: 'Layer', id: 'row' }, 'column');
	assert.equal(controller.busy, false);
	controller.setViewport({ row_start: 3, row_count: 8, axis_start: 3, axis_count: 8 });
	assert.equal(controller.busy, false);
	controller.observe(query('other'));
	assert.equal(controller.busy, false);
	pending.resolve(snapshot('other'));
	await flush();
});

test('equivalent measured and persisted viewports neither read again nor invalidate an active reply', async () => {
	const host = connection(),
		pending = deferred<MediaEditResult>();
	let calls = 0;
	const state = matrixState(),
		controller = new MatrixController(
			() => host,
			state,
			async () => {
				calls++;
				return pending.promise;
			}
		);
	controller.observe(query());
	const viewport = { axis_count: 16, axis_start: 0, row_count: 32, row_start: 0 };
	controller.setViewport(viewport);
	controller.restore(viewport, null, null);
	assert.equal(calls, 1);
	pending.resolve(snapshot());
	await flush();
	assert.equal(controller.window?.media, 'matrix');
});

test('persisted bindings copy typed identities, clamp view bounds and reject malformed document scopes', () => {
	const id = '11111111-2222-4333-8444-555555555555';
	const input = {
		document: id,
		media: id,
		locked: true,
		selection: { item: { kind: 'Layer' as const, id }, column: id }
	};
	const a = matrixPanelState(input),
		b = matrixPanelState(input);
	a.selection!.item!.id = 'changed';
	assert.equal(b.selection?.item?.id, id);
	assert.equal(matrixPanelState({ media: id, locked: true }).media, null);
	assert.equal(matrixPanelState({ scroll: { top: Infinity, left: -1 } }).scroll.top, 0);
	assert(
		matrixPanelState({
			viewport: { row_start: 0, row_count: 1000, axis_start: 0, axis_count: 1000 }
		}).viewport.row_count <= 64
	);
});
