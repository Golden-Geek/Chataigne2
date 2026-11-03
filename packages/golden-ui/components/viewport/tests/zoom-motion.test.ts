import { describe, expect, it } from 'vitest';
import { createZoomMotion } from '../zoom-motion';
import type { ViewCamera } from '../geometry';

function fixture() {
	let camera: ViewCamera = { zoom: 1, pan: { x: 40, y: -20 } };
	let now = 0,
		serial = 0;
	const frames = new Map<number, (time: number) => void>();
	const motion = createZoomMotion(
		() => camera,
		(value) => (camera = value),
		{
			now: () => now,
			request: (callback) => {
				frames.set(++serial, callback);
				return serial;
			},
			cancel: (id) => {
				frames.delete(id);
			}
		}
	);
	return {
		motion,
		frames,
		camera: () => camera,
		advance(time: number) {
			now = time;
			const pending = [...frames.values()];
			frames.clear();
			for (const callback of pending) callback(now);
		}
	};
}
describe('frame-driven document zoom', () => {
	it('animates through intermediate cameras while preserving the pointer world point', () => {
		const f = fixture();
		f.motion.zoom(2, { x: 140, y: 80 });
		expect(f.camera().zoom).toBe(1);
		for (const time of [16, 40, 80, 120]) {
			f.advance(time);
			const camera = f.camera();
			expect(camera.zoom).toBeGreaterThan(1);
			expect(camera.zoom).toBeLessThan(2);
			expect(camera.pan.x + 100 * camera.zoom).toBeCloseTo(140);
			expect(camera.pan.y + 100 * camera.zoom).toBeCloseTo(80);
			expect(f.frames.size).toBe(1);
		}
		f.advance(160);
		expect(f.camera()).toEqual({ zoom: 2, pan: { x: -60, y: -120 } });
		expect(f.frames.size).toBe(0);
	});
	it('accumulates rapid targets and reanchors from the displayed camera without jumping', () => {
		const f = fixture();
		f.motion.zoom(2, { x: 140, y: 80 });
		f.advance(40);
		const displayed = f.camera(),
			anchor = { x: -50, y: 70 };
		const world = {
			x: (anchor.x - displayed.pan.x) / displayed.zoom,
			y: (anchor.y - displayed.pan.y) / displayed.zoom
		};
		f.motion.zoom(2, anchor);
		expect(f.camera()).toEqual(displayed);
		expect(f.frames.size).toBe(1);
		f.advance(80);
		expect(f.camera().pan.x + world.x * f.camera().zoom).toBeCloseTo(anchor.x);
		f.advance(200);
		expect(f.camera().zoom).toBe(4);
		expect(f.frames.size).toBe(0);
	});
	it('cancels obsolete frames on interaction and cannot restart after disposal', () => {
		const f = fixture();
		f.motion.zoom(2, { x: 0, y: 0 });
		f.advance(32);
		const held = f.camera();
		f.motion.cancel();
		f.advance(1000);
		expect(f.camera()).toEqual(held);
		expect(f.frames.size).toBe(0);
		f.motion.zoom(2, { x: 0, y: 0 });
		f.motion.dispose();
		f.advance(2000);
		f.motion.zoom(2, { x: 0, y: 0 });
		expect(f.camera()).toEqual(held);
		expect(f.frames.size).toBe(0);
	});
	it('snaps reduced-motion targets, bounds zoom and ignores invalid wheel input', () => {
		const f = fixture();
		f.motion.zoom(2, { x: 0, y: 0 });
		f.motion.zoom(2, { x: 0, y: 0 }, true);
		expect(f.camera().zoom).toBe(4);
		expect(f.frames.size).toBe(0);
		f.motion.zoom(1e6, { x: 0, y: 0 }, true);
		expect(f.camera().zoom).toBe(64);
		f.motion.zoom(1e-9, { x: 0, y: 0 }, true);
		expect(f.camera().zoom).toBe(0.05);
		for (const factor of [0, -1, NaN, Infinity]) f.motion.zoom(factor, { x: 0, y: 0 });
		expect(f.frames.size).toBe(0);
		expect(f.camera().zoom).toBe(0.05);
	});
});
