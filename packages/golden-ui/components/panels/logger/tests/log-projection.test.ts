import { describe, expect, it } from 'vitest';
import {
	LoggerProjection,
	filterLoggerEntries,
	formatLogEntryForClipboard,
	normalizeSearchText
} from '../log-projection';
import type { UiLogRecord } from '../../../../types';

const record = (
	id: number,
	message: string,
	repeatCount = 1,
	overrides: Partial<UiLogRecord> = {}
): UiLogRecord => ({
	id,
	timestamp_ms: id * 1000,
	level: 'info',
	tag: 'transport',
	message,
	repeat_count: repeatCount,
	origin: 42,
	...overrides
});

const originLabel = () => 'Fixture node';

describe('logger presentation projection', () => {
	it('windows recent collapsed records without merging separate source records', () => {
		const projection = new LoggerProjection();
		const records = [
			record(1, 'Ready', 2),
			record(2, 'Ready', 3),
			record(3, 'Stopped', 1, { level: 'warning' })
		];
		const result = projection.display(records, 2, true, false, originLabel);
		expect(result.entries.map((entry) => [entry.key, entry.repeatCount])).toEqual([
			['r:2', 3],
			['r:3', 1]
		]);
		expect(result.collapsedGroupCount).toBe(2);
	});

	it('merges matching signatures and orders groups by their latest record', () => {
		const projection = new LoggerProjection();
		const records = [
			record(1, 'Ready', 2),
			record(3, 'Stopped', 1, { level: 'warning' }),
			record(2, 'Ready', 3)
		];
		const result = projection.display(records, 2, true, true, originLabel);
		expect(result.entries.map((entry) => [entry.key, entry.record.id, entry.repeatCount])).toEqual([
			['g:3', 3, 1],
			['g:1', 2, 5]
		]);
		expect(result.collapsedGroupCount).toBe(2);
	});

	it('expands repeated records from the tail within a finite render limit', () => {
		const projection = new LoggerProjection();
		const records = [record(1, 'Old', 9), record(2, 'New', 3), record(3, 'Latest')];
		const result = projection.display(records, 4, false, false, originLabel);
		expect(result.entries.map((entry) => entry.key)).toEqual([
			'r:2:o0',
			'r:2:o1',
			'r:2:o2',
			'r:3:o0'
		]);
		expect(result.collapsedGroupCount).toBe(0);
	});

	it('keeps duplicate record IDs distinct and clamps invalid repeat counts', () => {
		const projection = new LoggerProjection();
		const records = [record(1, 'First', Number.NaN), record(1, 'Second', 0)];
		const result = projection.display(records, 2, true, false, originLabel);
		expect(result.entries.map((entry) => [entry.key, entry.repeatCount])).toEqual([
			['r:1', 1],
			['r:1:d1', 1]
		]);
	});

	it('filters normalized fields and formats a collapsed entry for clipboard', () => {
		const projection = new LoggerProjection();
		const [entry] = projection.display(
			[record(1, 'Ready now', 4)],
			1,
			true,
			false,
			originLabel
		).entries;
		expect(
			filterLoggerEntries([entry], {
				source: normalizeSearchText('  FIXTURE  '),
				tag: normalizeSearchText('TRANSPORT'),
				content: normalizeSearchText('NOW')
			})
		).toEqual([entry]);
		expect(filterLoggerEntries([entry], { source: '', tag: '', content: 'missing' })).toEqual([]);
		expect(formatLogEntryForClipboard(entry)).toMatch(
			/^\[\d{2}:\d{2}:\d{2}\.\d{3}\] \[info\] \[Fixture node\] \[transport\] \[x4\]\nReady now$/
		);
	});

	it('resolves source labels once per session and refreshes after reset', () => {
		const projection = new LoggerProjection();
		let lookups = 0;
		const first = () => {
			lookups += 1;
			return 'Before';
		};
		const rows = [record(1, 'Ready')];
		expect(projection.display(rows, 1, true, false, first).entries[0].sourceLabel).toBe('Before');
		projection.display(rows, 1, true, false, first);
		expect(lookups).toBe(1);
		projection.clear();
		expect(projection.display(rows, 1, true, false, () => 'After').entries[0].sourceLabel).toBe(
			'After'
		);
	});

	it('refreshes cached decorations when a record changes and labels host records as engine', () => {
		const projection = new LoggerProjection();
		const before = projection.display(
			[record(1, 'Before', 1, { origin: undefined, tag: '' })],
			1,
			true,
			false,
			originLabel
		).entries[0];
		const after = projection.display(
			[record(1, 'After', 1, { origin: undefined, tag: '', timestamp_ms: 2000 })],
			1,
			true,
			false,
			originLabel
		).entries[0];
		expect(before.sourceLabel).toBe('engine');
		expect(after.sourceLabel).toBe('engine');
		expect(after.contentFilterText).toBe('after');
		expect(after.formattedTime).not.toBe(before.formattedTime);
		expect(formatLogEntryForClipboard(after)).not.toContain('[]');
	});
});
