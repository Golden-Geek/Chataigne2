import type { NodeId, UiEventDto } from '../../types';

export const TRIGGER_PARAM_EVENT_TOPIC = '__param.trigger';

export interface WorkbenchCustomEventStore {
	getCustomEventSequence(topic: string, origin?: NodeId | null): number;
	getCustomEventPayload<T = unknown>(topic: string, origin?: NodeId | null): T | null;
	applyBatchEvents(events: UiEventDto[]): void;
	reset(): void;
}

const customEventKey = (topic: string, origin: NodeId | null | undefined): string =>
	`${topic}\u0000${origin ?? 'global'}`;

export const createWorkbenchCustomEventStore = (): WorkbenchCustomEventStore => {
	let sequencesByKey = $state<Record<string, number>>({});
	let payloadsByKey = $state<Record<string, unknown>>({});

	const getCustomEventSequence = (topic: string, origin?: NodeId | null): number => {
		return sequencesByKey[customEventKey(topic, origin)] ?? 0;
	};

	const getCustomEventPayload = <T = unknown>(topic: string, origin?: NodeId | null): T | null => {
		const key = customEventKey(topic, origin);
		return Object.prototype.hasOwnProperty.call(payloadsByKey, key)
			? (payloadsByKey[key] as T)
			: null;
	};

	const applyBatchEvents = (events: UiEventDto[]): void => {
		let nextSequences: Record<string, number> | null = null;
		let nextPayloads: Record<string, unknown> | null = null;

		for (const event of events) {
			if (event.kind.kind === 'custom') {
				const key = customEventKey(event.kind.topic, event.kind.origin ?? null);
				nextSequences ??= { ...sequencesByKey };
				nextSequences[key] = (nextSequences[key] ?? 0) + 1;
				nextPayloads ??= { ...payloadsByKey };
				nextPayloads[key] = event.kind.payload;
				continue;
			}

			if (event.kind.kind === 'paramChanged' && event.kind.new_value.kind === 'trigger') {
				const key = customEventKey(TRIGGER_PARAM_EVENT_TOPIC, event.kind.param);
				nextSequences ??= { ...sequencesByKey };
				nextSequences[key] = (nextSequences[key] ?? 0) + 1;
			}
		}

		if (nextSequences !== null) {
			sequencesByKey = nextSequences;
		}
		if (nextPayloads !== null) {
			payloadsByKey = nextPayloads;
		}
	};

	const reset = (): void => {
		sequencesByKey = {};
		payloadsByKey = {};
	};

	return {
		getCustomEventSequence,
		getCustomEventPayload,
		applyBatchEvents,
		reset
	};
};
