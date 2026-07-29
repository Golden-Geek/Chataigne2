import type { EventTime, UiEventBatch, UiEventDto, UiGraphOp } from '../types';

const DEFAULT_MAX_WORK_PER_FRAME = 512;
const DEFAULT_MAX_BACKLOG_EVENTS = 8_192;
const DEFAULT_MAX_BACKLOG_WORK = 131_072;
const DEFAULT_MAX_BACKLOG_BYTES = 64 * 1024 * 1024;
const DEFAULT_EVENT_BYTES = 256;
const ESTIMATED_NODE_BYTES = 384;
const ESTIMATED_ID_BYTES = 8;

export interface StagedEventCost {
	work: number;
	estimatedBytes: number;
}

export interface StagedEventWorkResult {
	workUsed: number;
	done: boolean;
}

/**
 * Detached work for one event that is too expensive to execute atomically in a
 * frame. `advance` must not publish partial state. The scheduler emits the
 * original event only after `done` so its consumer can commit prepared state.
 */
export interface StagedEventWork {
	advance(maxWork: number): StagedEventWorkResult;
	cancel?(): void;
}

export interface StagedFrameBatchLimits {
	maxWorkPerFrame: number;
	maxBacklogEvents: number;
	maxBacklogWork: number;
	maxBacklogBytes: number;
}

export interface StagedBacklogOverflow {
	replayFrom: EventTime | undefined;
	queuedEvents: number;
	queuedWork: number;
	queuedBytes: number;
	incomingEvents: number;
	limits: StagedFrameBatchLimits;
}

interface StagedQueuedEvent {
	event: UiEventDto;
	cost: StagedEventCost;
	order: number;
	latestKey?: string;
	superseded: boolean;
}

interface StagedEventRun {
	events: StagedQueuedEvent[];
	index: number;
	end: number;
}

interface ActiveEventWork {
	entry: StagedQueuedEvent;
	work: StagedEventWork;
}

export interface StagedFrameBatchSchedulerOptions {
	onBatch: (batch: UiEventBatch) => void;
	onOrderingViolation: (replayFrom: EventTime | undefined) => void;
	onBacklogOverflow?: (overflow: StagedBacklogOverflow) => void;
	createEventWork?: (event: UiEventDto) => StagedEventWork | undefined;
	estimateEventCost?: (event: UiEventDto) => StagedEventCost;
	limits?: Partial<StagedFrameBatchLimits>;
	isClosed: () => boolean;
	getCursor: () => EventTime | undefined;
	setCursor: (cursor: EventTime) => void;
}

export interface StagedFrameBatchScheduler {
	stage(batch: UiEventBatch): void;
	reset(): void;
}

const compareEventTime = (left: EventTime, right: EventTime): number =>
	left.tick - right.tick || left.micro - right.micro || left.seq - right.seq;

const laterEventTime = (
	left: EventTime | undefined,
	right: EventTime | undefined
): EventTime | undefined => {
	if (!left) {
		return right;
	}
	if (!right) {
		return left;
	}
	return compareEventTime(left, right) >= 0 ? left : right;
};

const graphOpCost = (op: UiGraphOp): StagedEventCost => {
	switch (op.kind) {
		case 'nodeCreated':
			return {
				work: 1 + op.snapshot.children.length,
				estimatedBytes:
					ESTIMATED_NODE_BYTES + op.snapshot.children.length * ESTIMATED_ID_BYTES
			};
		case 'subtreeInserted':
			return {
				// Node count is available without walking every nested child list.
				// The projector charges those child edges precisely while advancing.
				work: 1 + op.nodes.length + op.parent_children_after.length,
				estimatedBytes:
					op.nodes.length * ESTIMATED_NODE_BYTES +
					op.parent_children_after.length * ESTIMATED_ID_BYTES
			};
		case 'subtreeRemoved':
			return {
				work: 1 + op.removed_ids.length + (op.parent_after?.children.length ?? 0),
				estimatedBytes:
					DEFAULT_EVENT_BYTES +
					(op.removed_ids.length + (op.parent_after?.children.length ?? 0)) *
						ESTIMATED_ID_BYTES
			};
		case 'nodeMoved':
			return {
				work:
					1 +
					(op.old_parent_after?.children.length ?? 0) +
					(op.new_parent_after?.children.length ?? 0),
				estimatedBytes:
					DEFAULT_EVENT_BYTES +
					((op.old_parent_after?.children.length ?? 0) +
						(op.new_parent_after?.children.length ?? 0)) *
						ESTIMATED_ID_BYTES
			};
		case 'childrenReordered':
			return {
				work: 1 + op.children.length,
				estimatedBytes: DEFAULT_EVENT_BYTES + op.children.length * ESTIMATED_ID_BYTES
			};
		case 'loggerPatched':
			return {
				work: 1 + op.records_added.length,
				estimatedBytes: DEFAULT_EVENT_BYTES + op.records_added.length * DEFAULT_EVENT_BYTES
			};
		default:
			return { work: 1, estimatedBytes: DEFAULT_EVENT_BYTES };
	}
};

export const estimateStagedEventCost = (event: UiEventDto): StagedEventCost => {
	if (event.kind.kind === 'graphTransaction') {
		let work = 1 + event.kind.ops.length;
		let estimatedBytes = DEFAULT_EVENT_BYTES;
		for (const op of event.kind.ops) {
			const cost = graphOpCost(op);
			work += cost.work;
			estimatedBytes += cost.estimatedBytes;
		}
		return { work, estimatedBytes };
	}
	if (event.kind.kind === 'custom') {
		const payload = event.kind.payload;
		if (typeof payload === 'string') {
			return {
				work: 1,
				estimatedBytes: DEFAULT_EVENT_BYTES + payload.length * 2
			};
		}
		if (Array.isArray(payload)) {
			return {
				work: 1,
				estimatedBytes: DEFAULT_EVENT_BYTES + payload.length * 16
			};
		}
	}
	return { work: 1, estimatedBytes: DEFAULT_EVENT_BYTES };
};

const normalizeCost = (cost: StagedEventCost): StagedEventCost => ({
	work: Math.max(1, Math.ceil(Number.isFinite(cost.work) ? cost.work : 1)),
	estimatedBytes: Math.max(
		1,
		Math.ceil(Number.isFinite(cost.estimatedBytes) ? cost.estimatedBytes : DEFAULT_EVENT_BYTES)
	)
});

const normalizeLimits = (
	limits: Partial<StagedFrameBatchLimits> | undefined
): StagedFrameBatchLimits => ({
	maxWorkPerFrame: Math.max(1, Math.floor(limits?.maxWorkPerFrame ?? DEFAULT_MAX_WORK_PER_FRAME)),
	maxBacklogEvents: Math.max(1, Math.floor(limits?.maxBacklogEvents ?? DEFAULT_MAX_BACKLOG_EVENTS)),
	maxBacklogWork: Math.max(1, Math.floor(limits?.maxBacklogWork ?? DEFAULT_MAX_BACKLOG_WORK)),
	maxBacklogBytes: Math.max(1, Math.floor(limits?.maxBacklogBytes ?? DEFAULT_MAX_BACKLOG_BYTES))
});

const latestEventKey = (event: UiEventDto): string | undefined => {
	switch (event.kind.kind) {
		case 'paramChanged':
			return `param-value:${event.kind.param}`;
		case 'paramControlChanged':
			return `param-control:${event.kind.param}`;
		case 'paramConstraintsChanged':
			return `param-constraints:${event.kind.param}`;
		case 'custom':
			return event.kind.retention === 'latest'
				? `custom:${event.kind.topic}:${event.kind.origin ?? ''}`
				: undefined;
		default:
			return undefined;
	}
};

const compareEntries = (left: StagedQueuedEvent, right: StagedQueuedEvent): number => {
	const timeOrder = compareEventTime(left.event.time, right.event.time);
	return timeOrder || left.order - right.order;
};

const compareRuns = (left: StagedEventRun, right: StagedEventRun): number =>
	compareEntries(left.events[left.index], right.events[right.index]);

const pushRun = (heap: StagedEventRun[], run: StagedEventRun): void => {
	let index = heap.length;
	heap.push(run);
	while (index > 0) {
		const parent = Math.floor((index - 1) / 2);
		const parentRun = heap[parent];
		if (!parentRun || compareRuns(parentRun, run) <= 0) {
			break;
		}
		heap[index] = parentRun;
		index = parent;
	}
	heap[index] = run;
};

const popRun = (heap: StagedEventRun[]): StagedEventRun | undefined => {
	const first = heap[0];
	const last = heap.pop();
	if (!first || !last || heap.length === 0) {
		return first;
	}

	let index = 0;
	while (true) {
		const leftIndex = index * 2 + 1;
		if (leftIndex >= heap.length) {
			break;
		}
		const rightIndex = leftIndex + 1;
		let childIndex = leftIndex;
		const left = heap[leftIndex];
		const right = heap[rightIndex];
		if (right && left && compareRuns(right, left) < 0) {
			childIndex = rightIndex;
		}
		const child = heap[childIndex];
		if (!child || compareRuns(last, child) <= 0) {
			break;
		}
		heap[index] = child;
		index = childIndex;
	}
	heap[index] = last;
	return first;
};

export const createStagedFrameBatchScheduler = (
	options: StagedFrameBatchSchedulerOptions
): StagedFrameBatchScheduler => {
	const limits = normalizeLimits(options.limits);
	const eventRuns: StagedEventRun[] = [];
	const pendingLatest = new Map<string, StagedQueuedEvent>();
	let nextEventOrder = 0;
	let from: EventTime | undefined;
	let runtime: UiEventBatch['runtime'];
	let hasBatch = false;
	let frameScheduled = false;
	let generation = 0;
	let queuedEvents = 0;
	let queuedWork = 0;
	let queuedBytes = 0;
	let retainedSupersededEntries = 0;
	let activeWork: ActiveEventWork | undefined;

	const cancelActiveWork = (): void => {
		activeWork?.work.cancel?.();
		activeWork = undefined;
	};

	const reset = (): void => {
		cancelActiveWork();
		eventRuns.length = 0;
		pendingLatest.clear();
		nextEventOrder = 0;
		from = undefined;
		runtime = undefined;
		hasBatch = false;
		frameScheduled = false;
		queuedEvents = 0;
		queuedWork = 0;
		queuedBytes = 0;
		retainedSupersededEntries = 0;
		generation += 1;
	};

	const removeQueuedEntry = (entry: StagedQueuedEvent): void => {
		queuedEvents = Math.max(0, queuedEvents - 1);
		queuedWork = Math.max(0, queuedWork - entry.cost.work);
		queuedBytes = Math.max(0, queuedBytes - entry.cost.estimatedBytes);
		if (entry.latestKey && pendingLatest.get(entry.latestKey) === entry) {
			pendingLatest.delete(entry.latestKey);
		}
	};

	const advanceRun = (run: StagedEventRun): void => {
		run.index += 1;
		if (run.index < run.end) {
			pushRun(eventRuns, run);
		}
	};

	const popNextQueuedEntry = (): StagedQueuedEvent | undefined => {
		while (eventRuns.length > 0) {
			const run = popRun(eventRuns);
			if (!run) {
				return undefined;
			}
			const entry = run.events[run.index];
			advanceRun(run);
			if (!entry || entry.superseded) {
				if (entry?.superseded) {
					retainedSupersededEntries = Math.max(0, retainedSupersededEntries - 1);
				}
				continue;
			}
			return entry;
		}
		return undefined;
	};

	const peekNextQueuedEntry = (): StagedQueuedEvent | undefined => {
		while (eventRuns.length > 0) {
			const run = eventRuns[0];
			const entry = run?.events[run.index];
			if (entry && !entry.superseded) {
				return entry;
			}
			const discarded = popRun(eventRuns);
			if (discarded) {
				const discardedEntry = discarded.events[discarded.index];
				if (discardedEntry?.superseded) {
					retainedSupersededEntries = Math.max(0, retainedSupersededEntries - 1);
				}
				advanceRun(discarded);
			}
		}
		return undefined;
	};

	const requeueEntry = (entry: StagedQueuedEvent): void => {
		pushRun(eventRuns, { events: [entry], index: 0, end: 1 });
	};

	const stageRuns = (entries: StagedQueuedEvent[]): void => {
		let runStart = 0;
		for (let index = 1; index <= entries.length; index += 1) {
			const previous = entries[index - 1];
			const current = entries[index];
			if (
				index < entries.length &&
				previous &&
				current &&
				compareEntries(previous, current) <= 0
			) {
				continue;
			}
			if (runStart < index) {
				pushRun(eventRuns, {
					events: entries,
					index: runStart,
					end: index
				});
			}
			runStart = index;
		}
	};

	const compactSupersededRuns = (): void => {
		const retainedSlack = Math.max(64, Math.min(1_024, limits.maxBacklogEvents));
		if (retainedSupersededEntries <= retainedSlack) {
			return;
		}
		const live: StagedQueuedEvent[] = [];
		while (eventRuns.length > 0) {
			const run = popRun(eventRuns);
			if (!run) {
				break;
			}
			for (let index = run.index; index < run.end; index += 1) {
				const entry = run.events[index];
				if (entry && !entry.superseded) {
					live.push(entry);
				}
			}
		}
		live.sort(compareEntries);
		if (live.length > 0) {
			pushRun(eventRuns, { events: live, index: 0, end: live.length });
		}
		retainedSupersededEntries = 0;
	};

	const schedule = (): void => {
		if (options.isClosed() || frameScheduled || !hasBatch) {
			return;
		}
		frameScheduled = true;
		const scheduledGeneration = generation;
		const flush = (): void => {
			if (generation !== scheduledGeneration) {
				return;
			}
			frameScheduled = false;
			if (options.isClosed() || !hasBatch) {
				return;
			}

			const events: UiEventDto[] = [];
			let remainingWork = limits.maxWorkPerFrame;
			let projectedEventCompleted = false;

			if (activeWork?.entry.superseded) {
				cancelActiveWork();
			}
			const earlierEntry = peekNextQueuedEntry();
			if (
				activeWork &&
				earlierEntry &&
				compareEntries(earlierEntry, activeWork.entry) < 0
			) {
				const interrupted = activeWork.entry;
				cancelActiveWork();
				requeueEntry(interrupted);
			}

			while (remainingWork > 0) {
				if (activeWork) {
					const result = activeWork.work.advance(remainingWork);
					const workUsed = Math.max(
						1,
						Math.min(
							remainingWork,
							Math.ceil(Number.isFinite(result.workUsed) ? result.workUsed : 1)
						)
					);
					remainingWork -= workUsed;
					if (!result.done) {
						break;
					}
					events.push(activeWork.entry.event);
					removeQueuedEntry(activeWork.entry);
					activeWork = undefined;
					projectedEventCompleted = true;
					break;
				}

				const entry = popNextQueuedEntry();
				if (!entry) {
					break;
				}
				if (entry.cost.work > remainingWork && events.length > 0) {
					requeueEntry(entry);
					break;
				}

				if (entry.cost.work > limits.maxWorkPerFrame && options.createEventWork) {
					const work = options.createEventWork(entry.event);
					if (work) {
						activeWork = { entry, work };
						continue;
					}
				}

				events.push(entry.event);
				removeQueuedEntry(entry);
				remainingWork -= Math.min(remainingWork, entry.cost.work);
			}

			const drained = queuedEvents === 0;
			const eventTo = events.at(-1)?.time;
			const cursor = options.getCursor();
			// A plane's source `to` can cover peer-plane events that have not arrived
			// yet. Only an event actually emitted to the consumer may advance resume
			// state; reconnect then replays every unapplied peer plane and staged tail.
			const nextCursor = eventTo ? laterEventTime(cursor, eventTo) : undefined;
			const shouldPublish = events.length > 0 || runtime !== undefined;
			const batch: UiEventBatch = {
				from,
				to: nextCursor,
				runtime,
				events
			};
			if (shouldPublish) {
				runtime = undefined;
			}
			if (nextCursor) {
				options.setCursor(nextCursor);
			}

			if (drained) {
				nextEventOrder = 0;
				from = undefined;
				hasBatch = runtime !== undefined;
			} else if (nextCursor) {
				from = nextCursor;
			}

			const continueScheduling = (): void => {
				// A projected graph event is an atomic barrier. Publishing it before
				// preparing the next event lets the graph store establish the exact
				// base state for the next detached projection.
				if (projectedEventCompleted || !drained || runtime !== undefined) {
					schedule();
				}
			};
			if (!shouldPublish) {
				continueScheduling();
				return;
			}
			try {
				options.onBatch(batch);
			} finally {
				continueScheduling();
			}
		};
		if (typeof requestAnimationFrame === 'function') {
			requestAnimationFrame(flush);
		} else {
			setTimeout(flush, 0);
		}
	};

	const stage = (batch: UiEventBatch): void => {
		const cursor = options.getCursor();
		// Distinct planes can arrive after an earlier plane has already painted.
		// Applying an unseen older event would violate global EngineTime ordering,
		// while dropping it would silently lose a reliable event. Rewind the
		// reconnect cursor to the source group's safe boundary and require a full
		// snapshot recovery instead.
		if (cursor && batch.events.some((event) => compareEventTime(event.time, cursor) <= 0)) {
			reset();
			options.onOrderingViolation(batch.from);
			return;
		}
		if (batch.events.length === 0 && !batch.runtime) {
			return;
		}

		const incoming = batch.events.map<StagedQueuedEvent>((event) => ({
			event,
			cost: normalizeCost((options.estimateEventCost ?? estimateStagedEventCost)(event)),
			order: nextEventOrder++,
			latestKey: latestEventKey(event),
			superseded: false
		}));
		const incomingLatest = new Map<string, StagedQueuedEvent>();
		for (const entry of incoming) {
			if (!entry.latestKey) {
				continue;
			}
			const previous = incomingLatest.get(entry.latestKey);
			if (previous) {
				if (compareEntries(previous, entry) <= 0) {
					previous.superseded = true;
				} else {
					entry.superseded = true;
					continue;
				}
			}
			incomingLatest.set(entry.latestKey, entry);
		}
		const candidateIncoming = incoming.filter((entry) => !entry.superseded);
		const replaced = new Set<StagedQueuedEvent>();
		for (const entry of candidateIncoming) {
			if (!entry.latestKey) {
				continue;
			}
			const previous = pendingLatest.get(entry.latestKey);
			if (previous && !previous.superseded) {
				if (compareEntries(previous, entry) <= 0) {
					replaced.add(previous);
				} else {
					entry.superseded = true;
				}
			}
		}
		const liveIncoming = candidateIncoming.filter((entry) => !entry.superseded);

		let projectedEvents = queuedEvents - replaced.size + liveIncoming.length;
		let projectedWork = queuedWork;
		let projectedBytes = queuedBytes;
		for (const entry of replaced) {
			projectedWork -= entry.cost.work;
			projectedBytes -= entry.cost.estimatedBytes;
		}
		for (const entry of liveIncoming) {
			projectedWork += entry.cost.work;
			projectedBytes += entry.cost.estimatedBytes;
		}
		const overflow =
			projectedEvents > limits.maxBacklogEvents ||
			projectedWork > limits.maxBacklogWork ||
			projectedBytes > limits.maxBacklogBytes;
		if (overflow) {
			const overflowState: StagedBacklogOverflow = {
				replayFrom: cursor ?? from ?? batch.from,
				queuedEvents: projectedEvents,
				queuedWork: projectedWork,
				queuedBytes: projectedBytes,
				incomingEvents: liveIncoming.length,
				limits
			};
			reset();
			if (options.onBacklogOverflow) {
				options.onBacklogOverflow(overflowState);
			} else {
				// Existing transports already recover ordering violations through a
				// coordinated snapshot. Until they wire the richer callback, overflow
				// uses the same safe recovery path rather than dropping reliable work.
				options.onOrderingViolation(overflowState.replayFrom);
			}
			return;
		}

		for (const entry of replaced) {
			entry.superseded = true;
			if (activeWork?.entry !== entry) {
				retainedSupersededEntries += 1;
			}
			removeQueuedEntry(entry);
		}
		for (const entry of liveIncoming) {
			queuedEvents += 1;
			queuedWork += entry.cost.work;
			queuedBytes += entry.cost.estimatedBytes;
			if (entry.latestKey) {
				pendingLatest.set(entry.latestKey, entry);
			}
		}

		if (!hasBatch) {
			hasBatch = true;
			from = laterEventTime(cursor, batch.from);
		} else if (!from && batch.from) {
			from = batch.from;
		}
		if (batch.runtime) {
			runtime = batch.runtime;
		}
		stageRuns(liveIncoming);
		compactSupersededRuns();
		schedule();
	};

	return { stage, reset };
};
