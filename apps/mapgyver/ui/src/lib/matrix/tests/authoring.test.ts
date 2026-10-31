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
function snapshot(media: string, row = 0): MediaEditResult {
	return {
		kind: 'MatrixViewport',
		snapshot: {
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
								empty_cell: 'Transparent',
								transition: { duration_ms: 250, curve: 'Linear' }
							}
						}
					},
					size: { kind: 'Automatic' },
					filter: 'Linear'
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
			playback: {
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
	assert.equal(calls.length, 0);
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
	await task;
	await flush();
	assert.equal(state.media, null);
	assert.equal(state.pending, false);
	controller.dispose();
});
