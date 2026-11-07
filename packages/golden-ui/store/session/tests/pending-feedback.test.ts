import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	PendingFeedback,
	type PendingFeedbackInput,
	type PendingFeedbackView
} from '../pending-feedback';

describe('outstanding-operation feedback', () => {
	let state: PendingFeedbackView;
	let input: PendingFeedbackInput;
	let feedback: PendingFeedback;
	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(0);
		state = { visible: false, label: '', phase: '', phaseLabel: '', count: 0 };
		input = {
			owner: {},
			enabled: true,
			count: 1,
			oldestQueuedAtMs: 0,
			label: 'Applying changes',
			phase: 'queued'
		};
		feedback = new PendingFeedback((next) => {
			state = next;
		}, Date.now);
	});
	afterEach(() => {
		feedback.dispose();
		vi.useRealTimers();
	});
	it('ignores a long continuously occupied stream whose oldest operation always finishes quickly', () => {
		for (let i = 0; i < 400; i++) {
			feedback.update({ ...input, count: 3, oldestQueuedAtMs: Date.now() - 30 });
			vi.advanceTimersByTime(10);
			expect(state.visible).toBe(false);
		}
		feedback.update({ ...input, count: 0, oldestQueuedAtMs: null });
		vi.advanceTimersByTime(1000);
		expect(state.visible).toBe(false);
	});
	it('uses actual operation age, not phase/count churn or component mount time', () => {
		vi.advanceTimersByTime(200);
		feedback.update(input);
		vi.advanceTimersByTime(50);
		feedback.update({ ...input, phase: 'received', count: 2 });
		vi.advanceTimersByTime(49);
		expect(state.visible).toBe(false);
		vi.advanceTimersByTime(1);
		expect(state).toMatchObject({ visible: true, count: 2, phase: 'received' });
	});
	it('coalesces overlapping slow operations without restarting the visible lifetime', () => {
		feedback.update(input);
		vi.advanceTimersByTime(300);
		expect(state.visible).toBe(true);
		feedback.update({ ...input, oldestQueuedAtMs: 200, label: 'Second operation' });
		vi.advanceTimersByTime(200);
		expect(state).toMatchObject({ visible: true, label: 'Second operation' });
		feedback.update({ ...input, count: 0, oldestQueuedAtMs: null });
		expect(state).toMatchObject({
			visible: true,
			label: 'Changes applied',
			phaseLabel: 'Done',
			count: 0
		});
		vi.advanceTimersByTime(150);
		expect(state.visible).toBe(false);
	});
	it('never flashes a newly shown short tail, but finishes with a truthful completed caption', () => {
		feedback.update(input);
		vi.advanceTimersByTime(300);
		feedback.update({ ...input, count: 0, oldestQueuedAtMs: null });
		vi.advanceTimersByTime(349);
		expect(state).toMatchObject({ visible: true, phaseLabel: 'Done' });
		vi.advanceTimersByTime(1);
		expect(state.visible).toBe(false);
	});
	it('fresh fast work cannot perpetually extend a previous slow operation notice', () => {
		feedback.update(input);
		vi.advanceTimersByTime(300);
		for (let i = 0; i < 40; i++) {
			feedback.update({ ...input, oldestQueuedAtMs: Date.now() });
			vi.advanceTimersByTime(10);
		}
		expect(state.visible).toBe(false);
	});
	it('a young overlapping successor does not flicker after a long operation finishes', () => {
		feedback.update(input);
		vi.advanceTimersByTime(1000);
		feedback.update({ ...input, oldestQueuedAtMs: 990, label: 'Successor' });
		vi.advanceTimersByTime(289);
		expect(state.visible).toBe(true);
		vi.advanceTimersByTime(1);
		expect(state).toMatchObject({ visible: true, label: 'Successor' });
	});
	it('cancellation before the delay retires the timer without ever showing progress', () => {
		feedback.update(input);
		vi.advanceTimersByTime(299);
		feedback.update({ ...input, count: 0, oldestQueuedAtMs: null });
		vi.advanceTimersByTime(1000);
		expect(state.visible).toBe(false);
	});
	it('retained failure and overlapping grace never advertise retired actions as pending', () => {
		feedback.update({ ...input, count: 3, phase: 'rejected' });
		feedback.update({ ...input, count: 0, oldestQueuedAtMs: null });
		expect(state).toMatchObject({ visible: true, phaseLabel: 'Failed', count: 0 });
		feedback.update({ ...input, count: 2, oldestQueuedAtMs: Date.now() });
		expect(state.count).toBe(2);
	});
	it('retains a fast rejection and clears abandoned owners/connection timers', () => {
		feedback.update({ ...input, phase: 'rejected' });
		expect(state).toMatchObject({ visible: true, phaseLabel: 'Failed' });
		feedback.update({ ...input, enabled: false });
		expect(state.visible).toBe(false);
		vi.advanceTimersByTime(1000);
		expect(state.visible).toBe(false);
		feedback.update({ ...input, owner: {}, oldestQueuedAtMs: Date.now() });
		vi.advanceTimersByTime(299);
		expect(state.visible).toBe(false);
		feedback.dispose();
		vi.advanceTimersByTime(1000);
		expect(state.visible).toBe(false);
	});
});
