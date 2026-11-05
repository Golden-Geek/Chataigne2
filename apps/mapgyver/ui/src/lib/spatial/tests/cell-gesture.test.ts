import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createLatestEditGestureState } from 'golden_ui/store/latest-edit-gesture';
import { createCellGesture, type CellHandle } from '../cell-gesture.ts';
import type { MediaEdit } from '../../media/generated/MediaEdit';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const result = {
	kind: 'MatrixCellGeometryChanged',
	media: 'matrix',
	cell: 'cell',
	settings: { position: [3, 4], size: null, fit_mode: 'Fit', opacity: 0.5, enabled: false }
} satisfies MediaEditResult;

test('Cell translation and resize keep immutable typed bases with one active/latest sample and one history group', async () => {
	for (const handle of [
		{ kind: 'Translate', origin: [2, 3], start: { x: 0, y: 0 } },
		{
			kind: 'Resize',
			origin: { position: [2, 3], size: { width: 8, height: 4 } },
			handle: 'BottomRight',
			start: { x: 0, y: 0 }
		}
	] satisfies CellHandle[]) {
		const host = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } },
			state = createLatestEditGestureState();
		let resolve!: (result: MediaEditResult) => void;
		const pending = new Promise<MediaEditResult>((yes) => {
			resolve = yes;
		});
		const requests: MediaEdit[] = [],
			accepted: unknown[] = [];
		let ended = 0;
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
		const gesture = createCellGesture(() => host, state, {
			group: () => group,
			invoke: async (_, edit) => {
				requests.push(edit);
				return requests.length === 1 ? pending : result;
			},
			accepted: (value) => accepted.push(value),
			active() {},
			finished() {}
		});
		gesture.start({ media: 'matrix', cell: 'cell' }, handle);
		gesture.move({ x: 1, y: 2 });
		await flush();
		for (let step = 2; step <= 100; step++) gesture.move({ x: step, y: 2 });
		gesture.finish();
		assert.equal(requests.length, 1);
		resolve(result);
		await flush();
		await flush();
		assert.deepEqual(requests[1], {
			kind: 'Matrix',
			media: 'matrix',
			edit: { kind: 'EditCellGeometry', cell: 'cell', edit: { ...handle, point: { x: 100, y: 2 } } }
		});
		assert.equal(accepted.length, 2);
		assert.equal(ended, 1);
		assert.equal(state.active, false);
	}
});

test('Cell geometry drops late project replies and retires the owned group after cancellation', async () => {
	const host = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } },
		state = createLatestEditGestureState();
	let resolve!: (result: MediaEditResult) => void,
		ended = 0;
	const pending = new Promise<MediaEditResult>((yes) => {
		resolve = yes;
	});
	const accepted: unknown[] = [];
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
	const gesture = createCellGesture(() => host, state, {
		group: () => group,
		invoke: () => pending,
		accepted: (value) => accepted.push(value),
		active() {},
		finished() {}
	});
	gesture.start(
		{ media: 'matrix', cell: 'cell' },
		{ kind: 'Translate', origin: [0, 0], start: { x: 0, y: 0 } }
	);
	gesture.move({ x: 2, y: 3 });
	await flush();
	host.snapshotGeneration++;
	gesture.observe();
	resolve(result);
	await flush();
	await flush();
	assert.deepEqual(accepted, []);
	assert.equal(ended, 1);
	assert.equal(state.active, false);
});
