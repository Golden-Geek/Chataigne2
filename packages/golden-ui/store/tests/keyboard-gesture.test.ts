import { describe, expect, it } from 'vitest';
import { KeyboardGesture, type KeyboardOffset } from '../keyboard-gesture';

const key = (name: string, options = {}) => ({
	key: name,
	repeat: false,
	shiftKey: false,
	altKey: false,
	ctrlKey: false,
	metaKey: false,
	...options
});
const target = { id: 'document:object:handle', origin: [3, 4] };
function fixture() {
	const starts: (typeof target)[] = [],
		samples: KeyboardOffset[] = [];
	let ends = 0,
		stops = 0;
	const keyboard = new KeyboardGesture<typeof target>({
		identity: (value) => value.id,
		start: (value) => (starts.push(value), true),
		move: (value) => samples.push(value),
		finish: () => ends++,
		stop: () => stops++
	});
	return { keyboard, starts, samples, counts: () => ({ ends, stops }) };
}
describe('domain-neutral held keyboard gestures', () => {
	it('retains cumulative repeats and Shift across multiple held arrows in one group', () => {
		const f = fixture();
		expect(f.keyboard.keydown(key('ArrowRight'), target)).toBe(true);
		for (let i = 1; i < 100; i++) f.keyboard.keydown(key('ArrowRight', { repeat: true }), target);
		f.keyboard.keydown(key('ArrowDown', { shiftKey: true }), target);
		f.keyboard.keyup('ArrowRight');
		expect(f.counts().ends).toBe(0);
		f.keyboard.keyup('ArrowDown');
		expect(f.starts).toHaveLength(1);
		expect(f.samples.at(-1)).toEqual({ x: 100, y: 10 });
		expect(f.counts()).toEqual({ ends: 1, stops: 0 });
	});
	it('queues distinct taps behind asynchronous closure and captures the fresh basis', () => {
		const f = fixture();
		f.keyboard.keydown(key('ArrowRight'), target);
		f.keyboard.keyup('ArrowRight');
		f.keyboard.observe(target, true, false);
		f.keyboard.keydown(key('ArrowDown'), target);
		f.keyboard.keyup('ArrowDown');
		expect(f.starts).toHaveLength(1);
		const next = { ...target, origin: [4, 4] };
		f.keyboard.observe(next, false, true);
		expect(f.starts[1]).toBe(next);
		expect(f.samples).toEqual([
			{ x: 1, y: 0 },
			{ x: 0, y: 1 }
		]);
		expect(f.counts().ends).toBe(2);
	});
	it('fences target/document/mode replacement and prevents a stale repeat from reopening', () => {
		for (const next of [
			null,
			{ ...target, id: 'different:object:handle' },
			{ ...target, id: 'document:object:other-handle' }
		]) {
			const f = fixture();
			f.keyboard.keydown(key('ArrowLeft'), target);
			f.keyboard.observe(next, true);
			expect(f.keyboard.keydown(key('ArrowLeft', { repeat: true }), next)).toBe(false);
			expect(f.counts()).toEqual({ ends: 0, stops: 1 });
		}
	});
	it('bounds pending taps and discards them on focus or availability loss', () => {
		for (const cancel of [false, true]) {
			const f = fixture();
			f.keyboard.observe(target, false, false);
			for (let i = 0; i < 32; i++) {
				expect(f.keyboard.keydown(key('ArrowUp'), target)).toBe(true);
				f.keyboard.keyup('ArrowUp');
			}
			expect(f.keyboard.keydown(key('ArrowUp'), target)).toBe(false);
			if (cancel) f.keyboard.observe(null, false);
			else f.keyboard.finish();
			f.keyboard.observe(target, false, true);
			expect(f.starts).toHaveLength(0);
		}
	});
	it('leaves modified or unrelated keys unconsumed and flushes focus loss once', () => {
		const f = fixture();
		for (const modifier of ['ctrlKey', 'altKey', 'metaKey'])
			expect(f.keyboard.keydown(key('ArrowRight', { [modifier]: true }), target)).toBe(false);
		expect(f.keyboard.keydown(key('Enter'), target)).toBe(false);
		expect(f.keyboard.keydown(key('ArrowRight'), null)).toBe(false);
		f.keyboard.keydown(key('ArrowRight'), target);
		f.keyboard.finish();
		f.keyboard.finish();
		expect(f.counts()).toEqual({ ends: 1, stops: 0 });
	});
});
