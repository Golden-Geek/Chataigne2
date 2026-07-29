import type { EventTime, UiAck, UiControlLifecycle, UiEditIntent } from '../../types';
import type { WorkbenchIntentProgressObserver } from './intent-pipeline';

export type WorkbenchUserActionPhase = 'queued' | UiControlLifecycle;

interface WorkbenchUserActionEntry {
	id: number;
	label: string;
	phase: WorkbenchUserActionPhase;
	queuedAtMs: number;
	phaseChangedAtMs: number;
}

export interface WorkbenchUserActionHandle {
	lifecycle(phase: UiControlLifecycle): void;
	finish(): void;
}

export interface WorkbenchUserActionActivity {
	readonly pendingCount: number;
	readonly oldestQueuedAtMs: number | null;
	readonly currentLabel: string | null;
	readonly currentPhase: WorkbenchUserActionPhase | null;
	begin(label: string): WorkbenchUserActionHandle;
	reset(): void;
}

export interface WorkbenchUserActionCoordinator {
	readonly pendingCount: number;
	readonly oldestQueuedAtMs: number | null;
	readonly currentLabel: string | null;
	readonly currentPhase: WorkbenchUserActionPhase | null;
	run(
		label: string,
		dispatch: (progress: WorkbenchIntentProgressObserver) => Promise<void>
	): Promise<void>;
	observeAppliedEventTime(eventTime: EventTime): void;
	resetEventBoundary(eventTime: EventTime | null): void;
	reset(): void;
}

interface WorkbenchUserActionCoordinatorOptions {
	nowMs?: () => number;
	afterStateUpdate: () => Promise<void>;
	eventBoundaryTimeoutMs?: number;
	paintTimeoutMs?: number;
	failureVisibleMs?: number;
}

const DEFAULT_EVENT_BOUNDARY_TIMEOUT_MS = 5000;
const DEFAULT_PAINT_TIMEOUT_MS = 100;
const DEFAULT_FAILURE_VISIBLE_MS = 1200;

const normalizedIntentLabel = (value: string | undefined): string | null => {
	const normalized = value?.trim() ?? '';
	return normalized.length > 0 ? normalized : null;
};

/**
 * Returns presentation-only activity text for explicit user work.
 *
 * Edit-session framing, live parameter streams, and node-event traffic are deliberately silent:
 * they are either high-frequency or also carry background preview/demand traffic.
 */
export const userActionLabelForIntent = (intent: UiEditIntent): string | null => {
	switch (intent.kind) {
		case 'beginEdit':
		case 'endEdit':
		case 'setParam':
		case 'sendNodeEvent':
			return null;
		case 'setTextParamSmart':
			return 'Updating text';
		case 'setParamControlState':
			return 'Changing control mode';
		case 'setParamConstraints':
			return 'Updating constraints';
		case 'moveNode':
			return 'Moving node';
		case 'removeNode':
			return 'Removing node';
		case 'removeNodes':
			return intent.nodes.length === 1 ? 'Removing node' : `Removing ${intent.nodes.length} nodes`;
		case 'createUserItem': {
			const label = normalizedIntentLabel(intent.label);
			return label ? `Creating ${label}` : 'Creating item';
		}
		case 'createDashboardContainerWidget':
		case 'createDashboardNodeWidget':
		case 'createDashboardGenericWidget':
			return 'Creating dashboard widget';
		case 'bindDashboardNodeWidgetTarget':
		case 'bindDashboardGenericWidgetTarget':
			return 'Binding dashboard widget';
		case 'wrapDashboardWidgetInContainer':
			return 'Wrapping dashboard widget';
		case 'duplicateNode':
			return 'Duplicating node';
		case 'duplicateNodes': {
			const count = (intent.nodes?.length ?? 0) + (intent.created_items?.length ?? 0);
			return count <= 1 ? 'Duplicating node' : `Duplicating ${count} nodes`;
		}
		case 'fitAnimationCurvePath':
			return 'Fitting animation curve';
		case 'patchMeta':
			return intent.patch.label === undefined ? 'Updating node' : 'Renaming node';
		case 'reevaluateGraph':
			return 'Reevaluating graph';
		case 'clearLogs':
			return 'Clearing logs';
		case 'setLogMaxEntries':
			return 'Updating log settings';
		case 'undo':
			return 'Undoing change';
		case 'redo':
			return 'Redoing change';
		case 'ensureUserContextScope':
			return 'Adding user context';
		case 'removeUserContextScope':
			return 'Removing user context';
		case 'upsertUserContextEntry':
			return 'Updating user context';
		case 'removeUserContextEntry':
			return 'Removing user context entry';
	}
	const exhaustiveIntent: never = intent;
	return exhaustiveIntent;
};

export const userActionLabelForIntents = (intents: readonly UiEditIntent[]): string | null => {
	const editLabel = intents
		.filter(
			(intent): intent is Extract<UiEditIntent, { kind: 'beginEdit' }> =>
				intent.kind === 'beginEdit'
		)
		.map((intent) => normalizedIntentLabel(intent.label))
		.find((label): label is string => label !== null);
	if (editLabel) {
		return editLabel;
	}

	const visibleLabels = intents
		.map((intent) => userActionLabelForIntent(intent))
		.filter((label): label is string => label !== null);
	if (visibleLabels.length === 0) {
		return null;
	}

	if (visibleLabels.length === 1) {
		return visibleLabels[0];
	}
	return `Applying ${visibleLabels.length} changes`;
};

export const createWorkbenchUserActionActivity = (
	nowMs: () => number = () => Date.now()
): WorkbenchUserActionActivity => {
	let entries = $state<WorkbenchUserActionEntry[]>([]);
	let nextId = 0;

	const updatePhase = (id: number, phase: UiControlLifecycle): void => {
		const index = entries.findIndex((entry) => entry.id === id);
		if (index < 0 || entries[index]?.phase === phase) {
			return;
		}
		entries = entries.map((entry) =>
			entry.id === id
				? {
						...entry,
						phase,
						phaseChangedAtMs: nowMs()
					}
				: entry
		);
	};

	const finish = (id: number): void => {
		if (!entries.some((entry) => entry.id === id)) {
			return;
		}
		entries = entries.filter((entry) => entry.id !== id);
	};

	const begin = (label: string): WorkbenchUserActionHandle => {
		nextId += 1;
		const id = nextId;
		const queuedAtMs = nowMs();
		entries = [
			...entries,
			{
				id,
				label: normalizedIntentLabel(label) ?? 'Applying change',
				phase: 'queued',
				queuedAtMs,
				phaseChangedAtMs: queuedAtMs
			}
		];

		let active = true;
		return {
			lifecycle: (phase): void => {
				if (active) {
					updatePhase(id, phase);
				}
			},
			finish: (): void => {
				if (!active) {
					return;
				}
				active = false;
				finish(id);
			}
		};
	};

	return {
		get pendingCount(): number {
			return entries.length;
		},
		get oldestQueuedAtMs(): number | null {
			return entries[0]?.queuedAtMs ?? null;
		},
		get currentLabel(): string | null {
			return entries[0]?.label ?? null;
		},
		get currentPhase(): WorkbenchUserActionPhase | null {
			return entries[0]?.phase ?? null;
		},
		begin,
		reset: (): void => {
			entries = [];
		}
	};
};

const compareEventTime = (left: EventTime, right: EventTime): number =>
	left.tick - right.tick || left.micro - right.micro || left.seq - right.seq;

export const createWorkbenchUserActionCoordinator = (
	options: WorkbenchUserActionCoordinatorOptions
): WorkbenchUserActionCoordinator => {
	const activity = createWorkbenchUserActionActivity(options.nowMs);
	const eventBoundaryTimeoutMs =
		options.eventBoundaryTimeoutMs ?? DEFAULT_EVENT_BOUNDARY_TIMEOUT_MS;
	const paintTimeoutMs = options.paintTimeoutMs ?? DEFAULT_PAINT_TIMEOUT_MS;
	const failureVisibleMs = options.failureVisibleMs ?? DEFAULT_FAILURE_VISIBLE_MS;
	let appliedEventTime: EventTime | null = null;
	let eventBoundaryWaiters: Array<{
		boundary: EventTime;
		resolve: () => void;
		timer: ReturnType<typeof setTimeout>;
	}> = [];

	const observeAppliedEventTime = (eventTime: EventTime): void => {
		if (appliedEventTime === null || compareEventTime(eventTime, appliedEventTime) > 0) {
			appliedEventTime = eventTime;
		}
		const remaining: typeof eventBoundaryWaiters = [];
		for (const waiter of eventBoundaryWaiters) {
			if (appliedEventTime !== null && compareEventTime(appliedEventTime, waiter.boundary) >= 0) {
				clearTimeout(waiter.timer);
				waiter.resolve();
			} else {
				remaining.push(waiter);
			}
		}
		eventBoundaryWaiters = remaining;
	};

	const clearEventBoundaryWaiters = (): void => {
		for (const waiter of eventBoundaryWaiters) {
			clearTimeout(waiter.timer);
			waiter.resolve();
		}
		eventBoundaryWaiters = [];
	};

	const resetEventBoundary = (eventTime: EventTime | null): void => {
		clearEventBoundaryWaiters();
		appliedEventTime = eventTime;
	};

	const waitForAppliedEventBoundary = (boundary: EventTime | undefined): Promise<void> => {
		if (
			boundary === undefined ||
			(appliedEventTime !== null && compareEventTime(appliedEventTime, boundary) >= 0)
		) {
			return Promise.resolve();
		}
		return new Promise((resolve) => {
			const timer = setTimeout(() => {
				eventBoundaryWaiters = eventBoundaryWaiters.filter(
					(candidate) => candidate.resolve !== resolve
				);
				resolve();
			}, eventBoundaryTimeoutMs);
			eventBoundaryWaiters.push({ boundary, resolve, timer });
		});
	};

	const waitForFrame = (): Promise<void> => {
		if (typeof requestAnimationFrame !== 'function') {
			return Promise.resolve();
		}
		return new Promise((resolve) => {
			let settled = false;
			let timer: ReturnType<typeof setTimeout> | null = null;
			const finish = (): void => {
				if (settled) {
					return;
				}
				settled = true;
				if (timer !== null) {
					clearTimeout(timer);
				}
				resolve();
			};
			requestAnimationFrame(finish);
			timer = setTimeout(finish, paintTimeoutMs);
		});
	};

	const waitForAppliedFeedback = async (acknowledgements: readonly UiAck[]): Promise<void> => {
		const boundary = acknowledgements
			.map((ack) => ack.latest_event_time ?? ack.earliest_event_time)
			.filter((time): time is EventTime => time !== undefined)
			.reduce<EventTime | undefined>(
				(latest, time) =>
					latest === undefined || compareEventTime(time, latest) > 0 ? time : latest,
				undefined
			);
		await waitForAppliedEventBoundary(boundary);
		await options.afterStateUpdate();
		// One frame applies a transport-staged delta; the second keeps the
		// finishing feedback visible through an expensive graph render.
		await waitForFrame();
		await waitForFrame();
	};

	const waitForRejectedFeedback = async (): Promise<void> => {
		await Promise.all([
			waitForAppliedFeedback([]),
			new Promise<void>((resolve) => {
				setTimeout(resolve, failureVisibleMs);
			})
		]);
	};

	const run = (
		label: string,
		dispatch: (progress: WorkbenchIntentProgressObserver) => Promise<void>
	): Promise<void> => {
		const handle = activity.begin(label);
		let retirementScheduled = false;
		const scheduleRejectedRetirement = (): void => {
			if (retirementScheduled) {
				return;
			}
			retirementScheduled = true;
			handle.lifecycle('rejected');
			void waitForRejectedFeedback().then(handle.finish, handle.finish);
		};
		const progress: WorkbenchIntentProgressObserver = {
			lifecycle: (phase) => {
				handle.lifecycle(phase);
				if (phase === 'rejected') {
					scheduleRejectedRetirement();
				}
			},
			applied: (acknowledgements) => {
				if (retirementScheduled) {
					return;
				}
				retirementScheduled = true;
				void waitForAppliedFeedback(acknowledgements).then(handle.finish, handle.finish);
			}
		};

		let completion: Promise<void>;
		try {
			completion = dispatch(progress);
		} catch (error) {
			scheduleRejectedRetirement();
			return Promise.reject(error);
		}
		return completion
			.catch((error: unknown) => {
				scheduleRejectedRetirement();
				throw error;
			})
			.finally(() => {
				if (!retirementScheduled) {
					handle.finish();
				}
			});
	};

	return {
		get pendingCount(): number {
			return activity.pendingCount;
		},
		get oldestQueuedAtMs(): number | null {
			return activity.oldestQueuedAtMs;
		},
		get currentLabel(): string | null {
			return activity.currentLabel;
		},
		get currentPhase(): WorkbenchUserActionPhase | null {
			return activity.currentPhase;
		},
		run,
		observeAppliedEventTime,
		resetEventBoundary,
		reset: activity.reset
	};
};
