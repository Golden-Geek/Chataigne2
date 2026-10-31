import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
	SurfaceGesture,
	createSurfaceGestureState,
	type SurfaceHandle
} from '../surface-gesture.ts';
import type { SurfaceGeometryEdit } from '../generated/SurfaceGeometryEdit';
import type { Point } from '../generated/Point';
import type { SurfaceSnapshot } from '../generated/SurfaceSnapshot';

const reference = { screen: 'screen', surface: 'surface' };
const deferred = <T>() => {
	let resolve!: (value: T) => void;
	let reject!: (error: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
};
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
function fixture(handle: SurfaceHandle = { kind: 'QuadCorner', corner: 0 }) {
	const host = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } };
	let current = host;
	const state = createSurfaceGestureState();
	const pending: ReturnType<typeof deferred<SurfaceSnapshot>>[] = [];
	const points: Point[] = [];
	const edits: SurfaceGeometryEdit[] = [];
	const accepted: SurfaceSnapshot[] = [];
	const active: boolean[] = [];
	let begins = 0,
		ends = 0,
		finished = 0;
	const group = {
		active: false,
		async begin() {
			begins++;
			this.active = true;
		},
		async end() {
			ends++;
			this.active = false;
		}
	};
	const gesture = new SurfaceGesture(() => current, state, {
		group: () => group,
		move: async (_host, _reference, edit) => {
			points.push(edit.point);
			edits.push(edit);
			const request = deferred<SurfaceSnapshot>();
			pending.push(request);
			return request.promise;
		},
		accepted: (snapshot) => accepted.push(snapshot),
		active: (value) => active.push(value),
		finished: () => {
			finished++;
		}
	});
	return {
		host,
		state,
		gesture,
		start: () => gesture.start(reference, handle),
		edits,
		pending,
		points,
		accepted,
		active,
		group,
		replace() {
			current = { ...host };
		},
		counts: () => ({ begins, ends, finished })
	};
}
const snapshot = (x: number): SurfaceSnapshot => ({
	reference,
	label: 'Surface',
	definition: {
		source: null,
		opacity: 1,
		enabled: true,
		calibration: 'Off',
		crop: { x: 0, y: 0, width: 1, height: 1 },
		quad: {
			corners: [
				{ x, y: 0 },
				{ x: 1, y: 0 },
				{ x: 1, y: 1 },
				{ x: 0, y: 1 }
			]
		}
	}
});

test('one in-flight request and one latest sample drain into exactly one group', async () => {
	const f = fixture();
	assert(f.start());
	f.gesture.move({ x: 0.01, y: 0.1 });
	await flush();
	for (let i = 2; i <= 100; i++) f.gesture.move({ x: i / 1000, y: 0.1 });
	f.gesture.finish();
	assert.equal(f.pending.length, 1);
	f.pending[0].resolve(snapshot(0.01));
	await flush();
	assert.equal(f.points.length, 2);
	assert.deepEqual(f.points[0], { x: 0.01, y: 0.1 });
	assert.deepEqual(f.points[1], { x: 0.1, y: 0.1 });
	f.pending[1].resolve(snapshot(0.1));
	await flush();
	assert.deepEqual(f.counts(), { begins: 1, ends: 1, finished: 1 });
	assert.deepEqual(f.active, [true, false]);
	assert.equal(f.state.active, false);
});

test('invalid sample can recover and stopping discards only unsent input', async () => {
	const f = fixture();
	f.start();
	f.gesture.move({ x: 2, y: 2 });
	await flush();
	f.pending[0].reject(new Error('invalid quad'));
	await flush();
	assert.match(f.state.error, /invalid quad/);
	f.gesture.move({ x: 0.1, y: 0.1 });
	await flush();
	f.gesture.move({ x: 0.2, y: 0.2 });
	f.gesture.stop();
	f.pending[1].resolve(snapshot(0.1));
	await flush();
	assert.equal(f.points.length, 2);
	assert.equal(f.accepted.length, 1);
	assert.equal(f.state.error, '');
	assert.deepEqual(f.counts(), { begins: 1, ends: 1, finished: 1 });
});

test('project replacement, disconnect, disposal and lost group fence late results', async () => {
	for (const mode of ['generation', 'replacement', 'disconnect', 'dispose', 'group']) {
		const f = fixture();
		f.start();
		f.gesture.move({ x: 0.1, y: 0.1 });
		await flush();
		if (mode === 'generation') f.host.snapshotGeneration++;
		else if (mode === 'replacement') f.replace();
		else if (mode === 'disconnect') f.host.status = 'disconnected';
		else if (mode === 'dispose') f.gesture.dispose();
		else f.group.active = false;
		f.gesture.observe();
		f.pending[0].resolve(snapshot(0.1));
		await flush();
		assert.equal(f.accepted.length, 0, mode);
		assert.equal(f.state.active, false, mode);
		assert.equal(f.counts().ends, 1, mode);
	}
});

test('disposal while BeginEdit is pending never sends a sample and closes its group', async () => {
	const f = fixture();
	const begin = deferred<void>();
	f.group.begin = async () => {
		await begin.promise;
		f.group.active = true;
	};
	f.start();
	f.gesture.move({ x: 0.1, y: 0.1 });
	f.gesture.dispose();
	begin.resolve();
	await flush();
	assert.equal(f.points.length, 0);
	assert.equal(f.counts().ends, 1);
	assert.equal(f.state.active, false);
	assert.equal(f.start(), false);
});

test('a rejected BeginEdit surfaces the error and releases local gesture state', async () => {
	const f = fixture();
	f.group.begin = async () => {
		throw new Error('begin refused');
	};
	f.start();
	f.gesture.move({ x: 0.1, y: 0.1 });
	await flush();
	assert.equal(f.points.length, 0);
	assert.equal(f.state.active, false);
	assert.match(f.state.error, /begin refused/);
});

test('crop resize and origin handles use the same exclusive bounded gesture', async () => {
	for (const handle of [
		{ kind: 'CropCorner', corner: 2 },
		{ kind: 'CropOrigin' }
	] satisfies SurfaceHandle[]) {
		const f = fixture(handle);
		assert(f.start());
		assert.equal(f.start(), false);
		f.gesture.move({ x: 0.5, y: 0.6 });
		f.gesture.finish();
		await flush();
		assert.deepEqual(f.edits[0], { ...handle, point: { x: 0.5, y: 0.6 } });
		f.pending[0].resolve(snapshot(0));
		await flush();
		assert.deepEqual(f.counts(), { begins: 1, ends: 1, finished: 1 });
	}
});
