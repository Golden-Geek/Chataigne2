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

interface CustomEventEntry {
	readonly sequence: number;
	readonly payload: unknown;
	recordCustom(payload: unknown): void;
	recordTrigger(): void;
}

const createCustomEventEntry = (): CustomEventEntry => {
	let sequence = $state(0);
	let payload = $state.raw<unknown>(null);

	return {
		get sequence(): number {
			return sequence;
		},
		get payload(): unknown {
			return payload;
		},
		recordCustom(nextPayload): void {
			payload = nextPayload;
			sequence += 1;
		},
		recordTrigger(): void {
			sequence += 1;
		}
	};
};

export const createWorkbenchCustomEventStore = (): WorkbenchCustomEventStore => {
	const entriesByKey = new Map<string, CustomEventEntry>();
	let entryGeneration = $state(0);
	let resetGeneration = $state(0);

	const existingEntryForKey = (key: string): CustomEventEntry | null => {
		// Present-entry readers must also rerun after reset so they drop the detached entry.
		void resetGeneration;
		const entry = entriesByKey.get(key);
		if (entry) {
			return entry;
		}
		void entryGeneration;
		return null;
	};

	const entryForWrite = (key: string): CustomEventEntry => {
		let entry = entriesByKey.get(key);
		if (!entry) {
			entry = createCustomEventEntry();
			entriesByKey.set(key, entry);
			entryGeneration += 1;
		}
		return entry;
	};

	const getCustomEventSequence = (topic: string, origin?: NodeId | null): number => {
		return existingEntryForKey(customEventKey(topic, origin))?.sequence ?? 0;
	};

	const getCustomEventPayload = <T = unknown>(topic: string, origin?: NodeId | null): T | null => {
		return (existingEntryForKey(customEventKey(topic, origin))?.payload as T | undefined) ?? null;
	};

	const applyBatchEvents = (events: UiEventDto[]): void => {
		for (const event of events) {
			if (event.kind.kind === 'custom') {
				const key = customEventKey(event.kind.topic, event.kind.origin ?? null);
				entryForWrite(key).recordCustom(event.kind.payload);
				continue;
			}

			if (event.kind.kind === 'paramChanged' && event.kind.new_value.kind === 'trigger') {
				const key = customEventKey(TRIGGER_PARAM_EVENT_TOPIC, event.kind.param);
				entryForWrite(key).recordTrigger();
			}
		}
	};

	const reset = (): void => {
		entriesByKey.clear();
		resetGeneration += 1;
	};

	return {
		getCustomEventSequence,
		getCustomEventPayload,
		applyBatchEvents,
		reset
	};
};
