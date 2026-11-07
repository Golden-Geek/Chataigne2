import assert from 'node:assert/strict';
import { test } from 'vitest';
import { EditorReadQueue, editorHistoryMatches } from '../editor-read-queue.ts';

const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const host = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	projectGeneration: 1,
	loading: { visible: false },
	hasActiveEditSession: true,
	editSessionEpoch: 4,
	currentHistoryStateId: 1
});
function deferred() {
	let resolve!: (value: number) => void;
	const promise = new Promise<number>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

test('all live editors publish same-group observations while working history advances', async () => {
	const session = host(),
		queue = new EditorReadQueue(),
		first = deferred(),
		last = deferred();
	const seen: number[] = [];
	let calls = 0;
	const observe = () =>
		queue.observe(
			session,
			'same owner',
			() => true,
			() => (++calls === 1 ? first.promise : last.promise),
			(value) => seen.push(value),
			(error) => assert.fail(String(error)),
			() => {}
		);
	observe();
	for (let index = 0; index < 100; index++) {
		session.currentHistoryStateId++;
		observe();
	}
	assert.equal(calls, 1);
	first.resolve(1);
	await flush();
	assert.deepEqual(seen, [1]);
	assert.equal(calls, 2);
	last.resolve(2);
	await flush();
	assert.deepEqual(seen, [1, 2]);
	assert.equal(editorHistoryMatches(session, -1), true);
});

test('group closure, context replacement and cancellation fence active and queued observations', async () => {
	for (const boundary of [
		'epoch',
		'group',
		'history',
		'snapshot',
		'project',
		'disconnect',
		'loading',
		'cancel'
	]) {
		const session = host(),
			queue = new EditorReadQueue(),
			old = deferred();
		const seen: number[] = [];
		queue.observe(
			session,
			'old',
			() => true,
			() => old.promise,
			(value) => seen.push(value),
			(error) => assert.fail(String(error)),
			() => {}
		);
		if (boundary === 'epoch') session.editSessionEpoch++;
		if (boundary === 'group') session.hasActiveEditSession = false;
		if (boundary === 'history') {
			session.hasActiveEditSession = false;
			session.currentHistoryStateId++;
		}
		if (boundary === 'snapshot') session.snapshotGeneration++;
		if (boundary === 'project') session.projectGeneration++;
		if (boundary === 'disconnect') session.status = 'disconnected';
		if (boundary === 'loading') session.loading.visible = true;
		if (boundary === 'cancel') queue.invalidate();
		old.resolve(1);
		await flush();
		assert.deepEqual(seen, [], boundary);
	}
});

test('an unknown project cannot start an editor inspection', () => {
	const session = { ...host(), projectGeneration: null };
	const queue = new EditorReadQueue();
	let calls = 0;
	queue.observe(
		session,
		'owner',
		() => true,
		async () => ++calls,
		() => {},
		() => {},
		() => {}
	);
	assert.equal(calls, 0);
});
