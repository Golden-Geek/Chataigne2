import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createSurfaceKeyboard, type KeyboardTarget } from '../surface-keyboard.ts';
import type { PixelOffset } from '../generated/PixelOffset';

const target: KeyboardTarget = {
	document: '1:screen',
	reference: { screen: 'screen', surface: 'surface' },
	corner: 0,
	origin: { x: 0.1, y: 0.1 }
};
const key = (name: string, options = {}) => ({
	key: name,
	repeat: false,
	shiftKey: false,
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	...options
});
function fixture(allowed = true) {
	const samples: PixelOffset[] = [];
	let begins = 0,
		ends = 0,
		stops = 0;
	const keyboard = createSurfaceKeyboard({
		start: () => {
			begins++;
			return allowed;
		},
		move: (pixels) => samples.push(pixels),
		finish: () => {
			ends++;
		},
		stop: () => {
			stops++;
		}
	});
	return { keyboard, samples, counts: () => ({ begins, ends, stops }) };
}
test('repeated arrows and Shift retain every integer input in one held gesture', () => {
	const f = fixture();
	assert(f.keyboard.keydown(key('ArrowRight'), target));
	for (let i = 0; i < 99; i++)
		assert(f.keyboard.keydown(key('ArrowRight', { repeat: true }), target));
	f.keyboard.keydown(key('ArrowDown', { shiftKey: true }), target);
	f.keyboard.observe({ ...target, origin: { x: 0.4, y: 0.4 } }, true);
	f.keyboard.keyup('ArrowRight');
	assert.deepEqual(f.counts(), { begins: 1, ends: 0, stops: 0 });
	f.keyboard.keyup('Shift');
	f.keyboard.keyup('ArrowDown');
	f.keyboard.keyup('ArrowDown');
	assert.deepEqual(f.samples.at(-1), { x: 100, y: 10 });
	assert.deepEqual(f.counts(), { begins: 1, ends: 1, stops: 0 });
});
test('selection/document/ownership loss cancels and a stale repeat cannot reopen', () => {
	for (const replacement of [
		null,
		{ ...target, document: '2:screen' },
		{ ...target, corner: 1 },
		{ ...target, reference: { ...target.reference, surface: 'other' } }
	]) {
		const f = fixture();
		f.keyboard.keydown(key('ArrowUp'), target);
		f.keyboard.observe(replacement, true);
		assert(!f.keyboard.keydown(key('ArrowUp', { repeat: true }), replacement));
		f.keyboard.keyup('ArrowUp');
		assert.deepEqual(f.counts(), { begins: 1, ends: 0, stops: 1 });
	}
	const f = fixture();
	f.keyboard.keydown(key('ArrowLeft'), target);
	f.keyboard.observe(target, false);
	f.keyboard.cancel();
	assert.deepEqual(f.counts(), { begins: 1, ends: 0, stops: 1 });
});
test('focus release flushes once; modifiers, absent target and denied begin produce no samples', () => {
	const f = fixture();
	for (const modifier of ['altKey', 'ctrlKey', 'metaKey'])
		assert(!f.keyboard.keydown(key('ArrowRight', { [modifier]: true }), target));
	assert(!f.keyboard.keydown(key('Enter'), target));
	assert(!f.keyboard.keydown(key('ArrowRight'), null));
	assert.deepEqual(f.counts(), { begins: 0, ends: 0, stops: 0 });
	f.keyboard.keydown(key('ArrowLeft'), target);
	f.keyboard.finish();
	f.keyboard.finish();
	assert.deepEqual(f.samples, [{ x: -1, y: 0 }]);
	assert.deepEqual(f.counts(), { begins: 1, ends: 1, stops: 0 });
	const denied = fixture(false);
	assert(!denied.keyboard.keydown(key('ArrowRight'), target));
	assert.equal(denied.samples.length, 0);
});

test('separate taps wait for closure and begin from the latest accepted origin', () => {
	const starts: KeyboardTarget[] = [];
	const samples: PixelOffset[] = [];
	let finishes = 0;
	const keyboard = createSurfaceKeyboard({
		start: (current) => (starts.push(current), true),
		move: (pixels) => samples.push(pixels),
		finish: () => finishes++,
		stop: () => assert.fail('valid queued gestures must not cancel')
	});
	keyboard.keydown(key('ArrowRight'), target);
	keyboard.keyup('ArrowRight');
	keyboard.observe(target, true, false);
	keyboard.keydown(key('ArrowDown', { shiftKey: true }), target);
	keyboard.keyup('ArrowDown');
	assert.equal(starts.length, 1);
	assert.equal(finishes, 1);
	keyboard.observe({ ...target, origin: { x: 0.2, y: 0.3 } }, false, true);
	assert.deepEqual(starts[1].origin, { x: 0.2, y: 0.3 });
	assert.deepEqual(samples, [
		{ x: 1, y: 0 },
		{ x: 0, y: 10 }
	]);
	assert.equal(finishes, 2);
});

test('queued taps are bounded and never replay after focus, target or connection loss', () => {
	for (const loss of ['focus', 'target', 'connection']) {
		const f = fixture();
		f.keyboard.observe(target, false, false);
		for (let i = 0; i < 32; i++) {
			assert(f.keyboard.keydown(key('ArrowRight'), target));
			f.keyboard.keyup('ArrowRight');
		}
		assert(!f.keyboard.keydown(key('ArrowRight'), target));
		if (loss === 'focus') f.keyboard.finish();
		else f.keyboard.observe(loss === 'connection' ? null : { ...target, corner: 1 }, false);
		f.keyboard.observe(target, false, true);
		assert.equal(f.samples.length, 0);
		assert.equal(f.counts().begins, 0);
	}
});
