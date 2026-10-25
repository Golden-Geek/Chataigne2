import { createWorkbenchLoggerStore } from '../../../store/session/logger.svelte';
import type { UiEventDto, UiLogRecord } from '../../../types';

const record = (id: number, message: string): UiLogRecord => ({
	id,
	timestamp_ms: id,
	level: 'info',
	tag: 'fixture',
	message
});
const event = (record: UiLogRecord): UiEventDto => ({
	time: { tick: 1, micro: 0, seq: 0 },
	kind: {
		kind: 'custom',
		retention: 'replay',
		topic: '__logger.record',
		payload: {
			id: record.id,
			timestamp_ms: record.timestamp_ms,
			level: record.level,
			tag: record.tag,
			message: record.message
		}
	}
});
const require = (condition: boolean, message: string) => {
	if (!condition) throw new Error(message);
};
const until = async (condition: () => boolean) => {
	const started = performance.now();
	while (!condition()) {
		if (performance.now() - started > 2000) throw new Error('Logger publication timed out.');
		await new Promise(requestAnimationFrame);
	}
};

export async function qualifyLoggerStore() {
	const store = createWorkbenchLoggerStore();
	try {
		store.applySnapshotLogger({ max_entries: 4, records: [record(1, 'One')] });
		const initial = store.logRecords;
		store.partitionBatchEvents([event(record(2, 'Two'))]);
		await until(() => store.logRecords.length === 2);
		require(initial.length === 1 &&
			store.logRecords !== initial, 'Backend append mutated a published history.');
		const beforeReplace = store.logRecords;
		store.partitionBatchEvents([event(record(1, 'Changed')), event(record(3, 'Three'))]);
		await until(() => store.logRecords.length === 3);
		require(beforeReplace[0].message === 'One' &&
			beforeReplace.length === 2 &&
			store.logRecords[0].message ===
				'Changed', 'Backend replacement mutated a published history.');
		const beforeClear = store.logRecords;
		store.partitionBatchEvents([
			{
				time: { tick: 2, micro: 0, seq: 0 },
				kind: { kind: 'custom', topic: '__logger.cleared', payload: null, retention: 'replay' }
			},
			event(record(4, 'After clear'))
		]);
		await until(() => store.logRecords.length === 1 && store.logRecords[0].id === 4);
		require(beforeClear.length === 3, 'Clear mutated a retained history reference.');
		store.reset();
		store.applySnapshotLogger({ max_entries: 2, records: [] });
		store.appendUiLogRecord('info', 'fixture', 'A');
		store.appendUiLogRecord('info', 'fixture', 'B');
		const beforeTrim = store.logRecords;
		store.appendUiLogRecord('info', 'fixture', 'C');
		require(beforeTrim[0].message === 'A' &&
			store.logRecords[0].message === 'B', 'Trim changed the prior snapshot.');
		const beforeRepeat = store.logRecords;
		store.appendUiLogRecord('info', 'fixture', 'C');
		require(beforeRepeat.at(-1)!.repeat_count === undefined &&
			store.logRecords.at(-1)!.repeat_count === 2 &&
			store.logRecords !== beforeRepeat, 'UI repeat replacement mutated a published record.');
		return { backendAppend: true, backendReplace: true, clear: true, trim: true, repeat: true };
	} finally {
		store.reset();
	}
}
