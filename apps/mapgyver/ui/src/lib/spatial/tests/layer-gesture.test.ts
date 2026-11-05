import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createLatestEditGestureState } from 'golden_ui/store/latest-edit-gesture';
import { createLayerGesture } from '../layer-gesture.ts';
import type { MediaEdit } from '../../media/generated/MediaEdit';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const result = {
	kind: 'MatrixLayerGeometryChanged',
	media: 'matrix',
	layer: 'layer',
	settings: { position: [3, 4], size: { width: 80, height: 40 }, duration_ms: 777 }
} satisfies MediaEditResult;
const handle = {
	mode: 'Translate' as const,
	origin: { position: [0, 0] as [number, number], size: { width: 80, height: 40 } },
	start: { x: 0, y: 0 }
};

test('Layer geometry shares one edit group and one active/latest sample while retaining exact identities and basis', async () => {
	let resolve!: (result: MediaEditResult) => void;
	const pending = new Promise<MediaEditResult>((yes) => {
		resolve = yes;
	});
	const host = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } };
	const requests: MediaEdit[] = [],
		accepted: unknown[] = [];
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
	const state = createLatestEditGestureState();
	const gesture = createLayerGesture(() => host, state, {
		group: () => group,
		invoke: async (_, edit) => {
			requests.push(edit);
			return requests.length === 1 ? pending : result;
		},
		accepted: (value) => accepted.push(value),
		active() {},
		finished() {}
	});
	assert(gesture.start({ media: 'matrix', layer: 'layer' }, handle));
	gesture.move({ x: 1, y: 1 });
	await flush();
	for (let step = 2; step <= 100; step++) gesture.move({ x: step, y: 4 });
	gesture.finish();
	assert.equal(requests.length, 1);
	resolve(result);
	await flush();
	await flush();
	assert.equal(requests.length, 2);
	assert.deepEqual(requests[1], {
		kind: 'Matrix',
		media: 'matrix',
		edit: {
			kind: 'EditLayerGeometry',
			layer: 'layer',
			edit: { ...handle, point: { x: 100, y: 4 } }
		}
	});
	assert.equal(ends, 1);
	assert.equal(accepted.length, 2);
	assert.equal(state.active, false);
});

test('Layer project/snapshot replacement fences late receipts and always ends the owned group', async () => {
	let resolve!: (result: MediaEditResult) => void;
	const pending = new Promise<MediaEditResult>((yes) => {
		resolve = yes;
	});
	const host = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } };
	const accepted: unknown[] = [];
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
	const state = createLatestEditGestureState();
	const gesture = createLayerGesture(() => host, state, {
		group: () => group,
		invoke: () => pending,
		accepted: (value) => accepted.push(value),
		active() {},
		finished() {}
	});
	gesture.start({ media: 'matrix', layer: 'layer' }, handle);
	gesture.move({ x: 2, y: 3 });
	await flush();
	host.snapshotGeneration++;
	gesture.observe();
	resolve(result);
	await flush();
	await flush();
	assert.equal(accepted.length, 0);
	assert.equal(ends, 1);
	assert.equal(state.active, false);
});
