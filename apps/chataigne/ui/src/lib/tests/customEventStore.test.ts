import { describe, expect, it } from 'vitest';
import {
	createWorkbenchCustomEventStore,
	TRIGGER_PARAM_EVENT_TOPIC
} from '../../../../../../packages/golden-ui/store/session/custom-events.svelte';
import type { UiEventDto } from '../../../../../../packages/golden-ui/types';

const eventTime = (seq: number) => ({ tick: 1, micro: 0, seq });

const customEvent = (
	seq: number,
	topic: string,
	payload: unknown,
	origin?: number
): UiEventDto => ({
	time: eventTime(seq),
	kind: {
		kind: 'custom',
		topic,
		origin,
		payload,
		retention: 'transient'
	}
});

describe('workbench custom event store', () => {
	it('tracks each custom-event key and keeps large payloads raw', () => {
		const store = createWorkbenchCustomEventStore();
		const firstPayload = { value: 1 };

		expect(store.getCustomEventSequence('preview', 10)).toBe(0);
		expect(store.getCustomEventPayload('preview', 10)).toBeNull();

		store.applyBatchEvents([customEvent(1, 'preview', firstPayload, 10)]);
		expect(store.getCustomEventSequence('preview', 10)).toBe(1);
		expect(store.getCustomEventPayload('preview', 10)).toBe(firstPayload);

		store.applyBatchEvents([customEvent(2, 'unrelated', { value: 2 }, 10)]);
		expect(store.getCustomEventSequence('preview', 10)).toBe(1);
		expect(store.getCustomEventPayload('preview', 10)).toBe(firstPayload);

		store.applyBatchEvents([
			customEvent(3, 'preview', { value: 3 }, 10),
			customEvent(4, 'preview', { value: 4 }, 10)
		]);
		expect(store.getCustomEventSequence('preview', 10)).toBe(3);
		expect(store.getCustomEventPayload('preview', 10)).toEqual({ value: 4 });

		store.applyBatchEvents([customEvent(5, 'preview', { value: 5 }, 11)]);
		expect(store.getCustomEventSequence('preview', 10)).toBe(3);
		expect(store.getCustomEventPayload('preview', 10)).toEqual({ value: 4 });
		expect(store.getCustomEventSequence('preview', 11)).toBe(1);
		expect(store.getCustomEventPayload('preview', 11)).toEqual({ value: 5 });
	});

	it('tracks trigger parameter events independently and resets subscribers', () => {
		const store = createWorkbenchCustomEventStore();
		expect(store.getCustomEventSequence(TRIGGER_PARAM_EVENT_TOPIC, 42)).toBe(0);

		store.applyBatchEvents([
			{
				time: eventTime(1),
				kind: {
					kind: 'paramChanged',
					param: 42,
					old_value: { kind: 'trigger' },
					new_value: { kind: 'trigger' }
				}
			}
		]);
		expect(store.getCustomEventSequence(TRIGGER_PARAM_EVENT_TOPIC, 42)).toBe(1);
		expect(store.getCustomEventPayload(TRIGGER_PARAM_EVENT_TOPIC, 42)).toBeNull();

		store.reset();
		expect(store.getCustomEventSequence(TRIGGER_PARAM_EVENT_TOPIC, 42)).toBe(0);
		expect(store.getCustomEventPayload(TRIGGER_PARAM_EVENT_TOPIC, 42)).toBeNull();
	});
});
