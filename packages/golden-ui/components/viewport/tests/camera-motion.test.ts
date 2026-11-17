import { describe, expect, it } from 'vitest';
import { createCameraMotion } from '../camera-motion';
import type { ViewCamera } from '../geometry';

function fixture() {
	let camera: ViewCamera = { zoom: 1, pan: { x: 40, y: -20 } };
	let now = 0,
		serial = 0;
	const frames = new Map<number, (time: number) => void>();
	const motion = createCameraMotion(
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
describe('frame-driven document camera', () => {
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
	it('eases Frame/Home in and out, completing an exact target without restarting held shortcuts', () => {
		const f = fixture();
		const target = { zoom: 4, pan: { x: -80, y: 100 } };
		f.motion.move(target);
		expect(f.camera()).toEqual({ zoom: 1, pan: { x: 40, y: -20 } });
		f.advance(40);
		expect(f.camera().zoom).toBe(1 + 3 * 0.15625);
		f.motion.move(target);
		f.advance(80);
		expect(f.camera()).toEqual({ zoom: 2.5, pan: { x: -20, y: 40 } });
		f.advance(120);
		expect(f.camera().zoom).toBe(1 + 3 * 0.84375);
		f.advance(160);
		expect(f.camera()).toEqual(target);
		expect(f.frames.size).toBe(0);
	});
	it('retargets navigation from its displayed position and lets pointer zoom interrupt it', () => {
		const f = fixture();
		f.motion.move({ zoom: 4, pan: { x: -80, y: 100 } });
		f.advance(80);
		const displayed = f.camera();
		f.motion.move({ zoom: 1, pan: { x: 0, y: 0 } });
		expect(f.camera()).toEqual(displayed);
		f.advance(120);
		const interrupted = f.camera();
		f.motion.zoom(2, { x: 140, y: 80 });
		expect(f.camera()).toEqual(interrupted);
		f.advance(280);
		expect(f.camera().zoom).toBe(interrupted.zoom * 2);
		expect(f.frames.size).toBe(0);
	});
	it('freezes navigation on editing, snaps reduced motion and cannot revive after disposal', () => {
		const f = fixture();
		const target = { zoom: 4, pan: { x: -80, y: 100 } };
		f.motion.move(target);
		f.advance(40);
		const displayed = f.camera();
		f.motion.cancel();
		f.advance(160);
		expect(f.camera()).toEqual(displayed);
		f.motion.move(target, true);
		expect(f.camera()).toEqual(target);
		expect(f.frames.size).toBe(0);
		f.motion.dispose();
		f.motion.move({ zoom: 1, pan: { x: 0, y: 0 } });
		expect(f.frames.size).toBe(0);
	});
});
