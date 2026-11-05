import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RenderOpportunity } from '../render-opportunity';

describe('native layout rendering opportunity', () => {
	let callbacks: Map<number, FrameRequestCallback>;
	let next: number;
	beforeEach(() => {
		vi.useFakeTimers();
		callbacks = new Map();
		next = 0;
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
			callbacks.set(++next, callback);
			return next;
		});
		vi.stubGlobal('cancelAnimationFrame', (id: number) => callbacks.delete(id));
	});
	afterEach(() => {
		vi.unstubAllGlobals();
		vi.useRealTimers();
	});
	it('resolves in a task after the animation callback, rather than its microtask checkpoint', async () => {
		const gate = new RenderOpportunity();
		const done = vi.fn();
		const result = gate.wait().then(done);
		callbacks.get(1)!(0);
		await Promise.resolve();
		expect(done).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(0);
		await result;
		expect(done).toHaveBeenCalledWith(true);
		expect(vi.getTimerCount()).toBe(0);
	});
	it.each(['before frame', 'after frame'])(
		'cancels %s without leaving timers or a blocked pump',
		async (phase) => {
			const gate = new RenderOpportunity();
			const result = gate.wait();
			if (phase === 'after frame') callbacks.get(1)!(0);
			gate.cancel();
			expect(await result).toBe(false);
			expect(vi.getTimerCount()).toBe(0);
			await vi.advanceTimersByTimeAsync(2000);
		}
	);
	it('bounds a suppressed rendering opportunity and cancels its frame callback', async () => {
		const gate = new RenderOpportunity();
		const result = gate.wait();
		await vi.advanceTimersByTimeAsync(1500);
		expect(await result).toBe(false);
		expect(callbacks.size).toBe(0);
		expect(vi.getTimerCount()).toBe(0);
	});
});
