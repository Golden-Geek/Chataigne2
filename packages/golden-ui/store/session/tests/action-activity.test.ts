import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UiAck } from '../../../types';
import {
	createWorkbenchUserActionActivity,
	createWorkbenchUserActionCoordinator
} from '../action-activity.svelte';
import type { WorkbenchIntentProgressObserver } from '../intent-pipeline';
import { PendingFeedback, type PendingFeedbackView } from '../pending-feedback';

const drain = async () => {
	for (let i = 0; i < 12; i++) await Promise.resolve();
};
const ack: UiAck = { success: true, status: 'applied' };

describe('action activity and feedback lifetime', () => {
	let coordinator: ReturnType<typeof createWorkbenchUserActionCoordinator>;
	let feedback: PendingFeedback;
	let view: PendingFeedbackView;
	const sync = () =>
		feedback.update({
			owner: coordinator,
			enabled: true,
			count: coordinator.pendingCount,
			oldestQueuedAtMs: coordinator.oldestQueuedAtMs,
			label: coordinator.currentLabel,
			phase: coordinator.currentPhase
		});
	function operation(label = 'Applying changes') {
		let progress!: WorkbenchIntentProgressObserver;
		let resolve!: () => void;
		let reject!: (error: Error) => void;
		const promise = coordinator.run(label, (observer) => {
			progress = observer;
			return new Promise<void>((yes, no) => {
				resolve = yes;
				reject = no;
			});
		});
		return {
			promise,
			progress,
			apply(acknowledgements: UiAck[] = [ack]) {
				progress.lifecycle?.('applied');
				progress.applied?.(acknowledgements);
				resolve();
			},
			reject
		};
	}
	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(0);
		coordinator = createWorkbenchUserActionCoordinator({
			nowMs: Date.now,
			afterStateUpdate: async () => {}
		});
		view = { visible: false, label: '', phase: '', phaseLabel: '', count: 0 };
		feedback = new PendingFeedback((next) => (view = next), Date.now);
	});
	afterEach(() => {
		coordinator.reset();
		feedback.dispose();
		vi.unstubAllGlobals();
		vi.useRealTimers();
	});
	it('keeps the oldest operation age while prioritizing a newer failure', () => {
		const activity = createWorkbenchUserActionActivity(Date.now);
		const older = activity.begin('Older work');
		vi.advanceTimersByTime(50);
		const failed = activity.begin('Failed work');
		failed.lifecycle('rejected');
		expect(activity.pendingCount).toBe(2);
		expect(activity.oldestQueuedAtMs).toBe(0);
		expect(activity.currentLabel).toBe('Failed work');
		expect(activity.currentPhase).toBe('rejected');
		failed.finish();
		expect(activity.currentLabel).toBe('Older work');
		older.finish();
		expect(activity.pendingCount).toBe(0);
	});
	it('keeps a sustained stream of promptly acknowledged operations quiet', async () => {
		for (let i = 0; i < 100; i++) {
			const current = operation();
			sync();
			vi.advanceTimersByTime(20);
			current.apply();
			await current.promise;
			await drain();
			sync();
			expect(coordinator.pendingCount).toBe(0);
			expect(view.visible).toBe(false);
		}
		expect(vi.getTimerCount()).toBe(0);
	});
	it('waits for an actual applied event boundary before retiring slow work', async () => {
		const current = operation();
		current.progress.lifecycle?.('received');
		sync();
		vi.advanceTimersByTime(299);
		expect(view.visible).toBe(false);
		vi.advanceTimersByTime(1);
		expect(view).toMatchObject({ visible: true, phase: 'received' });
		current.apply([{ ...ack, latest_event_time: { tick: 20, micro: 0, seq: 0 } }]);
		await current.promise;
		sync();
		expect(view.phaseLabel).toBe('Finishing');
		coordinator.observeAppliedEventTime({ tick: 19, micro: 0, seq: 0 });
		await drain();
		expect(coordinator.pendingCount).toBe(1);
		coordinator.observeAppliedEventTime({ tick: 20, micro: 0, seq: 0 });
		await drain();
		sync();
		expect(view).toMatchObject({ visible: true, phaseLabel: 'Done', count: 0 });
		vi.advanceTimersByTime(350);
		expect(view.visible).toBe(false);
		expect(vi.getTimerCount()).toBe(0);
	});
	it('retains a newer failure while older work remains outstanding, then resumes its progress', async () => {
		const older = operation('Older work');
		const failed = operation('Failed work');
		const error = new Error('rejected by engine');
		const rejected = expect(failed.promise).rejects.toBe(error);
		failed.reject(error);
		await rejected;
		sync();
		expect(view).toMatchObject({
			visible: true,
			label: 'Failed work',
			phaseLabel: 'Failed',
			count: 2
		});
		await vi.advanceTimersByTimeAsync(1199);
		sync();
		expect(view.phaseLabel).toBe('Failed');
		await vi.advanceTimersByTimeAsync(1);
		sync();
		expect(view).toMatchObject({
			visible: true,
			label: 'Older work',
			phaseLabel: 'Queued',
			count: 1
		});
		older.apply();
		await older.promise;
		await drain();
		sync();
		vi.advanceTimersByTime(150);
		expect(view.visible).toBe(false);
	});
	it('cancels an old event-boundary wait without retiring new-session work', async () => {
		const old = operation();
		old.apply([{ ...ack, latest_event_time: { tick: 20, micro: 0, seq: 0 } }]);
		await old.promise;
		expect(vi.getTimerCount()).toBe(1);
		coordinator.reset();
		expect(vi.getTimerCount()).toBe(0);
		const next = operation('New session');
		await drain();
		expect(coordinator.pendingCount).toBe(1);
		expect(coordinator.currentLabel).toBe('New session');
		next.apply();
		await next.promise;
		await drain();
		expect(coordinator.pendingCount).toBe(0);
	});
	it.each(['applied', 'rejected'] as const)(
		'cancels paint and retention timers on reset after %s',
		async (phase) => {
			let nextFrame = 0;
			const frames = new Map<number, FrameRequestCallback>();
			vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
				frames.set(++nextFrame, callback);
				return nextFrame;
			});
			vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id));
			const current = operation();
			if (phase === 'applied') {
				current.apply();
				await current.promise;
			} else {
				const rejected = expect(current.promise).rejects.toThrow('failed');
				current.reject(new Error('failed'));
				await rejected;
			}
			await drain();
			expect(frames.size).toBe(1);
			expect(vi.getTimerCount()).toBeGreaterThan(0);
			coordinator.reset();
			await drain();
			expect(coordinator.pendingCount).toBe(0);
			expect(frames.size).toBe(0);
			expect(vi.getTimerCount()).toBe(0);
		}
	);
	it('does not recreate timers when a cancelled operation rejects after reset', async () => {
		const old = operation();
		coordinator.reset();
		const next = operation('New session');
		const rejected = expect(old.promise).rejects.toThrow('connection replaced');
		old.reject(new Error('connection replaced'));
		await rejected;
		await drain();
		expect(coordinator.currentLabel).toBe('New session');
		expect(vi.getTimerCount()).toBe(0);
		next.apply();
		await next.promise;
	});
});
