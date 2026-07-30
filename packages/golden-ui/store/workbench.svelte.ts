import { tick } from 'svelte';
import { createGraphStore, type GraphStore } from './graph.svelte';
import {
	createDefaultUiClient,
	type UiTransportConnectionState,
	type UiTransportFactory,
	type UiTransportOptions
} from '../transport';
import type {
	EventTime,
	NodeId,
	UiNodeDto,
	UiClient,
	UiEditIntent,
	UiEventBatch,
	UiRuntimeStats,
	UiLogRecord,
	UiAck,
	UiSubscriptionBatchControl,
	UiSnapshot,
	UiSubscriptionScope
} from '../types';
import { wholeGraphScope } from '../types';
import { registerGlobalCommandShortcuts } from './commands.svelte';
import { appState } from './app-state.svelte';
import { resetProjectFileFormat, setProjectFileFormat } from './project-file-format.svelte';
import { syncProjectFilePathFromSnapshot } from './project-files.svelte';
import {
	recordPerformanceSample,
	setPerformanceProfilerEnabled
} from './performance-profiler.svelte';
import { createWorkbenchDescriptionStore } from './session/descriptions.svelte';
import { createWorkbenchCustomEventStore } from './session/custom-events.svelte';
import { createWorkbenchFooterHoverStore } from './session/footer-hover.svelte';
import { createWorkbenchHistoryStore } from './session/history.svelte';
import { createWorkbenchLoggerStore } from './session/logger.svelte';
import {
	formatReconnectDelay,
	workbenchConnectionStatus,
	type WorkbenchConnectionStatus
} from './session/connection-state';
import {
	createWorkbenchUserActionCoordinator,
	userActionLabelForIntent,
	userActionLabelForIntents,
	type WorkbenchUserActionPhase
} from './session/action-activity.svelte';
import {
	createWorkbenchIntentPipeline,
	type WorkbenchIntentProgressObserver,
	type WorkbenchIntentPipeline
} from './session/intent-pipeline';
import {
	createWorkbenchLoadingController,
	type WorkbenchLoadingOperation,
	type WorkbenchLoadingState,
	type WorkbenchLoadingStateRequest
} from './session/loading.svelte';
import {
	createProjectGraphTransitionController,
	type ProjectGraphTransitionControl,
	type ProjectGraphTransitionController
} from './session/project-graph-transition.svelte';
import {
	engineLowFrequencyFromGraph,
	parameterControlStateEquals
} from './session/runtime-settings';
import {
	createWorkbenchSnapshotController,
	type WorkbenchSnapshotController
} from './session/snapshots';
import { createWorkbenchSelectionStore } from './session/selection.svelte';
import { createWorkbenchWarningStore } from './session/warnings.svelte';
import { createWorkbenchCommandSuite } from './session/commands.svelte';
import type {
	FooterHoverInfo,
	NodeWarningRecord,
	SelectionMode,
	WorkbenchToast,
	WorkbenchToastLevel
} from './session/types';
export type {
	FooterHoverInfo,
	NodeWarningRecord,
	SelectionMode,
	WorkbenchToast,
	WorkbenchToastLevel
} from './session/types';
export type {
	WorkbenchLoadingFinishRequest,
	WorkbenchLoadingOperation,
	WorkbenchLoadingState,
	WorkbenchLoadingStateOptions,
	WorkbenchLoadingStateRequest,
	WorkbenchLoadingStep,
	WorkbenchLoadingStepDefinition,
	WorkbenchLoadingStepId,
	WorkbenchLoadingStepStatus,
	WorkbenchLoadingTone
} from './session/loading.svelte';
export type { ProjectGraphTransitionControl } from './session/project-graph-transition.svelte';
export type { WorkbenchUserActionPhase } from './session/action-activity.svelte';
export type { WorkbenchConnectionStatus } from './session/connection-state';

export { appState } from './app-state.svelte';

export interface WorkbenchSessionOptions {
	wsUrl?: string;
	httpBaseUrl?: string;
	scope?: UiSubscriptionScope;
	bootstrapRetryMs?: number;
	transportFactory?: UiTransportFactory;
	enableGlobalShortcuts?: boolean;
}

export interface WorkbenchSession {
	readonly graph: GraphStore;
	readonly client: UiClient;
	readonly loading: WorkbenchLoadingState;
	readonly logRecords: UiLogRecord[];
	readonly logMaxEntries: number;
	readonly logUiUpdateHz: number;
	readonly engineHz: number | null;
	readonly runtimeStats: UiRuntimeStats | null;
	/** Monotonic count of full snapshots applied by this session. */
	readonly snapshotGeneration: number;
	/** Monotonic signal emitted after a project graph transition barrier releases. */
	readonly graphTransitionRevision: number;
	readonly engineLowFrequencyHz: number;
	readonly selectedNodesIds: NodeId[];
	readonly selectedNodeId: NodeId | null;
	readonly status: WorkbenchConnectionStatus;
	readonly toasts: WorkbenchToast[];
	readonly historyBusy: boolean;
	readonly canUndo: boolean;
	readonly canRedo: boolean;
	readonly currentHistoryStateId: number;
	readonly hasActiveEditSession: boolean;
	readonly editSessionEpoch: number;
	readonly userActionPendingCount: number;
	readonly userActionOldestQueuedAtMs: number | null;
	readonly userActionCurrentLabel: string | null;
	readonly userActionCurrentPhase: WorkbenchUserActionPhase | null;
	getNodeData(nodeId: NodeId): UiNodeDto | null;
	getSelectedNodes(): UiNodeDto[];
	getFirstSelectedNode(): UiNodeDto | null;
	getNodeDescription(node: UiNodeDto | null | undefined): string | null;
	getCustomEventSequence(topic: string, origin?: NodeId | null): number;
	getCustomEventPayload<T = unknown>(topic: string, origin?: NodeId | null): T | null;
	getFooterHoverInfo(): FooterHoverInfo | null;
	getNodeVisibleWarnings(nodeId: NodeId): NodeWarningRecord[];
	getActiveWarnings(): NodeWarningRecord[];
	hasNodeWarnings(nodeId: NodeId): boolean;
	isNodeEnabledInHierarchy(nodeId: NodeId): boolean;
	isNodeSelected(nodeId: NodeId): boolean;
	setFooterHover(token: symbol, nodeId: NodeId): void;
	clearFooterHover(token: symbol): void;
	selectNode(nodeId: NodeId | null, selectionMode?: SelectionMode): void;
	selectNodes(nodeIds: NodeId[], selectionMode?: SelectionMode): void;
	clearSelection(): void;
	appendUiLogRecord(
		level: WorkbenchToastLevel,
		tag: string,
		message: string,
		origin?: NodeId
	): void;
	sendIntent(intent: UiEditIntent): Promise<void>;
	sendIntents(intents: UiEditIntent[]): Promise<void>;
	setLogUiUpdateHz(hz: number): void;
	dismissToast(toastId: number): void;
	undo(): Promise<void>;
	redo(): Promise<void>;
	refreshSnapshot(): Promise<boolean>;
	refreshSnapshotAfterCurrentSynchronization(): Promise<boolean>;
	runProjectGraphTransition<T>(
		operation: (control: ProjectGraphTransitionControl) => Promise<T>
	): Promise<T>;
	startLoadingOperation(initial: WorkbenchLoadingStateRequest): WorkbenchLoadingOperation;
	mount(): () => void;
}

const DEFAULT_RETRY_MS = 1000;
const MIN_INITIAL_LOADING_VISIBLE_MS = 650;
const UI_LOG_TAG_INTENT = 'intent';
const UI_LOG_TAG_TRANSPORT = 'transport';
const UI_PERF_SLOW_SNAPSHOT_MS = 100;

const defaultEventTime: EventTime = { tick: 0, micro: 0, seq: 0 };

export const createWorkbenchSession = (options: WorkbenchSessionOptions = {}): WorkbenchSession => {
	const scope = options.scope ?? wholeGraphScope;
	const retryMs = Math.max(100, options.bootstrapRetryMs ?? DEFAULT_RETRY_MS);
	const enableGlobalShortcuts = options.enableGlobalShortcuts ?? true;
	const graph = createGraphStore();
	const shouldLogUiPerf = (() => {
		if (typeof window === 'undefined') {
			return false;
		}
		try {
			return window.localStorage.getItem('gc_ui_perf') === '1';
		} catch {
			return false;
		}
	})();
	const performanceProfilerEnabled = (() => {
		if (typeof window === 'undefined') {
			return false;
		}
		try {
			return window.localStorage.getItem('gc_perf_profiler') === '1';
		} catch {
			return false;
		}
	})();
	setPerformanceProfilerEnabled(performanceProfilerEnabled);
	const logUiPerf = (message: string): void => {
		if (!shouldLogUiPerf) {
			return;
		}
		console.info(`[ui-perf] ${message}`);
	};
	const nowMs = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());

	let status = $state<WorkbenchConnectionStatus>('connecting');
	const loading = createWorkbenchLoadingController(nowMs, MIN_INITIAL_LOADING_VISIBLE_MS);
	const selection = createWorkbenchSelectionStore(graph);
	const warnings = createWorkbenchWarningStore(graph);
	const descriptions = createWorkbenchDescriptionStore();
	const customEvents = createWorkbenchCustomEventStore();
	const footerHover = createWorkbenchFooterHoverStore(graph, {
		getNodeDescription: (node) => descriptions.getNodeDescription(node)
	});
	const history = createWorkbenchHistoryStore();
	const logger = createWorkbenchLoggerStore();
	const userActions = createWorkbenchUserActionCoordinator({
		nowMs,
		afterStateUpdate: () => tick()
	});

	let mountedCleanup: (() => void) | null = null;
	let hasLoadedSnapshot = false;
	let snapshotGeneration = 0;
	let connectionState: UiTransportConnectionState = 'connecting';
	let engineHz = $state<number | null>(null);
	let runtimeStats = $state<UiRuntimeStats | null>(null);
	let intentPipeline: WorkbenchIntentPipeline;
	let snapshots: WorkbenchSnapshotController;
	let projectGraphTransitions: ProjectGraphTransitionController;
	const getNodeDescription = (node: UiNodeDto | null | undefined): string | null =>
		descriptions.getNodeDescription(node);

	const getCustomEventSequence = (topic: string, origin?: NodeId | null): number =>
		customEvents.getCustomEventSequence(topic, origin);

	const getCustomEventPayload = <T = unknown>(topic: string, origin?: NodeId | null): T | null =>
		customEvents.getCustomEventPayload<T>(topic, origin);

	const clearFooterHover = (token: symbol): void => {
		footerHover.clearFooterHover(token);
	};

	const setFooterHover = (token: symbol, nodeId: NodeId): void => {
		footerHover.setFooterHover(token, nodeId);
	};

	const getFooterHoverInfo = (): FooterHoverInfo | null => {
		return footerHover.getFooterHoverInfo();
	};

	const dismissToast = (toastId: number): void => {
		logger.dismissToast(toastId);
	};

	const setLogUiUpdateHz = (hz: number): void => {
		logger.setLogUiUpdateHz(hz);
	};

	const appendUiLogRecord = (
		level: WorkbenchToastLevel,
		tag: string,
		message: string,
		origin?: NodeId
	): void => {
		logger.appendUiLogRecord(level, tag, message, origin);
	};

	const syncConnectionStatus = (): void => {
		status = workbenchConnectionStatus(connectionState, hasLoadedSnapshot);
	};

	const updateLoadingForTransportState = (
		state: UiTransportConnectionState,
		detail?: string
	): void => {
		if (hasLoadedSnapshot) {
			return;
		}
		if (state === 'connected') {
			loading.setState({
				activeStep: 'snapshot',
				title: 'Runtime connected',
				message: 'Requesting the workspace graph',
				detail: detail ?? null,
				progress: 0.34
			});
			return;
		}
		if (state === 'reconnecting') {
			loading.setState(
				{
					activeStep: 'transport',
					title: 'Reconnecting to runtime',
					message: 'Restoring the live transport',
					detail: detail ?? null,
					progress: 0.22,
					tone: 'attention'
				},
				{ preserveProgress: false }
			);
			return;
		}
		if (state === 'disconnected') {
			loading.setState(
				{
					activeStep: 'transport',
					title: 'Runtime unavailable',
					message: 'Waiting before the next attempt',
					detail: detail ?? 'The interface will keep retrying',
					progress: 0.2,
					tone: 'attention'
				},
				{ preserveProgress: false }
			);
			return;
		}
		loading.setState({
			activeStep: 'transport',
			title: 'Connecting to runtime',
			message: 'Opening the live transport',
			detail: detail ?? null,
			progress: 0.18
		});
	};

	const transportOptions: UiTransportOptions = {
		wsUrl: options.wsUrl,
		httpBaseUrl: options.httpBaseUrl,
		onConnectionStateChange: (state, detail) => {
			connectionState = state;
			syncConnectionStatus();
			updateLoadingForTransportState(state, detail);
		},
		onResyncRequired: async (_scope, plane, reason) => {
			const planeDetail = plane ? ` (${plane})` : '';
			appendUiLogRecord(
				'warning',
				UI_LOG_TAG_TRANSPORT,
				`Runtime requested a scoped resync${planeDetail}: ${reason}`
			);
			const boundary = await snapshots.resync('Live runtime state resynchronized.');
			if (!boundary) {
				throw new Error('runtime snapshot resync failed');
			}
			return boundary;
		}
	};
	const client = (options.transportFactory ?? createDefaultUiClient)(transportOptions);

	const getNodeData = (nodeId: NodeId): UiNodeDto | null => {
		return graph.state.nodesById.get(nodeId) ?? null;
	};

	const isNodeEnabledInHierarchy = (nodeId: NodeId): boolean => {
		let current: NodeId | undefined = nodeId;
		while (current !== undefined) {
			const node = graph.state.nodesById.get(current);
			if (!node) {
				return false;
			}
			if (!node.meta.enabled) {
				return false;
			}
			current = graph.state.parentById.get(current);
		}
		return true;
	};

	const applySnapshotToState = (snapshot: UiSnapshot): void => {
		userActions.resetEventBoundary(snapshot.at);
		graph.loadSnapshot(snapshot);
		snapshotGeneration += 1;
		setProjectFileFormat(snapshot.project_file);
		syncProjectFilePathFromSnapshot(snapshot.project_file.current_path);
		descriptions.applySnapshotSchema(snapshot.schema);
		footerHover.prune();
		hasLoadedSnapshot = true;
		warnings.invalidate();
		selection.restorePersistedSelection();
		selection.reconcileSelection();
		history.applyHistoryState(snapshot.history);
		logger.applySnapshotLogger(snapshot.logger);
		syncConnectionStatus();
	};

	const logSnapshotTiming = (context: string, snapshot: UiSnapshot, applyStartedAt: number) => {
		const applyElapsedMs =
			(typeof performance !== 'undefined' ? performance.now() : Date.now()) - applyStartedAt;
		const flushStartedAt = typeof performance !== 'undefined' ? performance.now() : Date.now();
		tick().then(() => {
			const flushElapsedMs =
				(typeof performance !== 'undefined' ? performance.now() : Date.now()) - flushStartedAt;
			const t = (snapshot as any).__timings || {};
			const totalMs = (t.totalMs || 0) + applyElapsedMs + flushElapsedMs;
			const message = `[ui] snapshot ${context} nodes=${snapshot.nodes.length} bytes=${t.bytes || 0} fetch_ms=${(t.fetchMs || 0).toFixed(0)} read_ms=${(t.readMs || 0).toFixed(0)} parse_ms=${(t.parseMs || 0).toFixed(0)} apply_graph_ms=${applyElapsedMs.toFixed(0)} flush_ms=${flushElapsedMs.toFixed(0)} total_ms=${totalMs.toFixed(0)}`;
			logUiPerf(message);
			if (totalMs >= UI_PERF_SLOW_SNAPSHOT_MS) {
				recordPerformanceSample('warning', 'ui.snapshot', message, {
					context,
					nodes: snapshot.nodes.length,
					bytes: t.bytes || 0,
					fetchMs: t.fetchMs || 0,
					readMs: t.readMs || 0,
					parseMs: t.parseMs || 0,
					applyGraphMs: applyElapsedMs,
					flushMs: flushElapsedMs,
					totalMs
				});
			}
		});
	};

	snapshots = createWorkbenchSnapshotController({
		requestSnapshot: () => client.snapshot(scope),
		applySnapshot: applySnapshotToState,
		logSnapshotTiming,
		appendTransportLog: (level, message) => appendUiLogRecord(level, UI_LOG_TAG_TRANSPORT, message),
		suspendIntentDispatch: () => intentPipeline.suspendDispatch(),
		snapshotApplied: () => projectGraphTransitions.snapshotApplied(),
		nowMs
	});

	const applyBatch = (
		batch: UiEventBatch,
		subscriptionControl?: UiSubscriptionBatchControl
	): void => {
		const applyStartedAt = nowMs();
		if (batch.runtime) {
			engineHz = batch.runtime.engine_hz;
			runtimeStats = batch.runtime;
		}
		const graphEvents = logger.partitionBatchEvents(batch.events);
		customEvents.applyBatchEvents(graphEvents);
		let graphPatchMs = 0;

		if (graphEvents.length > 0) {
			const graphPatchStartedAt = nowMs();
			const graphChanged = graph.applyBatch({
				from: batch.from,
				to: graphEvents[graphEvents.length - 1]?.time ?? batch.to,
				events: graphEvents
			});
			graphPatchMs = nowMs() - graphPatchStartedAt;
			if (graphChanged) {
				footerHover.prune();
				if (
					warnings.batchAffectsWarnings({
						from: batch.from,
						to: batch.to,
						events: graphEvents
					})
				) {
					warnings.invalidate();
				}
				selection.reconcileSelection();
				if (graph.state.requiresResync) {
					if (subscriptionControl) {
						subscriptionControl.requestResync('graph_projection_invalidated');
					} else {
						void snapshots.resync();
					}
				}
			}
		}
		const appliedEventTime = batch.to ?? batch.events[batch.events.length - 1]?.time;
		if (appliedEventTime) {
			userActions.observeAppliedEventTime(appliedEventTime);
		}
		const totalMs = nowMs() - applyStartedAt;
		if (batch.events.length > 0 || totalMs >= 4) {
			logUiPerf(
				`[ui] batch events=${batch.events.length} graph_events=${graphEvents.length} graph_patch_ms=${graphPatchMs.toFixed(
					1
				)} ws_batch_apply_ms=${totalMs.toFixed(1)}`
			);
		}
	};

	const runIntent = async (
		intent: UiEditIntent,
		progress?: WorkbenchIntentProgressObserver
	): Promise<void> => {
		let error_logged = false;
		try {
			const ack = await client.sendIntent(intent, progress?.lifecycle);
			history.applyHistoryState(ack.history);
			if (!ack.success) {
				progress?.lifecycle?.('rejected');
				const message = ack.error_message ?? ack.error_code ?? 'unknown error';
				appendUiLogRecord('error', UI_LOG_TAG_INTENT, `Error: ${message}`);
				error_logged = true;
				throw new Error(message);
			}

			if (ack.status === 'staged') {
				appendUiLogRecord('info', UI_LOG_TAG_INTENT, 'Intent accepted and staged.');
				return;
			}

			if (intent.kind === 'setParamControlState') {
				const paramNode = graph.state.nodesById.get(intent.node);
				if (paramNode && paramNode.data.kind === 'parameter') {
					const currentState = paramNode.data.param.control;
					if (!parameterControlStateEquals(currentState, intent.state)) {
						graph.applyEvent({
							time: ack.earliest_event_time ?? graph.state.lastEventTime ?? defaultEventTime,
							kind: {
								kind: 'paramControlChanged',
								param: intent.node,
								old_state: currentState,
								new_state: intent.state
							}
						});
					}
				}
			}

			if (graph.state.requiresResync) {
				await snapshots.resync('Snapshot resynced.');
			}
			progress?.applied?.([ack]);
		} catch (error) {
			const message = error instanceof Error ? error.message : 'unknown error';
			if (!error_logged) {
				appendUiLogRecord('error', UI_LOG_TAG_INTENT, `Error: ${message}`);
			}
			throw error;
		}
	};

	const runIntentBatch = async (
		intents: UiEditIntent[],
		progress?: WorkbenchIntentProgressObserver
	): Promise<void> => {
		if (intents.length === 0) {
			return;
		}
		if (intents.length === 1) {
			await runIntent(intents[0], progress);
			return;
		}
		let error_logged = false;
		try {
			const acks = await client.sendIntents(intents, progress?.lifecycle);
			if (acks.length !== intents.length) {
				throw new Error(
					`intent batch acknowledgement count mismatch (${acks.length}/${intents.length})`
				);
			}
			let firstFailure: UiAck | null = null;
			for (const ack of acks) {
				history.applyHistoryState(ack.history);
				if (!ack.success && firstFailure === null) {
					firstFailure = ack;
				}
			}
			if (firstFailure) {
				progress?.lifecycle?.('rejected');
				const message = firstFailure.error_message ?? firstFailure.error_code ?? 'unknown error';
				appendUiLogRecord('error', UI_LOG_TAG_INTENT, `Error: ${message}`);
				error_logged = true;
				throw new Error(message);
			}
			if (graph.state.requiresResync) {
				await snapshots.resync('Snapshot resynced.');
			}
			progress?.applied?.(acks);
		} catch (error) {
			const message = error instanceof Error ? error.message : 'unknown error';
			if (!error_logged) {
				appendUiLogRecord('error', UI_LOG_TAG_INTENT, `Error: ${message}`);
			}
			throw error;
		}
	};

	intentPipeline = createWorkbenchIntentPipeline({
		getSnapshotGeneration: () => snapshotGeneration,
		dispatchIntent: runIntent,
		dispatchIntents: runIntentBatch
	});

	projectGraphTransitions = createProjectGraphTransitionController({
		runIntentBarrier: (operation) => intentPipeline.runProjectGraphTransition(operation),
		blockIntentDispatch: (reason) => intentPipeline.blockDispatch(reason),
		rejectPendingIntents: (reason) => intentPipeline.rejectPending(reason),
		waitForSnapshotIdle: () => snapshots.waitForIdle(),
		afterTransitionOperation: () => tick()
	});

	const sendIntent = (intent: UiEditIntent): Promise<void> => {
		const label = userActionLabelForIntent(intent);
		if (!label) {
			return intentPipeline.sendIntent(intent);
		}
		return userActions.run(label, (progress) => intentPipeline.sendIntent(intent, progress));
	};

	const sendIntents = (intents: UiEditIntent[]): Promise<void> => {
		const label = userActionLabelForIntents(intents);
		if (!label) {
			return intentPipeline.sendIntents(intents);
		}
		return userActions.run(label, (progress) => intentPipeline.sendIntents(intents, progress));
	};

	const runProjectGraphTransition = <T>(
		operation: (control: ProjectGraphTransitionControl) => Promise<T>
	): Promise<T> => projectGraphTransitions.run(operation);

	const undo = (): Promise<void> => {
		return history.undo(() => sendIntent({ kind: 'undo' }));
	};

	const redo = (): Promise<void> => {
		return history.redo(() => sendIntent({ kind: 'redo' }));
	};

	const revealNodeInInspector = (nodeId: NodeId): boolean => {
		if (!graph.state.nodesById.has(nodeId)) {
			return false;
		}
		selection.selectNode(nodeId, 'REPLACE');
		appState.panels
			?.showPanel({
				panelType: 'inspector',
				panelId: 'inspector'
			})
			?.setActive();
		return true;
	};

	const commandSuite = createWorkbenchCommandSuite({
		graph,
		sendIntent,
		getSelectedNodes: () => selection.getSelectedNodes(),
		getFirstSelectedNode: () => selection.getFirstSelectedNode(),
		getSelectedNodeIds: () => selection.selectedNodeIds,
		selectNodes: (nodeIds, selectionMode) => selection.selectNodes(nodeIds, selectionMode),
		revealNodeInInspector,
		undo,
		redo
	});

	const mount = (): (() => void) => {
		if (mountedCleanup !== null) {
			return mountedCleanup;
		}
		if (!hasLoadedSnapshot) {
			loading.resetInitial();
		}

		let stopped = false;
		let subscribed = false;
		let unsubscribe = () => {};
		let unregisterCommands = () => {};
		let bootstrapInFlight = false;
		let retryTimer: ReturnType<typeof setTimeout> | null = null;
		let retryDelayMs = retryMs;

		const clearRetry = (): void => {
			if (retryTimer !== null) {
				clearTimeout(retryTimer);
				retryTimer = null;
			}
		};

		const scheduleRetry = (): void => {
			if (stopped || subscribed || retryTimer !== null) {
				return;
			}
			const delay = retryDelayMs;
			retryDelayMs = Math.min(10000, Math.max(retryMs, retryDelayMs * 2));
			if (!hasLoadedSnapshot) {
				loading.setState(
					{
						activeStep: 'transport',
						title: 'Runtime unavailable',
						message: 'Retrying connection',
						detail: `Next attempt in ${formatReconnectDelay(delay)}`,
						progress: 0.22,
						tone: 'attention'
					},
					{ preserveProgress: false }
				);
			}
			retryTimer = setTimeout(() => {
				retryTimer = null;
				void bootstrap();
			}, delay);
		};

		const bootstrap = async (): Promise<void> => {
			if (stopped || subscribed || bootstrapInFlight) {
				return;
			}

			bootstrapInFlight = true;
			const endSnapshotSynchronization = snapshots.beginSynchronization();
			try {
				loading.setState({
					activeStep: 'snapshot',
					title: 'Loading workspace graph',
					message: 'Requesting the initial snapshot',
					detail: null,
					progress: 0.38
				});
				const fetchStartedAt = typeof performance !== 'undefined' ? performance.now() : Date.now();
				const snapshot = await snapshots.request();
				const fetchElapsedMs =
					(typeof performance !== 'undefined' ? performance.now() : Date.now()) - fetchStartedAt;
				if (stopped) {
					return;
				}
				loading.setState({
					activeStep: 'apply',
					title: 'Building workspace',
					message: 'Applying the project graph',
					detail: `${snapshot.nodes.length} nodes loaded`,
					progress: 0.72
				});
				const applyStartedAt = typeof performance !== 'undefined' ? performance.now() : Date.now();
				applySnapshotToState(snapshot);
				projectGraphTransitions.snapshotApplied();
				const applyElapsedMs =
					(typeof performance !== 'undefined' ? performance.now() : Date.now()) - applyStartedAt;
				logUiPerf(
					`bootstrap snapshot fetch_ms=${fetchElapsedMs.toFixed(1)} apply_ms=${applyElapsedMs.toFixed(
						1
					)} nodes=${snapshot.nodes.length}`
				);
				loading.setState({
					activeStep: 'subscribe',
					title: 'Starting live updates',
					message: 'Subscribing to runtime events',
					detail: null,
					progress: 0.9
				});
				unsubscribe =
					client.subscribeInterest?.(
						'workbench',
						scope,
						['structure', 'value', 'trigger', 'observation', 'catalog', 'preview'],
						snapshot.at,
						applyBatch,
						{ createEventWork: (event) => graph.createEventWork(event) }
					) ??
					client.subscribe(scope, snapshot.at, applyBatch, {
						createEventWork: (event) => graph.createEventWork(event)
					});
				subscribed = true;
				clearRetry();
				retryDelayMs = retryMs;
				loading.completeInitial();
			} catch (error) {
				if (stopped) {
					return;
				}
				const message = error instanceof Error ? error.message : 'unknown connection error';
				connectionState = 'disconnected';
				syncConnectionStatus();
				loading.setState(
					{
						activeStep: 'transport',
						title: 'Runtime unavailable',
						message: 'Connection failed. Retrying',
						detail: message,
						progress: 0.22,
						tone: 'attention'
					},
					{ preserveProgress: false }
				);
				appendUiLogRecord(
					'error',
					UI_LOG_TAG_TRANSPORT,
					`Connection failed: ${message} (retrying...)`
				);
				scheduleRetry();
			} finally {
				bootstrapInFlight = false;
				endSnapshotSynchronization();
			}
		};

		unregisterCommands = commandSuite.registerCommandHandlers();
		const unregisterGlobalShortcuts = registerGlobalCommandShortcuts(enableGlobalShortcuts);
		void bootstrap();

		mountedCleanup = (): void => {
			stopped = true;
			clearRetry();
			loading.clearHideTimer();
			intentPipeline.rejectPending(new Error('workbench session unmounted'));
			unregisterGlobalShortcuts();
			unregisterCommands();
			unsubscribe();
			graph.reset();
			warnings.reset();
			descriptions.reset();
			customEvents.reset();
			userActions.resetEventBoundary(null);
			footerHover.reset();
			history.reset();
			selection.reset();
			commandSuite.reset();
			logger.reset();
			userActions.reset();
			resetProjectFileFormat();
			hasLoadedSnapshot = false;
			connectionState = 'connecting';
			engineHz = null;
			runtimeStats = null;
			syncConnectionStatus();
			loading.resetInitial();
			mountedCleanup = null;
		};

		return mountedCleanup;
	};

	return {
		get graph(): GraphStore {
			return graph;
		},
		get client(): UiClient {
			return client;
		},
		get loading(): WorkbenchLoadingState {
			return loading.state;
		},
		get logRecords(): UiLogRecord[] {
			return logger.logRecords;
		},
		get logMaxEntries(): number {
			return logger.logMaxEntries;
		},
		get logUiUpdateHz(): number {
			return logger.logUiUpdateHz;
		},
		get engineHz(): number | null {
			return engineHz;
		},
		get runtimeStats(): UiRuntimeStats | null {
			return runtimeStats;
		},
		get snapshotGeneration(): number {
			return snapshotGeneration;
		},
		get graphTransitionRevision(): number {
			return projectGraphTransitions.revision;
		},
		get engineLowFrequencyHz(): number {
			return engineLowFrequencyFromGraph(graph);
		},
		get selectedNodesIds(): NodeId[] {
			return selection.selectedNodeIds;
		},
		get selectedNodeId(): NodeId | null {
			return selection.selectedNodeId;
		},
		get status(): WorkbenchConnectionStatus {
			return status;
		},
		get toasts(): WorkbenchToast[] {
			return logger.toasts;
		},
		get historyBusy(): boolean {
			return history.historyBusy;
		},
		get canUndo(): boolean {
			return history.canUndo;
		},
		get canRedo(): boolean {
			return history.canRedo;
		},
		get currentHistoryStateId(): number {
			return history.currentHistoryStateId;
		},
		get hasActiveEditSession(): boolean {
			return history.hasActiveEditSession;
		},
		get editSessionEpoch(): number {
			return history.editSessionEpoch;
		},
		get userActionPendingCount(): number {
			return userActions.pendingCount;
		},
		get userActionOldestQueuedAtMs(): number | null {
			return userActions.oldestQueuedAtMs;
		},
		get userActionCurrentLabel(): string | null {
			return userActions.currentLabel;
		},
		get userActionCurrentPhase(): WorkbenchUserActionPhase | null {
			return userActions.currentPhase;
		},
		getNodeData,
		getSelectedNodes: () => selection.getSelectedNodes(),
		getFirstSelectedNode: () => selection.getFirstSelectedNode(),
		getNodeDescription,
		getCustomEventSequence,
		getCustomEventPayload,
		getFooterHoverInfo,
		getNodeVisibleWarnings: (nodeId) => warnings.getNodeVisibleWarnings(nodeId),
		getActiveWarnings: () => warnings.getActiveWarnings(),
		hasNodeWarnings: (nodeId) => warnings.hasNodeWarnings(nodeId),
		isNodeEnabledInHierarchy,
		isNodeSelected: (nodeId) => selection.isNodeSelected(nodeId),
		setFooterHover,
		clearFooterHover,
		selectNode: (nodeId, selectionMode) => selection.selectNode(nodeId, selectionMode),
		selectNodes: (nodeIds, selectionMode) => selection.selectNodes(nodeIds, selectionMode),
		clearSelection: () => selection.clearSelection(),
		appendUiLogRecord,
		sendIntent,
		sendIntents,
		setLogUiUpdateHz,
		dismissToast,
		undo,
		redo,
		refreshSnapshot: () => snapshots.refresh(),
		refreshSnapshotAfterCurrentSynchronization: async () => {
			const success = await snapshots.refreshAfterCurrent();
			projectGraphTransitions.requiredSnapshotFinished(success);
			return success;
		},
		runProjectGraphTransition,
		startLoadingOperation: (initial) => loading.startOperation(initial),
		mount
	};
};
