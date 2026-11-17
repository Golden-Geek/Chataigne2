import { test, expect } from 'vitest';
import { animateOverlayPaint } from '../overlay-paint';
import type { OverlayPrimitive } from '../generated/OverlayPrimitive';

function fixture() {
	let time = 0,
		sequence = 0;
	const frames = new Map<number, FrameRequestCallback>();
	const preference = Object.assign(new EventTarget(), { matches: false });
	let latest: OverlayPrimitive[] = [];
	const view = {
		performance: { now: () => time },
		matchMedia: () => preference,
		requestAnimationFrame: (callback: FrameRequestCallback) => {
			frames.set(++sequence, callback);
			return sequence;
		},
		cancelAnimationFrame: (id: number) => {
			frames.delete(id);
		}
	} as unknown as Window;
	const motion = animateOverlayPaint(view, (value) => {
		latest = value;
	});
	return {
		motion,
		frames,
		preference,
		get latest() {
			return latest;
		},
		at(now: number) {
			time = now;
			const pending = [...frames.values()];
			frames.clear();
			pending.forEach((callback) => callback(time));
		}
	};
}
const quad = (alpha = 0, position = 0): OverlayPrimitive => ({
	hit: 'object',
	shape: {
		kind: 'Quad',
		corners: [
			[position, 0],
			[1, 0],
			[1, 1],
			[position, 1]
		]
	},
	fill: [0.4, 0.6, 1, alpha],
	stroke: [0.4, 0.6, 1, 1],
	width: 1,
	dash: null
});

test('native paint presents intermediate states while target geometry, width, dash and hit IDs remain immediate', () => {
	const f = fixture();
	f.motion.update([quad()]);
	const target = { ...quad(0.1, 0.25), width: 2, dash: [3, 2] as [number, number] };
	f.motion.update([target]);
	f.at(60);
	expect(f.latest[0].fill[3]).toBeCloseTo(0.05);
	expect(f.latest[0].shape).toBe(target.shape);
	expect(f.latest[0].hit).toBe('object');
	expect(f.latest[0].width).toBe(2);
	expect(f.latest[0].dash).toBe(target.dash);
	expect(f.frames.size).toBe(1);
	f.at(120);
	expect(f.latest[0]).toBe(target);
	expect(f.frames.size).toBe(0);
	f.motion.destroy();
});

test('geometry updates do not restart a fade; rapid paint reversal starts from the displayed color', () => {
	const f = fixture();
	f.motion.update([quad()]);
	f.motion.update([quad(0.1)]);
	f.at(60);
	f.motion.update([quad(0.1, 0.4)]);
	f.at(120);
	expect(f.latest[0].fill[3]).toBe(0.1);
	expect(f.frames.size).toBe(0);
	f.motion.update([quad(0)]);
	f.at(180);
	expect(f.latest[0].fill[3]).toBeCloseTo(0.05);
	f.motion.update([quad(0.1)]);
	expect(f.latest[0].fill[3]).toBeCloseTo(0.05);
	f.at(240);
	expect(f.latest[0].fill[3]).toBeCloseTo(0.075);
	f.at(300);
	expect(f.frames.size).toBe(0);
	f.motion.destroy();
});

test('removed primitives withdraw immediately; new handles retain hits throughout fade-in; reduced motion settles and disposal cancels', () => {
	const f = fixture();
	f.motion.update([quad()]);
	const handle: OverlayPrimitive = {
		...quad(0.8),
		hit: 'corner',
		shape: { kind: 'Circle', center: [0.5, 0.5], radius: 6 }
	};
	f.motion.update([quad(), handle]);
	expect(f.latest[1].fill[3]).toBe(0);
	expect(f.latest[1].hit).toBe('corner');
	f.at(30);
	expect(f.latest[1].fill[3]).toBeGreaterThan(0);
	f.motion.update([quad()]);
	expect(f.latest).toHaveLength(1);
	expect(f.frames.size).toBe(0);
	f.motion.update([quad(0.1)]);
	f.preference.matches = true;
	f.preference.dispatchEvent(new Event('change'));
	expect(f.latest[0].fill[3]).toBe(0.1);
	expect(f.frames.size).toBe(0);
	f.preference.matches = false;
	f.preference.dispatchEvent(new Event('change'));
	f.motion.update([quad()]);
	expect(f.frames.size).toBe(1);
	f.motion.destroy();
	expect(f.frames.size).toBe(0);
	const latest = f.latest;
	f.at(300);
	f.motion.update([handle]);
	expect(f.latest).toBe(latest);
});
