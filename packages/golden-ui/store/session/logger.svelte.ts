import type { NodeId, UiEventDto, UiLogRecord, UiSnapshot } from '../../types';
import { DEFAULT_LOG_UI_UPDATE_HZ, normalizeLogUiUpdateHz } from '../logger-ui-config';
import type { WorkbenchToast, WorkbenchToastLevel } from './types';

type PendingLogMutation =
	| { kind: 'clear' }
	| { kind: 'replaceRecord'; record: UiLogRecord }
	| { kind: 'append'; records: UiLogRecord[] };

export interface WorkbenchLoggerStore {
	readonly toasts: WorkbenchToast[];
	readonly logRecords: UiLogRecord[];
	readonly logMaxEntries: number;
	readonly logUiUpdateHz: number;
	dismissToast(toastId: number): void;
	setLogUiUpdateHz(hz: number): void;
	appendUiLogRecord(
		level: WorkbenchToastLevel,
		tag: string,
		message: string,
		origin?: NodeId
	): void;
	applySnapshotLogger(snapshotLogger: UiSnapshot['logger']): void;
	partitionBatchEvents(events: UiEventDto[]): UiEventDto[];
	reset(): void;
}

const MAX_TOASTS = 3;
const DEFAULT_TOAST_DURATION_MS = 5000;

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

const asUiLogRecord = (payload: unknown): UiLogRecord | null => {
	if (!isRecord(payload)) {
		return null;
	}

	const id = Number(payload.id);
	const timestampMs = Number(payload.timestamp_ms);
	const level = payload.level;
	const tag = payload.tag;
	const message = payload.message;
	const originRaw = payload.origin;
	const repeatCountRaw = payload.repeat_count;

	if (
		!Number.isFinite(id) ||
		!Number.isFinite(timestampMs) ||
		(level !== 'success' && level !== 'info' && level !== 'warning' && level !== 'error') ||
		typeof tag !== 'string' ||
		typeof message !== 'string'
	) {
		return null;
	}

	const origin = typeof originRaw === 'number' ? originRaw : undefined;
	const repeatCount =
		typeof repeatCountRaw === 'number' && Number.isFinite(repeatCountRaw)
			? Math.max(1, Math.floor(repeatCountRaw))
			: undefined;
	return {
		id,
		timestamp_ms: timestampMs,
		level,
		tag,
		message,
		repeat_count: repeatCount,
		origin
	};
};

export const createWorkbenchLoggerStore = (): WorkbenchLoggerStore => {
	let toasts = $state<WorkbenchToast[]>([]);
	let logRecords = $state<UiLogRecord[]>([]);
	let logMaxEntries = $state(0);
	let logUiUpdateHz = $state(DEFAULT_LOG_UI_UPDATE_HZ);

	const pendingLogMutations: PendingLogMutation[] = [];
	let logFlushTimer: ReturnType<typeof setTimeout> | null = null;
	let nextToastId = 0;
	let nextUiLogId = -1;
	const toastTimers = new Map<number, ReturnType<typeof setTimeout>>();
	const uiGeneratedLogIds = new Set<number>();

	const clearToastTimer = (toastId: number): void => {
		const timer = toastTimers.get(toastId);
		if (!timer) {
			return;
		}
		clearTimeout(timer);
		toastTimers.delete(toastId);
	};

	const dismissToast = (toastId: number): void => {
		clearToastTimer(toastId);
		const nextToasts = toasts.filter((toast) => toast.id !== toastId);
		if (nextToasts.length !== toasts.length) {
			toasts = nextToasts;
		}
	};

	const scheduleToastDismiss = (toastId: number, durationMs: number): void => {
		clearToastTimer(toastId);
		toastTimers.set(
			toastId,
			setTimeout(() => {
				dismissToast(toastId);
			}, durationMs)
		);
	};

	const clearToasts = (): void => {
		for (const toastId of [...toastTimers.keys()]) {
			clearToastTimer(toastId);
		}
		toasts = [];
	};

	const pushToast = (
		message: string,
		level: WorkbenchToastLevel = 'info',
		durationMs = DEFAULT_TOAST_DURATION_MS
	): void => {
		const normalizedMessage = message.trim();
		if (normalizedMessage.length === 0) {
			return;
		}

		const lastToast = toasts[toasts.length - 1];
		if (lastToast && lastToast.message === normalizedMessage && lastToast.level === level) {
			scheduleToastDismiss(lastToast.id, durationMs);
			return;
		}

		if (toasts.length >= MAX_TOASTS) {
			const oldestToast = toasts[0];
			if (oldestToast) {
				dismissToast(oldestToast.id);
			}
		}

		const toast: WorkbenchToast = {
			id: nextToastId,
			level,
			message: normalizedMessage
		};
		nextToastId += 1;
		toasts = [...toasts, toast];
		scheduleToastDismiss(toast.id, durationMs);
	};

	const emitToastForLogRecord = (
		record: Pick<UiLogRecord, 'level' | 'message'>,
		durationMs = DEFAULT_TOAST_DURATION_MS
	): void => {
		if (record.level === 'info') {
			return;
		}
		pushToast(record.message, record.level, durationMs);
	};

	const compareLogRecords = (left: UiLogRecord, right: UiLogRecord): number => {
		if (left.timestamp_ms !== right.timestamp_ms) {
			return left.timestamp_ms - right.timestamp_ms;
		}
		return left.id - right.id;
	};

	const trimLogRecords = (): void => {
		if (logMaxEntries <= 0) {
			return;
		}
		const overflow = logRecords.length - logMaxEntries;
		if (overflow <= 0) {
			return;
		}
		const removedRecords = logRecords.slice(0, overflow);
		for (const record of removedRecords) {
			uiGeneratedLogIds.delete(record.id);
		}
		logRecords = logRecords.slice(overflow);
	};

	const findLogRecordIndexById = (recordId: number): number => {
		for (let index = logRecords.length - 1; index >= 0; index -= 1) {
			if (logRecords[index]?.id === recordId) {
				return index;
			}
		}
		return -1;
	};

	const clearPendingLogFlush = (): void => {
		if (logFlushTimer !== null) {
			clearTimeout(logFlushTimer);
			logFlushTimer = null;
		}
	};

	const resetPendingLogMutations = (): void => {
		pendingLogMutations.length = 0;
		clearPendingLogFlush();
	};

	const flushPendingLogMutations = (): void => {
		logFlushTimer = null;
		if (pendingLogMutations.length === 0) {
			return;
		}

		for (const mutation of pendingLogMutations) {
			if (mutation.kind === 'clear') {
				if (logRecords.length > 0) {
					logRecords = [];
				}
				uiGeneratedLogIds.clear();
				continue;
			}

			if (mutation.kind === 'replaceRecord') {
				const recordIndex = findLogRecordIndexById(mutation.record.id);
				if (recordIndex >= 0) {
					logRecords[recordIndex] = mutation.record;
					logRecords = [...logRecords];
				} else {
					logRecords = [...logRecords, mutation.record];
				}
				continue;
			}

			if (mutation.records.length > 0) {
				logRecords.push(...mutation.records);
			}
		}

		pendingLogMutations.length = 0;
		trimLogRecords();
	};

	const schedulePendingLogFlush = (): void => {
		if (logFlushTimer !== null) {
			return;
		}
		const intervalMs = Math.max(1, Math.round(1000 / logUiUpdateHz));
		logFlushTimer = setTimeout(() => {
			flushPendingLogMutations();
		}, intervalMs);
	};

	const queuePendingLogMutations = (
		shouldClearLogs: boolean,
		replaceRecords: UiLogRecord[],
		pendingLogRecords: UiLogRecord[]
	): void => {
		if (shouldClearLogs) {
			pendingLogMutations.length = 0;
			pendingLogMutations.push({ kind: 'clear' });
		} else {
			for (const record of replaceRecords) {
				pendingLogMutations.push({ kind: 'replaceRecord', record });
			}
		}

		if (pendingLogRecords.length > 0) {
			pendingLogMutations.push({
				kind: 'append',
				records: [...pendingLogRecords]
			});
		}

		if (pendingLogMutations.length > 0) {
			schedulePendingLogFlush();
		}
	};

	const setLogUiUpdateHz = (hz: number): void => {
		const normalized = normalizeLogUiUpdateHz(hz);
		if (normalized === logUiUpdateHz) {
			return;
		}
		console.log(`Setting log UI update frequency to ${normalized} Hz`);
		logUiUpdateHz = normalized;
		if (pendingLogMutations.length > 0) {
			clearPendingLogFlush();
			schedulePendingLogFlush();
		}
	};

	const appendUiLogRecord = (
		level: WorkbenchToastLevel,
		tag: string,
		message: string,
		origin?: NodeId
	): void => {
		const normalizedMessage = message.trim();
		if (normalizedMessage.length === 0) {
			return;
		}

		flushPendingLogMutations();
		const timestampMs = Date.now();
		const lastRecord = logRecords[logRecords.length - 1];
		if (
			lastRecord &&
			uiGeneratedLogIds.has(lastRecord.id) &&
			lastRecord.level === level &&
			lastRecord.tag === tag &&
			lastRecord.message === normalizedMessage &&
			lastRecord.origin === origin
		) {
			const repeatCount = Math.max(1, Math.floor(lastRecord.repeat_count ?? 1)) + 1;
			const nextRecord: UiLogRecord = {
				...lastRecord,
				timestamp_ms: timestampMs,
				repeat_count: repeatCount
			};
			logRecords = [...logRecords.slice(0, -1), nextRecord];
			emitToastForLogRecord(nextRecord);
			return;
		}

		const record: UiLogRecord = {
			id: nextUiLogId,
			timestamp_ms: timestampMs,
			level,
			tag,
			message: normalizedMessage,
			origin
		};
		nextUiLogId -= 1;
		uiGeneratedLogIds.add(record.id);
		logRecords = [...logRecords, record];
		trimLogRecords();
		emitToastForLogRecord(record);
	};

	const applySnapshotLogger = (snapshotLogger: UiSnapshot['logger']): void => {
		const retainedUiLogRecords = logRecords.filter((record) => uiGeneratedLogIds.has(record.id));
		resetPendingLogMutations();
		logMaxEntries = snapshotLogger.max_entries;
		logRecords = [...snapshotLogger.records, ...retainedUiLogRecords].sort(compareLogRecords);
		trimLogRecords();
	};

	const partitionBatchEvents = (events: UiEventDto[]): UiEventDto[] => {
		const graphEvents: UiEventDto[] = [];
		const pendingLogRecords: UiLogRecord[] = [];
		let shouldClearLogs = false;
		let nextLogMaxEntries: number | null = null;
		const replaceLogRecords = new Map<number, UiLogRecord>();

		for (const event of events) {
			if (event.kind.kind !== 'custom') {
				graphEvents.push(event);
				continue;
			}

			if (event.kind.topic === '__logger.record') {
				const record = asUiLogRecord(event.kind.payload);
				if (!record) {
					continue;
				}
				const pendingTail = pendingLogRecords[pendingLogRecords.length - 1];
				if (pendingTail && pendingTail.id === record.id) {
					pendingLogRecords[pendingLogRecords.length - 1] = record;
					continue;
				}

				if (replaceLogRecords.has(record.id)) {
					replaceLogRecords.set(record.id, record);
					continue;
				}

				if (!shouldClearLogs && findLogRecordIndexById(record.id) >= 0) {
					replaceLogRecords.set(record.id, record);
					continue;
				}

				pendingLogRecords.push(record);
				emitToastForLogRecord(record);
				continue;
			}

			if (event.kind.topic === '__logger.cleared') {
				shouldClearLogs = true;
				pendingLogRecords.length = 0;
				replaceLogRecords.clear();
				continue;
			}

			if (event.kind.topic === '__logger.max_entries') {
				if (
					isRecord(event.kind.payload) &&
					Number.isFinite(Number(event.kind.payload.max_entries))
				) {
					nextLogMaxEntries = Math.max(1, Math.round(Number(event.kind.payload.max_entries)));
				}
				continue;
			}

			graphEvents.push(event);
		}

		if (nextLogMaxEntries !== null) {
			logMaxEntries = nextLogMaxEntries;
			trimLogRecords();
		}

		if (shouldClearLogs || replaceLogRecords.size > 0 || pendingLogRecords.length > 0) {
			queuePendingLogMutations(shouldClearLogs, [...replaceLogRecords.values()], pendingLogRecords);
		}

		return graphEvents;
	};

	const reset = (): void => {
		resetPendingLogMutations();
		uiGeneratedLogIds.clear();
		clearToasts();
		logRecords = [];
		logMaxEntries = 0;
		nextUiLogId = -1;
	};

	return {
		get toasts(): WorkbenchToast[] {
			return toasts;
		},
		get logRecords(): UiLogRecord[] {
			return logRecords;
		},
		get logMaxEntries(): number {
			return logMaxEntries;
		},
		get logUiUpdateHz(): number {
			return logUiUpdateHz;
		},
		dismissToast,
		setLogUiUpdateHz,
		appendUiLogRecord,
		applySnapshotLogger,
		partitionBatchEvents,
		reset
	};
};
