import { test } from 'vitest';
import assert from 'node:assert/strict';
import { createCurveInput } from '../input';
import type { ActiveKeyDrag } from '../types';

function fixture() {
	const calls: string[] = [];
	const held = new Set([7]);
	let resolveWrite!: () => void;
	const pending = new Promise<void>((resolve) => {
		resolveWrite = resolve;
	});
	let edit = true;
	const previews: unknown[] = [];
	const context: Parameters<typeof createCurveInput>[0] = {
		canvas: {
			hasPointerCapture: (id: number) => held.has(id),
			releasePointerCapture(id: number) {
				held.delete(id);
				calls.push('release');
				assert.equal(input.cancel(id), false, 'ownership retired before reentrant loss');
			}
		} as HTMLCanvasElement,
		key: null,
		bezier: null,
		curve: null,
		pan: null,
		draw: null,
		box: null,
		keyPreviews: new Map([[2, { position: 0.4, value: 0.6 }]]),
		easingPreviews: new Map([
			[2, { out_position: 0.2, out_value: 0.3, in_position: -0.2, in_value: -0.3 }]
		]),
		flush() {
			calls.push('flush');
		},
		discard() {
			calls.push('discard');
		},
		takeEdit() {
			if (!edit) return null;
			edit = false;
			return {
				session: {
					active: true,
					begin: async () => {},
					end: async () => {
						calls.push('end');
					}
				},
				begin: Promise.resolve(),
				pending: [pending]
			};
		},
		clearPreviews(keys, easing) {
			previews.push(keys, easing);
			calls.push('clear');
		},
		completeBox() {
			calls.push('box');
		},
		completeDraw() {
			calls.push('draw');
		},
		restoreView() {
			calls.push('restore');
		},
		clearHover() {}
	};
	const input = createCurveInput(context);
	const key: ActiveKeyDrag = {
		pointer_id: 7,
		anchor_key_id: 2,
		anchor_start_position: 0,
		anchor_start_value: 0,
		origins: [{ key_id: 2, position: 0, value: 0 }],
		position_delta_constraint: null,
		bezier_alignment_constraints: [],
		touched_bezier_key_ids: new Set([2])
	};
	return { context, input, calls, held, key, previews, resolveWrite };
}

test('cancellation rejects a foreign pointer, drops queued edits and lets submitted work settle', async () => {
	const f = fixture();
	f.context.key = f.key;
	assert.equal(f.input.cancel(8), false);
	assert.deepEqual(f.calls, []);
	assert.equal(f.input.cancel(7), true);
	assert.deepEqual(f.calls, ['discard', 'release', 'clear']);
	assert.equal(f.context.key, null);
	assert.equal(f.held.size, 0);
	f.input.finish(7);
	assert.deepEqual(
		f.calls,
		['discard', 'release', 'clear'],
		'late release cannot flush canceled input'
	);
	const keyPreview = f.context.keyPreviews.get(2);
	f.context.keyPreviews.set(2, { position: 0.8, value: 0.9 });
	f.resolveWrite();
	await new Promise((resolve) => setTimeout(resolve, 0));
	assert.deepEqual(f.calls, ['discard', 'release', 'clear', 'end']);
	assert.equal(
		(f.previews[0] as Map<number, unknown>).get(2),
		keyPreview,
		'old settlement receives only old preview identities'
	);
});

test('normal release flushes once and closes its edit after submitted work', async () => {
	const f = fixture();
	f.context.key = f.key;
	f.input.finish(7);
	f.input.finish(7);
	assert.deepEqual(f.calls, ['flush', 'release']);
	f.resolveWrite();
	await new Promise((resolve) => setTimeout(resolve, 0));
	assert.deepEqual(f.calls, ['flush', 'release', 'clear', 'end']);
});

for (const mode of ['replace', 'add', 'toggle'] as const) {
	test(`${mode} marquee cancel never completes selection`, () => {
		const f = fixture();
		f.context.box = {
			pointer_id: 7,
			start_screen_x: 0,
			start_screen_y: 0,
			current_screen_x: 100,
			current_screen_y: 100,
			mode
		};
		assert.equal(f.input.cancel(8), false);
		assert.equal(f.input.cancel(7), true);
		f.input.finish(7);
		assert.deepEqual(f.calls, ['release']);
	});
}

test('draw cancel discards its path; normal release alone commits it', () => {
	for (const cancel of [false, true]) {
		const f = fixture();
		f.context.draw = { pointer_id: 7, points: [], path_buckets: [] };
		if (cancel) f.input.cancel(7);
		else f.input.finish(7);
		assert.deepEqual(f.calls, cancel ? ['release'] : ['release', 'draw']);
		assert.equal(f.context.draw, null);
	}
});

test('pan cancel restores its basis; normal release retains navigation', () => {
	for (const cancel of [false, true]) {
		const f = fixture();
		f.context.pan = {
			pointer_id: 7,
			start_screen_x: 0,
			start_screen_y: 0,
			start_bounds: { x_min: 0, x_max: 1, y_min: 0, y_max: 1 }
		};
		if (cancel) f.input.cancel(7);
		else f.input.finish(7);
		assert.deepEqual(f.calls, cancel ? ['restore', 'release'] : ['release']);
	}
});

test('Bezier cancellation and independent controllers do not flush or share ownership', async () => {
	const a = fixture(),
		b = fixture();
	a.context.bezier = {
		pointer_id: 7,
		ref: { key_id: 2, kind: 'out' },
		opposite_ref: null,
		opposite_length: 0,
		touched_key_ids: new Set([2])
	};
	b.context.key = b.key;
	a.input.cancel();
	assert.deepEqual(b.calls, []);
	assert.equal(b.context.key, b.key);
	assert.deepEqual(a.calls, ['discard', 'release', 'clear']);
	a.resolveWrite();
	await new Promise((resolve) => setTimeout(resolve, 0));
	assert.deepEqual(a.calls, ['discard', 'release', 'clear', 'end']);
});
