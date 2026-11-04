import { test } from 'node:test';
import assert from 'node:assert/strict';
import { MatrixController, matrixState } from '../authoring.ts';
import { choiceState } from '../choices.ts';
import type { MediaEdit } from '../../media/generated/MediaEdit';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';

const flush = async () => {
	for (let i = 0; i < 12; i++) await Promise.resolve();
};
function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (error: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}
const connection = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	currentHistoryStateId: 0,
	loading: { visible: false }
});

test('authoritative Matrix absence clears held viewport data and Undo refresh recovers it', async () => {
	const session = connection(),
		state = matrixState();
	const replies: ReturnType<typeof deferred<MediaEditResult>>[] = [];
	const controller = new MatrixController(
		() => session,
		state,
		choiceState(),
		choiceState(),
		async () => {
			const reply = deferred<MediaEditResult>();
			replies.push(reply);
			return reply.promise;
		}
	);
	controller.observe(false);
	controller.selectMedia('matrix');
	replies[0].resolve(snapshot('matrix'));
	await flush();
	assert.equal(controller.window?.media, 'matrix');
	session.currentHistoryStateId++;
	controller.observe(false);
	replies[1].resolve({ kind: 'MatrixMissing', media: 'matrix' });
	await flush();
	assert.equal(Boolean(controller.window), false);
	assert.equal(state.readError, '');
	assert.equal(state.media, 'matrix');
	session.currentHistoryStateId++;
	controller.observe(false);
	replies[2].resolve(snapshot('matrix'));
	await flush();
	assert.equal(controller.window?.media, 'matrix');
	controller.dispose();
});

test('mismatched or late Matrix absence cannot clear a current viewport', async () => {
	const session = connection(),
		state = matrixState();
	const replies: ReturnType<typeof deferred<MediaEditResult>>[] = [];
	const controller = new MatrixController(
		() => session,
		state,
		choiceState(),
		choiceState(),
		async () => {
			const reply = deferred<MediaEditResult>();
			replies.push(reply);
			return reply.promise;
		}
	);
	controller.observe(false);
	controller.selectMedia('matrix');
	replies[0].resolve(snapshot('matrix'));
	await flush();
	controller.refresh();
	replies[1].resolve({ kind: 'MatrixMissing', media: 'other' });
	await flush();
	assert.equal(controller.window?.media, 'matrix');
	assert.equal(state.readError, 'Unexpected Matrix viewport');
	controller.refresh();
	controller.selectMedia('new');
	replies[2].resolve({ kind: 'MatrixMissing', media: 'matrix' });
	await flush();
	replies[3].resolve(snapshot('new'));
	await flush();
	assert.equal(controller.window?.media, 'new');
	assert.equal(state.readError, '');
	controller.dispose();
});
function snapshot(media: string, row = 0): MediaEditResult {
	return {
		kind: 'MatrixViewport',
		snapshot: {
			media_types: [],
			project: 'project-1',
			media,
			final_output: {
				reference: { media, channel: 'final' },
				label: 'Final',
				definition: {
					source: {
						kind: 'Matrix',
						output: {
							kind: 'Final',
							definition: {
								canvas: { width: 4, height: 2 },
								empty_cell: 'Transparent'
							}
						}
					},
					size: { kind: 'Automatic' },
					fit_mode: 'Fit'
				}
			},
			revision: 1,
			row_total: 100000,
			column_total: 100000,
			row_start: row,
			column_start: 0,
			rows: [],
			columns: [],
			cells: [],
			selected: null,
			playback: {
				request_id: null,
				layers: [],
				desired_column: null,
				launch_id: null,
				transition: { duration_ms: 250, curve: 'Linear' }
			}
		}
	};
}

test('scroll bursts keep one active read and one latest window without changing cell selection or launching', async () => {
	const session = connection(),
		state = matrixState(),
		reads: { edit: MediaEdit; reply: ReturnType<typeof deferred<MediaEditResult>> }[] = [];
	state.media = 'matrix';
	const controller = new MatrixController(
		() => session,
		state,
		choiceState(),
		choiceState(),
		async (_session, edit) => {
			const reply = deferred<MediaEditResult>();
			reads.push({ edit, reply });
			return reply.promise;
		}
	);
	controller.select({ kind: 'Layer', id: 'layer' }, 'column');
	controller.refresh();
	for (let i = 1; i <= 1000; i++) controller.setViewport({ ...state.viewport, row_start: i });
	assert.equal(reads.length, 1);
	reads[0].reply.resolve(snapshot('matrix', 0));
	await flush();
	assert.equal(reads.length, 2);
	assert.equal(state.window === null, true);
	assert.equal(reads[1].edit.kind, 'Matrix');
	if (reads[1].edit.kind === 'Matrix' && reads[1].edit.edit.kind === 'InspectViewport')
		assert.equal(reads[1].edit.edit.viewport.row_start, 1000);
	reads[1].reply.resolve(snapshot('matrix', 1000));
	await flush();
	assert.equal(state.window?.row_start, 1000);
	assert.deepEqual(state.selected, { item: { kind: 'Layer', id: 'layer' }, column: 'column' });
	assert.ok(
		reads.every((read) => read.edit.kind === 'Matrix' && read.edit.edit.kind === 'InspectViewport')
	);
	controller.dispose();
});

test('matrix/project replacement, disconnect and disposal fence late windows and errors', async () => {
	for (const reason of ['matrix', 'project', 'disconnect', 'dispose']) {
		const session = connection(),
			state = matrixState();
		state.media = 'old';
		const replies: ReturnType<typeof deferred<MediaEditResult>>[] = [];
		const controller = new MatrixController(
			() => session,
			state,
			choiceState(),
			choiceState(),
			async () => {
				const reply = deferred<MediaEditResult>();
				replies.push(reply);
				return reply.promise;
			}
		);
		controller.observe(false);
		// observe establishes the connection context; selection starts the first window.
		controller.selectMedia('old');
		assert.equal(replies.length, 1);
		if (reason === 'matrix') controller.selectMedia('new');
		if (reason === 'project') {
			session.snapshotGeneration++;
			controller.observe(false);
			controller.selectMedia('new');
		}
		if (reason === 'disconnect') {
			session.status = 'disconnected';
			controller.observe(false);
		}
		if (reason === 'dispose') controller.dispose();
		replies[0].reject(new Error('stale error'));
		await flush();
		assert.equal(state.error, '');
		assert.equal(state.readError, '');
		if (reason === 'matrix' || reason === 'project') {
			assert.equal(replies.length, 2);
			replies[1].resolve(snapshot('new'));
			await flush();
			assert.equal(state.window?.media, 'new');
		} else {
			assert.equal(state.window, null);
			assert.equal(replies.length, 1);
		}
		controller.dispose();
		controller.observe();
		assert.equal(state.pending, false);
	}
});

test('viewport refresh cannot adopt a new project generation before parent observation resets old media', async () => {
	const session = connection(),
		state = matrixState(),
		calls: MediaEdit[] = [];
	const controller = new MatrixController(
		() => session,
		state,
		choiceState(),
		choiceState(),
		async (_, edit) => {
			calls.push(edit);
			assert.equal(edit.kind, 'Matrix');
			return snapshot(edit.media);
		}
	);
	controller.observe(false);
	controller.selectMedia('old');
	await flush();
	assert.equal(state.window?.media, 'old');
	const published = () => controller.window;
	assert.equal(published(), state.window);
	assert.equal(calls.length, 1);
	// A late grid resize runs before the parent's generation effect. It still owns old media.
	session.snapshotGeneration++;
	assert.equal(published(), null, 'old child forms must be unpublished immediately');
	assert.equal(controller.busy, true, 'old controls cannot author into the new generation');
	await controller.edit({ kind: 'LaunchColumn', column: 'old-column', mode: 'Cut' });
	controller.setViewport({ ...state.viewport, row_start: 5 });
	await flush();
	assert.equal(calls.length, 1, 'old media must never be inspected using the new generation');
	assert.equal(state.media, null);
	assert.equal(state.window, null);
	controller.selectMedia('new');
	await flush();
	assert.equal(published()?.media, 'new', 'current-project controls recover normally');
	assert.equal(calls.length, 2);
	controller.dispose();
});

test('assignment invokes only its explicit backend intent; a later launch is separate and errors survive refresh', async () => {
	const session = connection(),
		state = matrixState();
	state.media = 'matrix';
	const calls: MediaEdit[] = [];
	let rejectAssignment = false;
	const controller = new MatrixController(
		() => session,
		state,
		choiceState(),
		choiceState(),
		async (_session, edit) => {
			calls.push(edit);
			if (edit.kind === 'InspectChoices')
				return { kind: 'Choices', page: { choices: [], total: 0, start: 0 } };
			if (edit.kind === 'Matrix' && edit.edit.kind === 'InspectViewport')
				return snapshot(edit.media);
			if (rejectAssignment) throw new Error('media channel dependency cycle');
			session.currentHistoryStateId++;
			return { kind: 'Changed' };
		}
	);
	controller.select({ kind: 'Layer', id: 'layer' }, 'column');
	assert.deepEqual(calls, [
		{
			kind: 'Matrix',
			media: 'matrix',
			edit: { kind: 'InspectViewport', viewport: state.viewport, selection: state.selected }
		}
	]);
	await controller.edit({
		kind: 'SetCells',
		assignments: [
			{ layer: 'layer', column: 'column', source: { media: 'source', channel: 'source-channel' } }
		]
	});
	await flush();
	assert.equal(
		calls.filter((edit) => edit.kind === 'Matrix' && edit.edit.kind === 'LaunchColumn').length,
		0
	);
	await controller.edit({ kind: 'LaunchColumn', column: 'column', mode: 'Configured' });
	await flush();
	assert.equal(
		calls.filter((edit) => edit.kind === 'Matrix' && edit.edit.kind === 'LaunchColumn').length,
		1
	);
	rejectAssignment = true;
	await controller.edit({
		kind: 'SetCells',
		assignments: [{ layer: 'layer', column: 'column', source: null }]
	});
	await flush();
	assert.match(state.error, /dependency cycle/);
	controller.dispose();
});

test('a stale creation cannot select a backend identity in the replacement project', async () => {
	const session = connection(),
		state = matrixState(),
		creation = deferred<MediaEditResult>();
	const controller = new MatrixController(
		() => session,
		state,
		choiceState(),
		choiceState(),
		async () => creation.promise
	);
	controller.observe(false);
	const task = controller.create();
	session.snapshotGeneration++;
	controller.observe(false);
	creation.resolve({ kind: 'Created', reference: { media: 'old-created', channel: 'final' } });
	assert.equal(await task, undefined);
	await flush();
	assert.equal(state.media, null);
	assert.equal(state.pending, false);
	controller.dispose();
});

test('selection changes coalesce with viewport reads and suppress superseded Inspector replies', async () => {
	const session = connection(),
		state = matrixState();
	state.media = 'matrix';
	const reads: { edit: MediaEdit; reply: ReturnType<typeof deferred<MediaEditResult>> }[] = [];
	const controller = new MatrixController(
		() => session,
		state,
		choiceState(),
		choiceState(),
		async (_, edit) => {
			const reply = deferred<MediaEditResult>();
			reads.push({ edit, reply });
			return reply.promise;
		}
	);
	const first = { item: { kind: 'Layer' as const, id: 'first' }, column: 'first-column' };
	const last = { item: { kind: 'Layer' as const, id: 'last' }, column: 'last-column' };
	controller.select(first.item, first.column);
	controller.select(last.item, last.column);
	assert.equal(reads.length, 1);
	reads[0].reply.resolve(snapshot('matrix'));
	await flush();
	assert.equal(state.window, null, 'superseded selection must never publish');
	assert.equal(reads.length, 2);
	assert.deepEqual(reads[1].edit, {
		kind: 'Matrix',
		media: 'matrix',
		edit: { kind: 'InspectViewport', viewport: state.viewport, selection: last }
	});
	const current = snapshot('matrix');
	assert(current.kind === 'MatrixViewport');
	current.snapshot.selected = { selection: last, row: null, column: null, cell: null };
	reads[1].reply.resolve(current);
	await flush();
	assert.deepEqual(controller.window?.selected?.selection, last);
	controller.select(last.item, last.column);
	assert.equal(reads.length, 2, 'reselect does not create a duplicate read');
	controller.dispose();
});

test('current creation returns the committed backend channel identity for shared Inspector selection', async () => {
	const session = connection(),
		state = matrixState();
	const created: MediaEditResult = {
		kind: 'Created',
		reference: { media: 'created', channel: 'final' }
	};
	const controller = new MatrixController(
		() => session,
		state,
		choiceState(),
		choiceState(),
		async (_session, edit) => {
			if (edit.kind === 'CreateMedia') return created;
			if (edit.kind === 'Matrix') return snapshot('created');
			return { kind: 'Choices', page: { choices: [], total: 0, start: 0 } };
		}
	);
	assert.deepEqual(await controller.create(), created);
	assert.equal(state.media, 'created');
	controller.dispose();
});
