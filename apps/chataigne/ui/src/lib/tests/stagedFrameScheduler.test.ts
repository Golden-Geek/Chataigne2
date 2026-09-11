import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createStagedFrameBatchScheduler } from '../../../../../../packages/golden-ui/transport/staged-frame-batches';
import type {
	EventTime,
	UiEventBatch,
	UiEventDto
} from '../../../../../../packages/golden-ui/types';

const time = (tick: number): EventTime => ({ tick, micro: 0, seq: 0 });

const event: UiEventDto = {
	time: time(1),
	kind: {
		kind: 'custom',
		topic: 'test.timed_projection',
		payload: null,
		retention: 'replay'
	}
};

describe('staged frame scheduler time budget', () => {
	let frames: FrameRequestCallback[];

	beforeEach(() => {
		frames = [];
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
			frames.push(callback);
			return frames.length;
		});
	});

	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it('checks elapsed time between detached work units without publishing partial work', () => {
		const applied: UiEventBatch[] = [];
		const requestedWork: number[] = [];
		const frameDurations: number[] = [];
		let cursor = time(0);
		let clockMs = 0;
		let completedWork = 0;
		const scheduler = createStagedFrameBatchScheduler({
			onBatch: (batch) => applied.push(batch),
			onOrderingViolation: () => {
				throw new Error('unexpected ordering violation');
			},
			createEventWork: () => ({
				advance: (maxWork) => {
					requestedWork.push(maxWork);
					const workUsed = Math.min(maxWork, 10 - completedWork);
					completedWork += workUsed;
					clockMs += workUsed;
					return { workUsed, done: completedWork === 10 };
				}
			}),
			estimateEventCost: () => ({ work: 101, estimatedBytes: 1 }),
			limits: { maxWorkPerFrame: 100, maxFrameTimeMs: 3 },
			now: () => clockMs,
			isClosed: () => false,
			getCursor: () => cursor,
			setCursor: (nextCursor) => {
				cursor = nextCursor;
			}
		});

		scheduler.stage({ from: time(0), to: time(1), events: [event] });
		while (frames.length > 0) {
			const frame = frames.shift();
			if (!frame) {
				throw new Error('scheduled frame disappeared');
			}
			const startedAt = clockMs;
			frame(0);
			frameDurations.push(clockMs - startedAt);
			if (frames.length > 0) {
				expect(applied).toHaveLength(0);
				expect(cursor).toEqual(time(0));
			}
		}

		expect(frameDurations).toEqual([3, 3, 3, 1]);
		expect(requestedWork).toEqual(Array.from({ length: 10 }, () => 1));
		expect(applied).toHaveLength(1);
		expect(applied[0]?.events).toEqual([event]);
		expect(cursor).toEqual(time(1));
	});
});
