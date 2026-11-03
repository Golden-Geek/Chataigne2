import assert from 'node:assert/strict';
import { test } from 'node:test';
import { VideoStatusController, createVideoStatusState } from '../status.ts';
import type { VideoRuntimeStatus } from '../../preview/generated/VideoRuntimeStatus';

const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
function deferred<T>() {
	let resolve!: (value: T) => void, reject!: (error: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}
const snapshot = (sequence: number): VideoRuntimeStatus => ({
	sequence,
	state: 'Running',
	submitted_fps: 60,
	cpu_submit_ms: 0.4,
	gpu: 'Test GPU',
	resources: { used_bytes: 1024, limit_bytes: 4096, in_flight: 1, retiring: 0 },
	error: null,
	screens: []
});
function fixture() {
	const reads: ReturnType<typeof deferred<VideoRuntimeStatus>>[] = [];
	const receivers: ((status: VideoRuntimeStatus) => void)[] = [];
	let unsubscribed = 0;
	const state = createVideoStatusState();
	const controller = new VideoStatusController(
		{
			available: () => true,
			subscribe: async (receive) => {
				receivers.push(receive);
				return () => {
					unsubscribed++;
				};
			},
			read: () => {
				const read = deferred<VideoRuntimeStatus>();
				reads.push(read);
				return read.promise;
			}
		},
		state
	);
	return { state, controller, reads, receivers, unsubscribed: () => unsubscribed };
}

test('subscribe-before-read retains the latest sequence and disposal fences queued events/read replies', async () => {
	const f = fixture();
	const stop = f.controller.connect();
	await flush();
	f.receivers[0](snapshot(3));
	f.reads[0].resolve(snapshot(2));
	await flush();
	assert.equal(f.state.status?.sequence, 3);
	f.receivers[0](snapshot(1));
	assert.equal(f.state.status?.sequence, 3);
	stop();
	f.receivers[0](snapshot(4));
	assert.equal(f.state.status, null);
	assert.equal(f.unsubscribed(), 1);
});
test('failed endpoint clears metrics, ignores old callbacks and reconnects without leaking subscriptions', async () => {
	const f = fixture();
	const stop = f.controller.connect();
	await flush();
	f.reads[0].reject(new Error('offline'));
	await flush();
	assert.equal(f.state.connection, 'error');
	f.receivers[0](snapshot(9));
	assert.equal(f.state.status, null);
	await new Promise((resolve) => setTimeout(resolve, 1050));
	await flush();
	assert.equal(f.reads.length, 2);
	f.reads[1].resolve(snapshot(1));
	await flush();
	assert.equal((f.state.status as VideoRuntimeStatus | null)?.sequence, 1);
	assert.equal(f.state.connection, 'connected');
	stop();
	assert.equal(f.unsubscribed(), 2);
});
test('an expired stream loses its metrics; old cleanup cannot stop a newer mount', async () => {
	const f = fixture();
	const old = f.controller.connect();
	await flush();
	f.reads[0].resolve(snapshot(4));
	await flush();
	const stop = f.controller.connect();
	await flush();
	old();
	f.reads[1].resolve(snapshot(5));
	await flush();
	assert.equal(f.state.status?.sequence, 5);
	await new Promise((resolve) => setTimeout(resolve, 3050));
	await flush();
	assert.equal(f.state.status, null);
	assert.equal(f.state.connection, 'error');
	stop();
});
test('unavailable hosts create no subscriptions or fabricated metrics', () => {
	const state = createVideoStatusState();
	const controller = new VideoStatusController(
		{
			available: () => false,
			subscribe: async () => {
				throw new Error('must not subscribe');
			},
			read: async () => {
				throw new Error('must not read');
			}
		},
		state
	);
	const stop = controller.connect();
	assert.equal(state.connection, 'unavailable');
	assert.equal(state.status, null);
	stop();
});
