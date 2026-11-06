import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createPreviewSession, type PreviewGeometry, type PreviewSession } from '../session';
import type { PreviewRequest } from '../generated/PreviewRequest';
import type { OverlayPrimitive } from '../generated/OverlayPrimitive';

const overlay: OverlayPrimitive = {
	shape: { kind: 'Circle', center: [0.2, 0.2], radius: 6 },
	hit: 'corner',
	fill: [1, 1, 1, 1],
	stroke: [0, 0, 1, 1],
	width: 1,
	dash: null
};

describe('stationary native editor backdrop', () => {
	const sessions: PreviewSession[] = [];
	beforeEach(() => vi.useFakeTimers());
	afterEach(async () => {
		for (const session of sessions.splice(0)) void session.dispose();
		await vi.advanceTimersByTimeAsync(0);
		vi.useRealTimers();
		vi.unstubAllGlobals();
	});

	function fixture(paint: () => 'element' | 'native-backdrop' = () => 'native-backdrop') {
		const requests: PreviewRequest[] = [];
		const session = createPreviewSession({
			available: () => true,
			async request(request) {
				requests.push(request);
				if (request.kind === 'Connect')
					return {
						kind: 'Connected',
						session: 'owned-session',
						version: request.version,
						max_views: 64
					};
				if (request.kind === 'Layout' || request.kind === 'Status')
					return {
						kind: 'Applied',
						revision: request.revision,
						visible_views: 1,
						installed: true
					};
				return { kind: 'Disconnected' };
			}
		});
		sessions.push(session);
		const geometry: PreviewGeometry = {
			rect: { x: 100, y: 100, width: 400, height: 300 },
			clip: { x: 10, y: 10, width: 800, height: 600 },
			visible: true,
			overlays: [overlay],
			overlay_capacity: 2
		};
		const view = session.register('screen', () => geometry, { paint });
		return { session, view, geometry, layouts: () => requests.filter((r) => r.kind === 'Layout') };
	}

	function captureFrames() {
		const frames: FrameRequestCallback[] = [];
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) =>
			frames.push(callback)
		);
		vi.stubGlobal('cancelAnimationFrame', () => {});
		return frames;
	}

	it('dispatches repeated image pan/zoom in the measurement frame through the fixed canvas', async () => {
		const { session, view, geometry, layouts } = fixture();
		await session.connect();
		await vi.advanceTimersByTimeAsync(0);
		const frames = captureFrames();
		for (let index = 0; index < 5; index++) {
			geometry.rect = { x: 80 - index * 30, y: 90, width: 450 + index * 50, height: 350 };
			view.invalidate();
			expect(frames).toHaveLength(1);
			frames.shift()!(index);
			await vi.advanceTimersByTimeAsync(0);
			expect(frames).toHaveLength(0);
			expect(layouts().at(-1)?.views[0].rect).toEqual(geometry.rect);
			expect(session.state.installed).toBe(true);
		}
		expect(layouts()).toHaveLength(6);
		// The option is local DOM metadata, never a new handwritten wire field.
		expect(layouts().at(-1)?.views[0]).not.toHaveProperty('paint');
	});

	it('waits for DOM paint when ownership first changes to the native canvas', async () => {
		let native = false;
		const { session, view, geometry, layouts } = fixture(() =>
			native ? 'native-backdrop' : 'element'
		);
		await session.connect();
		await vi.advanceTimersByTimeAsync(0);
		const frames = captureFrames();
		native = true;
		geometry.rect = { ...geometry.rect, x: 90 };
		view.invalidate();
		frames.shift()!(0);
		await vi.advanceTimersByTimeAsync(0);
		expect(layouts()).toHaveLength(1);
		expect(frames).toHaveLength(1);
		frames.shift()!(1);
		await vi.advanceTimersByTimeAsync(0);
		expect(layouts()).toHaveLength(2);
		geometry.rect = { ...geometry.rect, x: 80 };
		view.invalidate();
		frames.shift()!(2);
		await vi.advanceTimersByTimeAsync(0);
		expect(layouts()).toHaveLength(3);
		expect(frames).toHaveLength(0);
	});

	it('fences ownership-only transitions even when the wire geometry stays identical', async () => {
		let native = true;
		const { session, view, layouts } = fixture(() => (native ? 'native-backdrop' : 'element'));
		await session.connect();
		await vi.advanceTimersByTimeAsync(0);
		const frames = captureFrames();
		for (const next of [false, true]) {
			const count = layouts().length;
			native = next;
			view.invalidate();
			frames.shift()!(0);
			await vi.advanceTimersByTimeAsync(0);
			expect(layouts()).toHaveLength(count);
			expect(frames).toHaveLength(1);
			frames.shift()!(1);
			await vi.advanceTimersByTimeAsync(0);
			expect(layouts()).toHaveLength(count + 1);
		}
	});

	it.each(['clip', 'scale', 'source', 'presence', 'capacity', 'ownership'] as const)(
		'keeps the browser paint fence for %s changes during canvas movement',
		async (change) => {
			let native = true;
			const { session, view, geometry, layouts } = fixture(() =>
				native ? 'native-backdrop' : 'element'
			);
			await session.connect();
			await vi.advanceTimersByTimeAsync(0);
			const frames = captureFrames();
			geometry.rect = { ...geometry.rect, x: 90 };
			if (change === 'clip') geometry.clip = { ...geometry.clip, width: 700 };
			if (change === 'scale') vi.stubGlobal('devicePixelRatio', 1.5);
			if (change === 'source') view.update('replacement');
			if (change === 'presence') geometry.overlays = [];
			if (change === 'capacity') geometry.overlay_capacity = 3;
			if (change === 'ownership') native = false;
			view.invalidate();
			frames.shift()!(0);
			await vi.advanceTimersByTimeAsync(0);
			expect(layouts()).toHaveLength(1);
			expect(frames).toHaveLength(1);
			frames.shift()!(1);
			await vi.advanceTimersByTimeAsync(0);
			expect(layouts()).toHaveLength(2);
		}
	);

	it.each(['ordinary', 'tiles'] as const)(
		'does not weaken %s preview geometry fencing',
		async (kind) => {
			const { session, view, geometry, layouts } = fixture(() =>
				kind === 'ordinary' ? 'element' : 'native-backdrop'
			);
			if (kind === 'tiles') {
				geometry.overlays = [];
				geometry.overlay_capacity = 0;
				geometry.tile_groups = [
					{ source: 'child', tiles: [geometry.rect], capacity: 1, fit: 'Contain' }
				];
			}
			await session.connect();
			await vi.advanceTimersByTimeAsync(0);
			const frames = captureFrames();
			if (kind === 'tiles') {
				geometry.tile_groups![0].tiles = [{ ...geometry.rect, x: 50 }];
			} else geometry.rect = { ...geometry.rect, width: 500 };
			view.invalidate();
			frames.shift()!(0);
			await vi.advanceTimersByTimeAsync(0);
			expect(layouts()).toHaveLength(1);
			expect(frames).toHaveLength(1);
			frames.shift()!(1);
			await vi.advanceTimersByTimeAsync(0);
			expect(layouts()).toHaveLength(2);
		}
	);
});
