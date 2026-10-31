import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createLatestEditGestureState } from 'golden_ui/store/latest-edit-gesture';
import { createInstanceGesture } from '../instance-gesture.ts';
import type { InstanceGeometryMode } from '../../media/generated/InstanceGeometryMode';
import type { MediaEdit } from '../../media/generated/MediaEdit';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
import type { InstanceSnapshot } from '../../media/generated/InstanceSnapshot';

const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const deferred = <T>() => {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((yes) => {
		resolve = yes;
	});
	return { promise, resolve };
};
const reference = { composition: { media: 'media', channel: 'channel' }, instance: 'instance' };
const origin = {
	position: [4, 4] as [number, number],
	scale: [-1, 2] as [number, number],
	rotation: 0.25,
	pivot: [0.2, 0.7] as [number, number]
};
const snapshot: InstanceSnapshot = {
	reference,
	label: 'Instance',
	definition: { transform: origin, source: null, opacity: 1, blend: 'Normal', enabled: true }
};

test('all instance handles use exact backend identities, immutable anchors and a bounded latest sample', async () => {
	for (const mode of ['Translate', 'Rotate', 'Scale'] satisfies InstanceGeometryMode[]) {
		const host = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } };
		const state = createLatestEditGestureState();
		const first = deferred<MediaEditResult>();
		const requests: MediaEdit[] = [];
		const accepted: InstanceSnapshot[] = [];
		let ends = 0;
		const group = {
			active: false,
			async begin() {
				this.active = true;
			},
			async end() {
				this.active = false;
				ends++;
			}
		};
		const gesture = createInstanceGesture(() => host, state, {
			group: () => group,
			invoke: async (_, edit) => {
				requests.push(edit);
				return requests.length === 1 ? first.promise : { kind: 'InstanceChanged', snapshot };
			},
			accepted: (value) => accepted.push(value),
			active: () => {},
			finished: () => {}
		});
		assert.equal(gesture.start(reference, { mode, origin, start: { x: 5, y: 6 } }), true);
		gesture.move({ x: 7, y: 8 });
		await flush();
		for (let n = 0; n < 100; n++) gesture.move({ x: n, y: n + 1 });
		gesture.finish();
		assert.equal(requests.length, 1);
		first.resolve({ kind: 'InstanceChanged', snapshot });
		await flush();
		assert.equal(requests.length, 2);
		assert.deepEqual(requests[1], {
			kind: 'Composition',
			reference: reference.composition,
			edit: {
				kind: 'EditInstanceGeometry',
				instance: reference.instance,
				edit: { mode, origin, start: { x: 5, y: 6 }, point: { x: 99, y: 100 } }
			}
		});
		assert.equal(accepted.length, 2);
		assert.equal(ends, 1);
		assert.equal(state.active, false);
	}
});

test('project replacement discards pending instance replies and unsent pointer samples', async () => {
	const host = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } };
	const state = createLatestEditGestureState();
	const reply = deferred<MediaEditResult>();
	let calls = 0,
		accepted = 0,
		ended = 0;
	const group = {
		active: false,
		async begin() {
			this.active = true;
		},
		async end() {
			this.active = false;
			ended++;
		}
	};
	const gesture = createInstanceGesture(() => host, state, {
		group: () => group,
		invoke: () => {
			calls++;
			return reply.promise;
		},
		accepted: () => {
			accepted++;
		},
		active: () => {},
		finished: () => {}
	});
	gesture.start(reference, { mode: 'Translate', origin, start: { x: 0, y: 0 } });
	gesture.move({ x: 1, y: 1 });
	await flush();
	gesture.move({ x: 100, y: 100 });
	host.snapshotGeneration++;
	gesture.observe();
	reply.resolve({ kind: 'InstanceChanged', snapshot });
	await flush();
	assert.equal(calls, 1);
	assert.equal(accepted, 0);
	assert.equal(ended, 1);
	assert.equal(state.active, false);
});
