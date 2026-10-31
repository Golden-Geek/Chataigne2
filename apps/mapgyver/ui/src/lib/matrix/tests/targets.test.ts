import { test } from 'node:test';
import assert from 'node:assert/strict';
import { TargetReader, targetState, targetKey } from '../target-choices.ts';
import { MatrixController, matrixState } from '../authoring.ts';
import { choiceState } from '../choices.ts';
import type { MatrixTargetScope } from '../../media/generated/MatrixTargetScope';
import type { MediaEdit } from '../../media/generated/MediaEdit';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
const flush = async () => {
	for (let i = 0; i < 12; i++) await Promise.resolve();
};
const connection = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	currentHistoryStateId: 0,
	hasActiveEditSession: false,
	loading: { visible: false }
});
function deferred<T>() {
	let resolve!: (value: T) => void, reject!: (value: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}
const page = (label: string): MediaEditResult => ({
	kind: 'MatrixTargets',
	page: { total: 1, start: 0, choices: [{ id: { kind: 'Column', column: 'column' }, label }] }
});

test('destination search captures bounded explicit scope and discards old group/search replies', async () => {
	const session = connection(),
		state = targetState();
	let scope: MatrixTargetScope = { kind: 'Items', parent: 'first-group' };
	const calls: { edit: MediaEdit; reply: ReturnType<typeof deferred<MediaEditResult>> }[] = [];
	const reader = new TargetReader(
		() => session,
		() => ({ media: 'matrix', scope }),
		state,
		async (_, edit) => {
			const reply = deferred<MediaEditResult>();
			calls.push({ edit, reply });
			return reply.promise;
		}
	);
	reader.refresh();
	scope = { kind: 'Items', parent: 'new-group' };
	for (let i = 0; i < 100; i++) {
		state.search = `target ${i}`;
		reader.refresh();
	}
	assert.equal(calls.length, 1);
	calls[0].reply.resolve(page('stale group'));
	await flush();
	assert.equal(state.page === null, true);
	assert.equal(calls.length, 2);
	assert.deepEqual(calls[1].edit, {
		kind: 'Matrix',
		media: 'matrix',
		edit: { kind: 'InspectTargets', query: { scope, search: 'target 99', start: 0, count: 64 } }
	});
	calls[1].reply.resolve(page('current'));
	await flush();
	assert.equal(state.page?.choices[0].label, 'current');
	reader.dispose();
});

test('target reads fence project/history changes, disconnect and disposal without late errors', async () => {
	for (const reason of ['project', 'history', 'disconnect', 'dispose']) {
		const session = connection(),
			state = targetState(),
			reply = deferred<MediaEditResult>();
		const reader = new TargetReader(
			() => session,
			() => ({ media: 'matrix', scope: { kind: 'Columns' } }),
			state,
			async () => reply.promise
		);
		reader.refresh();
		if (reason === 'project') session.snapshotGeneration++;
		if (reason === 'history') session.currentHistoryStateId++;
		if (reason === 'disconnect') session.status = 'disconnected';
		if (reason === 'dispose') reader.dispose();
		reply.reject(new Error('old target failure'));
		await flush();
		assert.equal(state.page, null);
		assert.equal(state.error, '');
		assert.equal(state.loading, false);
		reader.dispose();
	}
});

test('row and column selection work independently without creating commands or launching', () => {
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
			return { kind: 'Changed' };
		}
	);
	controller.select({ kind: 'Group', id: 'group' }, null);
	assert.deepEqual(state.selected, { item: { kind: 'Group', id: 'group' }, column: null });
	controller.select(null, 'column');
	assert.deepEqual(state.selected, { item: null, column: 'column' });
	assert.equal(calls.length, 0);
	assert.equal(
		targetKey({ kind: 'Item', item: { id: 'group', kind: 'Group' } }),
		targetKey({ kind: 'Item', item: { kind: 'Group', id: 'group' } })
	);
	controller.dispose();
});

test('destination reads defer through shared edit groups and inspect only after the group finishes', async () => {
	const session = connection(),
		state = targetState();
	let calls = 0;
	const reader = new TargetReader(
		() => session,
		() => ({ media: 'matrix', scope: { kind: 'Groups' } }),
		state,
		async () => {
			calls++;
			return page('group');
		}
	);
	session.hasActiveEditSession = true;
	for (let i = 0; i < 100; i++) reader.refresh();
	assert.equal(calls, 0);
	assert.equal(state.loading, false);
	session.hasActiveEditSession = false;
	reader.refresh();
	await flush();
	assert.equal(calls, 1);
	assert.equal(state.page?.choices[0].label, 'group');
	reader.dispose();
});
