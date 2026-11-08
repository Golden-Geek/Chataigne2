import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createVideoSourceState, VideoSourceReader } from '../video-source-reader.ts';
import type { VideoSourceStatus } from '../generated/VideoSourceStatus';

const ready: VideoSourceStatus = {
	reference: { media: 'media', channel: 'channel' },
	state: 'Ready',
	duration_ms: 250,
	error: null,
	retrying: false
};
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}
function timers() {
	const pending: Array<{ run: () => void; cancelled: boolean }> = [];
	return {
		pending,
		schedule(run: () => void) {
			const item = { run, cancelled: false };
			pending.push(item);
			return () => {
				item.cancelled = true;
			};
		}
	};
}

test('only one selected-source read runs at a time and null reads preserve healthy status', async () => {
	const state = createVideoSourceState(),
		timer = timers(),
		first = deferred<VideoSourceStatus>();
	let calls = 0;
	const reader = new VideoSourceReader(
		state,
		async () => {
			calls++;
			return calls === 1 ? first.promise : null;
		},
		() => true,
		timer.schedule
	);
	const stop = reader.start();
	assert.equal(calls, 1);
	assert.equal(timer.pending.length, 0);
	first.resolve(ready);
	await flush();
	assert.deepEqual(state.status, ready);
	assert.equal(timer.pending.length, 1);
	timer.pending[0].run();
	await flush();
	assert.equal(calls, 2);
	assert.deepEqual(state.status, ready);
	stop();
	assert.equal(state.status, null);
	assert(timer.pending[1].cancelled);
});

test('query failures clear stale readiness and recover on the next bounded retry', async () => {
	const state = createVideoSourceState(),
		timer = timers();
	let calls = 0;
	const reader = new VideoSourceReader(
		state,
		async () => {
			if (++calls === 1) throw new Error('offline');
			return ready;
		},
		() => true,
		timer.schedule
	);
	const stop = reader.start();
	await flush();
	assert.equal(state.status, null);
	assert.match(state.error, /offline/);
	timer.pending[0].run();
	await flush();
	assert.deepEqual(state.status, ready);
	assert.equal(state.error, '');
	stop();
});

test('late reads and cancelled timers cannot overwrite a replacement owner or cancel its retry', async () => {
	const state = createVideoSourceState(),
		timer = timers(),
		first = deferred<VideoSourceStatus>();
	let calls = 0;
	const reader = new VideoSourceReader(
		state,
		async () => (++calls === 1 ? first.promise : ready),
		() => true,
		timer.schedule
	);
	const oldStop = reader.start();
	const stop = reader.start();
	await flush();
	assert.deepEqual(state.status, ready);
	oldStop();
	assert.deepEqual(state.status, ready);
	first.resolve({ ...ready, state: 'Unavailable' });
	await flush();
	assert.deepEqual(state.status, ready);
	const oldTimer = timer.pending[0];
	const latestStop = reader.start();
	await flush();
	oldTimer.run();
	await flush();
	assert.deepEqual(state.status, ready);
	latestStop();
	assert(timer.pending.at(-1)!.cancelled);
	stop();
});

test('project/loading/disconnect invalidation rejects pending results and stops polling', async () => {
	const state = createVideoSourceState(),
		timer = timers(),
		pending = deferred<VideoSourceStatus>();
	let live = true;
	const reader = new VideoSourceReader(
		state,
		() => pending.promise,
		() => live,
		timer.schedule
	);
	const stop = reader.start();
	live = false;
	pending.resolve(ready);
	await flush();
	assert.equal(state.status, null);
	assert.equal(timer.pending.length, 0);
	stop();
});
