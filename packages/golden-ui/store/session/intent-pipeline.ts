import type { NodeId, UiAck, UiControlLifecycle, UiEditIntent } from '../../types';

export interface WorkbenchIntentProgressObserver {
	lifecycle?(phase: UiControlLifecycle): void;
	applied?(acknowledgements: readonly UiAck[]): void;
}

interface IntentWaiter {
	resolve: () => void;
	reject: (reason?: unknown) => void;
}

interface QueuedIntent {
	intent: UiEditIntent;
	snapshotGeneration: number;
	waiters: IntentWaiter[];
	progressObservers: WorkbenchIntentProgressObserver[];
}

export interface WorkbenchIntentPipelineOptions {
	getSnapshotGeneration(): number;
	dispatchIntent(intent: UiEditIntent, progress?: WorkbenchIntentProgressObserver): Promise<void>;
	dispatchIntents(
		intents: UiEditIntent[],
		progress?: WorkbenchIntentProgressObserver
	): Promise<void>;
}

export interface WorkbenchIntentPipeline {
	sendIntent(intent: UiEditIntent, progress?: WorkbenchIntentProgressObserver): Promise<void>;
	sendIntents(intents: UiEditIntent[], progress?: WorkbenchIntentProgressObserver): Promise<void>;
	suspendDispatch(): () => void;
	blockDispatch(reason: Error): () => void;
	runProjectGraphTransition<T>(operation: () => Promise<T>): Promise<T>;
	rejectPending(reason: Error): void;
}

const MAX_SET_PARAM_BATCH_SIZE = 512;

export const createWorkbenchIntentPipeline = (
	options: WorkbenchIntentPipelineOptions
): WorkbenchIntentPipeline => {
	let pending: QueuedIntent[] = [];
	let pendingHead = 0;
	const queuedSetParamsByGeneration = new Map<number, Map<NodeId, QueuedIntent>>();
	const idleWaiters: Array<() => void> = [];
	const dispatchBlocks: Array<{ id: number; reason: Error }> = [];
	let processing = false;
	let dispatchSuspensions = 0;
	let nextDispatchBlockId = 0;
	let activeDispatches = 0;

	const notifyIdle = (): void => {
		if (processing || activeDispatches > 0) {
			return;
		}
		for (const resolve of idleWaiters.splice(0)) {
			resolve();
		}
	};

	const waitForIdle = (): Promise<void> => {
		if (!processing && activeDispatches === 0) {
			return Promise.resolve();
		}
		return new Promise<void>((resolve) => {
			idleWaiters.push(resolve);
		});
	};

	const dispatch = async (operation: () => Promise<void>): Promise<void> => {
		activeDispatches += 1;
		try {
			await operation();
		} finally {
			activeDispatches = Math.max(0, activeDispatches - 1);
			notifyIdle();
		}
	};

	const resolveQueued = (queued: QueuedIntent[]): void => {
		for (const entry of queued) {
			for (const waiter of entry.waiters) {
				waiter.resolve();
			}
		}
	};

	const rejectQueued = (queued: QueuedIntent[], reason: unknown): void => {
		for (const entry of queued) {
			for (const waiter of entry.waiters) {
				waiter.reject(reason);
			}
		}
	};

	const hasPending = (): boolean => pendingHead < pending.length;

	const peekPending = (): QueuedIntent | undefined => pending[pendingHead];

	const indexQueuedSetParam = (queued: QueuedIntent): void => {
		if (queued.intent.kind !== 'setParam') {
			return;
		}
		let byNode = queuedSetParamsByGeneration.get(queued.snapshotGeneration);
		if (!byNode) {
			byNode = new Map();
			queuedSetParamsByGeneration.set(queued.snapshotGeneration, byNode);
		}
		byNode.set(queued.intent.node, queued);
	};

	const unindexQueuedSetParam = (queued: QueuedIntent): void => {
		if (queued.intent.kind !== 'setParam') {
			return;
		}
		const byNode = queuedSetParamsByGeneration.get(queued.snapshotGeneration);
		if (byNode?.get(queued.intent.node) !== queued) {
			return;
		}
		byNode.delete(queued.intent.node);
		if (byNode.size === 0) {
			queuedSetParamsByGeneration.delete(queued.snapshotGeneration);
		}
	};

	const compactPending = (): void => {
		if (pendingHead === pending.length) {
			pending = [];
			pendingHead = 0;
			return;
		}
		if (pendingHead >= 1024 && pendingHead * 2 >= pending.length) {
			pending = pending.slice(pendingHead);
			pendingHead = 0;
		}
	};

	const dequeuePending = (): QueuedIntent | undefined => {
		const queued = peekPending();
		if (!queued) {
			return undefined;
		}
		pendingHead += 1;
		unindexQueuedSetParam(queued);
		compactPending();
		return queued;
	};

	const clearPending = (): QueuedIntent[] => {
		const queued = pending.slice(pendingHead);
		pending = [];
		pendingHead = 0;
		queuedSetParamsByGeneration.clear();
		return queued;
	};

	const rejectPending = (reason: Error): void => {
		const queued = clearPending();
		rejectQueued(queued, reason);
	};

	const notifyLifecycle = (queued: readonly QueuedIntent[], phase: UiControlLifecycle): void => {
		for (const entry of queued) {
			for (const observer of entry.progressObservers) {
				observer.lifecycle?.(phase);
			}
		}
	};

	const notifyApplied = (
		queued: readonly QueuedIntent[],
		acknowledgements: readonly UiAck[]
	): void => {
		for (const entry of queued) {
			for (const observer of entry.progressObservers) {
				observer.applied?.(acknowledgements);
			}
		}
	};

	const findQueuedSetParam = (node: NodeId, snapshotGeneration: number): QueuedIntent | undefined =>
		queuedSetParamsByGeneration.get(snapshotGeneration)?.get(node);

	const drainLeadingSetParams = (snapshotGeneration: number): QueuedIntent[] => {
		const drained: QueuedIntent[] = [];
		while (drained.length < MAX_SET_PARAM_BATCH_SIZE && hasPending()) {
			const next = peekPending();
			if (
				!next ||
				next.snapshotGeneration !== snapshotGeneration ||
				next.intent.kind !== 'setParam'
			) {
				break;
			}
			const queued = dequeuePending();
			if (queued) {
				drained.push(queued);
			}
		}
		return drained;
	};

	const process = async (): Promise<void> => {
		if (processing) {
			return;
		}
		processing = true;
		try {
			while (hasPending()) {
				if (dispatchSuspensions > 0 || dispatchBlocks.length > 0) {
					break;
				}

				const next = peekPending();
				if (next?.snapshotGeneration !== options.getSnapshotGeneration()) {
					const stale = dequeuePending();
					if (stale) {
						rejectQueued(
							[stale],
							new Error('intent cancelled because the runtime graph snapshot changed')
						);
					}
					continue;
				}

				if (next?.intent.kind === 'setParam') {
					const batch = drainLeadingSetParams(next.snapshotGeneration);
					if (batch.length === 0) {
						continue;
					}
					try {
						await dispatch(() =>
							options.dispatchIntents(
								batch.map((entry) => entry.intent),
								{
									lifecycle: (phase) => notifyLifecycle(batch, phase),
									applied: (acknowledgements) => notifyApplied(batch, acknowledgements)
								}
							)
						);
						resolveQueued(batch);
					} catch (error) {
						rejectQueued(batch, error);
					}
					continue;
				}

				const queued = dequeuePending();
				if (!queued) {
					continue;
				}
				try {
					await dispatch(() =>
						options.dispatchIntent(queued.intent, {
							lifecycle: (phase) => notifyLifecycle([queued], phase),
							applied: (acknowledgements) => notifyApplied([queued], acknowledgements)
						})
					);
					resolveQueued([queued]);
				} catch (error) {
					rejectQueued([queued], error);
				}
			}
		} finally {
			processing = false;
			notifyIdle();
			if (dispatchSuspensions === 0 && dispatchBlocks.length === 0 && hasPending()) {
				void process();
			}
		}
	};

	const suspendDispatch = (): (() => void) => {
		dispatchSuspensions += 1;
		let released = false;
		return () => {
			if (released) {
				return;
			}
			released = true;
			dispatchSuspensions = Math.max(0, dispatchSuspensions - 1);
			if (dispatchSuspensions === 0 && dispatchBlocks.length === 0 && hasPending()) {
				void process();
			}
		};
	};

	const blockDispatch = (reason: Error): (() => void) => {
		nextDispatchBlockId += 1;
		const id = nextDispatchBlockId;
		dispatchBlocks.push({ id, reason });
		let released = false;
		return () => {
			if (released) {
				return;
			}
			released = true;
			const index = dispatchBlocks.findIndex((block) => block.id === id);
			if (index >= 0) {
				dispatchBlocks.splice(index, 1);
			}
			if (dispatchBlocks.length === 0 && dispatchSuspensions === 0 && hasPending()) {
				void process();
			}
		};
	};

	const activeDispatchBlockReason = (): Error | null =>
		dispatchBlocks[dispatchBlocks.length - 1]?.reason ?? null;

	const sendIntent = (
		intent: UiEditIntent,
		progress?: WorkbenchIntentProgressObserver
	): Promise<void> => {
		const blockedReason = activeDispatchBlockReason();
		if (blockedReason) {
			return Promise.reject(blockedReason);
		}
		return new Promise<void>((resolve, reject) => {
			const snapshotGeneration = options.getSnapshotGeneration();
			if (intent.kind === 'setParam') {
				const queued = findQueuedSetParam(intent.node, snapshotGeneration);
				if (queued) {
					queued.intent = intent;
					queued.waiters.push({ resolve, reject });
					if (progress) {
						queued.progressObservers.push(progress);
					}
					void process();
					return;
				}
			}

			const queued: QueuedIntent = {
				intent,
				snapshotGeneration,
				waiters: [{ resolve, reject }],
				progressObservers: progress ? [progress] : []
			};
			pending.push(queued);
			indexQueuedSetParam(queued);
			void process();
		});
	};

	const sendIntents = async (
		intents: UiEditIntent[],
		progress?: WorkbenchIntentProgressObserver
	): Promise<void> => {
		const blockedReason = activeDispatchBlockReason();
		if (blockedReason) {
			throw blockedReason;
		}
		if (dispatchSuspensions > 0) {
			throw new Error('intent batch cancelled because the runtime graph is synchronizing');
		}
		await dispatch(() => options.dispatchIntents(intents, progress));
	};

	const runProjectGraphTransition = async <T>(operation: () => Promise<T>): Promise<T> => {
		const release = suspendDispatch();
		rejectPending(new Error('intent cancelled because a project graph transition started'));
		try {
			await waitForIdle();
			return await operation();
		} finally {
			rejectPending(
				new Error('intent cancelled because it was produced during a project graph transition')
			);
			release();
		}
	};

	return {
		sendIntent,
		sendIntents,
		suspendDispatch,
		blockDispatch,
		runProjectGraphTransition,
		rejectPending
	};
};
