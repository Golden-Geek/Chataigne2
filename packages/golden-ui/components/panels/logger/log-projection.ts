import type { NodeId, UiLogRecord } from '../../../types';

export interface LoggerEntry {
	key: string;
	record: UiLogRecord;
	sourceLabel: string;
	sourceFilterText: string;
	tagFilterText: string;
	contentFilterText: string;
	formattedTime: string;
	repeatCount: number;
}

export interface DisplayedEntriesResult {
	entries: LoggerEntry[];
	collapsedGroupCount: number;
}

interface CachedRecordDecorations {
	timestampMs: number;
	sourceLabel: string;
	sourceFilterText: string;
	tagFilterText: string;
	contentFilterText: string;
	formattedTime: string;
}

interface CollapsedGroup {
	key: string;
	record: UiLogRecord;
	repeatCount: number;
	latestRecordIndex: number;
}

export interface LoggerFilters {
	source: string;
	tag: string;
	content: string;
}

export const normalizeSearchText = (value: string): string => value.trim().toLowerCase();

const repeatCountForRecord = (record: UiLogRecord): number => {
	const raw = Number(record.repeat_count ?? 1);
	if (!Number.isFinite(raw)) {
		return 1;
	}
	return Math.max(1, Math.floor(raw));
};

const formatTimestamp = (timestampMs: number): string => {
	const date = new Date(timestampMs);
	const hours = String(date.getHours()).padStart(2, '0');
	const minutes = String(date.getMinutes()).padStart(2, '0');
	const seconds = String(date.getSeconds()).padStart(2, '0');
	const milliseconds = String(date.getMilliseconds()).padStart(3, '0');
	return `${hours}:${minutes}:${seconds}.${milliseconds}`;
};

const uniqueRecordKey = (recordId: number, duplicateIndex: number): string =>
	duplicateIndex <= 0 ? `r:${recordId}` : `r:${recordId}:d${duplicateIndex}`;

const collapseSignatureForRecord = (record: UiLogRecord): string =>
	JSON.stringify([record.level, record.tag, record.origin ?? null, record.message]);

export const formatLogEntryForClipboard = (entry: LoggerEntry): string => {
	const record = entry.record;
	const headerParts = [
		`[${entry.formattedTime}]`,
		`[${record.level}]`,
		`[${entry.sourceLabel}]`,
		record.tag.trim().length > 0 ? `[${record.tag}]` : null,
		entry.repeatCount > 1 ? `[x${entry.repeatCount}]` : null
	].filter((part): part is string => part !== null);
	const header = headerParts.join(' ');
	return record.message.length > 0 ? `${header}\n${record.message}` : header;
};

export const filterLoggerEntries = (
	entries: LoggerEntry[],
	filters: LoggerFilters
): LoggerEntry[] => {
	if (!filters.source && !filters.tag && !filters.content) {
		return entries;
	}
	return entries.filter(
		(entry) =>
			(!filters.source || entry.sourceFilterText.includes(filters.source)) &&
			(!filters.tag || entry.tagFilterText.includes(filters.tag)) &&
			(!filters.content || entry.contentFilterText.includes(filters.content))
	);
};

export class LoggerProjection {
	readonly #sourceLabelCache = new Map<NodeId, string>();
	readonly #recordDecorationsCache = new Map<number, CachedRecordDecorations>();

	clear(): void {
		this.#sourceLabelCache.clear();
		this.#recordDecorationsCache.clear();
	}

	retain(records: readonly UiLogRecord[]): void {
		if (records.length === 0) {
			this.#recordDecorationsCache.clear();
			return;
		}
		const retainedIds = new Set(records.map((record) => record.id));
		for (const cachedId of this.#recordDecorationsCache.keys()) {
			if (!retainedIds.has(cachedId)) {
				this.#recordDecorationsCache.delete(cachedId);
			}
		}
	}

	display(
		records: readonly UiLogRecord[],
		renderLimit: number,
		collapseDuplicates: boolean,
		collapseAllDuplicates: boolean,
		resolveOriginLabel: (origin: NodeId) => string | undefined
	): DisplayedEntriesResult {
		if (collapseDuplicates) {
			if (collapseAllDuplicates) {
				const groupsBySignature = new Map<string, CollapsedGroup>();
				for (let recordIndex = 0; recordIndex < records.length; recordIndex += 1) {
					const record = records[recordIndex];
					const signature = collapseSignatureForRecord(record);
					const repeatCount = repeatCountForRecord(record);
					const existing = groupsBySignature.get(signature);
					if (existing) {
						existing.record = record;
						existing.latestRecordIndex = recordIndex;
						existing.repeatCount += repeatCount;
					} else {
						groupsBySignature.set(signature, {
							key: `g:${record.id}`,
							record,
							repeatCount,
							latestRecordIndex: recordIndex
						});
					}
				}
				const sortedGroups = [...groupsBySignature.values()].sort(
					(left, right) => left.latestRecordIndex - right.latestRecordIndex
				);
				const startIndex = Math.max(0, sortedGroups.length - renderLimit);
				return {
					entries: sortedGroups
						.slice(startIndex)
						.map((group) =>
							this.#makeEntry(group.record, group.key, group.repeatCount, resolveOriginLabel)
						),
					collapsedGroupCount: sortedGroups.length
				};
			}

			const entries: LoggerEntry[] = [];
			const duplicateCountById = new Map<number, number>();
			const startIndex = Math.max(0, records.length - renderLimit);
			for (let recordIndex = startIndex; recordIndex < records.length; recordIndex += 1) {
				const record = records[recordIndex];
				const duplicateIndex = duplicateCountById.get(record.id) ?? 0;
				duplicateCountById.set(record.id, duplicateIndex + 1);
				entries.push(
					this.#makeEntry(
						record,
						uniqueRecordKey(record.id, duplicateIndex),
						repeatCountForRecord(record),
						resolveOriginLabel
					)
				);
			}
			return { entries, collapsedGroupCount: entries.length };
		}

		let remaining = renderLimit;
		const reversedEntries: LoggerEntry[] = [];
		const duplicateCountById = new Map<number, number>();
		for (let recordIndex = records.length - 1; recordIndex >= 0; recordIndex -= 1) {
			if (remaining <= 0) {
				break;
			}
			const record = records[recordIndex];
			const duplicateIndex = duplicateCountById.get(record.id) ?? 0;
			duplicateCountById.set(record.id, duplicateIndex + 1);
			const recordKey = uniqueRecordKey(record.id, duplicateIndex);
			const repeatCount = repeatCountForRecord(record);
			const takeCount = Math.min(remaining, repeatCount);
			for (let offset = 0; offset < takeCount; offset += 1) {
				const occurrenceIndex = repeatCount - 1 - offset;
				reversedEntries.push(
					this.#makeEntry(record, `${recordKey}:o${occurrenceIndex}`, 1, resolveOriginLabel)
				);
			}
			remaining -= takeCount;
		}
		reversedEntries.reverse();
		return { entries: reversedEntries, collapsedGroupCount: 0 };
	}

	#makeEntry(
		record: UiLogRecord,
		key: string,
		repeatCount: number,
		resolveOriginLabel: (origin: NodeId) => string | undefined
	): LoggerEntry {
		const decorations = this.#decorationsForRecord(record, resolveOriginLabel);
		return {
			key,
			record,
			sourceLabel: decorations.sourceLabel,
			sourceFilterText: decorations.sourceFilterText,
			tagFilterText: decorations.tagFilterText,
			contentFilterText: decorations.contentFilterText,
			formattedTime: decorations.formattedTime,
			repeatCount
		};
	}

	#decorationsForRecord(
		record: UiLogRecord,
		resolveOriginLabel: (origin: NodeId) => string | undefined
	): CachedRecordDecorations {
		const sourceLabel = this.#sourceLabel(record, resolveOriginLabel);
		const sourceFilterText = sourceLabel.toLowerCase();
		const tagFilterText = record.tag.toLowerCase();
		const contentFilterText = record.message.toLowerCase();
		const cached = this.#recordDecorationsCache.get(record.id);
		if (
			cached &&
			cached.timestampMs === record.timestamp_ms &&
			cached.sourceFilterText === sourceFilterText &&
			cached.tagFilterText === tagFilterText &&
			cached.contentFilterText === contentFilterText
		) {
			return cached;
		}
		const next: CachedRecordDecorations = {
			timestampMs: record.timestamp_ms,
			sourceLabel,
			sourceFilterText,
			tagFilterText,
			contentFilterText,
			formattedTime: formatTimestamp(record.timestamp_ms)
		};
		this.#recordDecorationsCache.set(record.id, next);
		return next;
	}

	#sourceLabel(
		record: UiLogRecord,
		resolveOriginLabel: (origin: NodeId) => string | undefined
	): string {
		if (record.origin === undefined) {
			return 'engine';
		}
		const cached = this.#sourceLabelCache.get(record.origin);
		if (cached !== undefined) {
			return cached;
		}
		const label = resolveOriginLabel(record.origin);
		const resolved = label ? `${label}` : `node ${record.origin}`;
		this.#sourceLabelCache.set(record.origin, resolved);
		return resolved;
	}
}
