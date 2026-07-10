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
	ParamValue,
	UiNodeDto,
	UiClient,
	UiEditIntent,
	UiEventBatch,
	UiLogRecord,
	UiAck,
	UiSnapshot,
	UiSubscriptionScope,
	UiParameterControlState
} from '../types';
import { wholeGraphScope } from '../types';
import { handleCommandShortcut } from './commands.svelte';
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

export { appState } from './app-state.svelte';

export interface WorkbenchSessionOptions {
	wsUrl?: string;
	httpBaseUrl?: string;
	pollIntervalMs?: number;
	scope?: UiSubscriptionScope;
	bootstrapRetryMs?: number;
	transportFactory?: UiTransportFactory;
	enableGlobalShortcuts?: boolean;
}

export type WorkbenchConnectionStatus = 'disconnected' | 'connecting' | 'connected';
export type WorkbenchLoadingTone = 'busy' | 'attention' | 'ready';
export type WorkbenchLoadingStepStatus = 'pending' | 'active' | 'complete' | 'attention';

export type WorkbenchLoadingStepId = string;

export interface WorkbenchLoadingStepDefinition {
	id: WorkbenchLoadingStepId;
	label: string;
}

const WORKBENCH_LOADING_STEP_DEFINITIONS: readonly WorkbenchLoadingStepDefinition[] = [
	{ id: 'session', label: 'Prepare interface' },
	{ id: 'transport', label: 'Connect runtime' },
	{ id: 'snapshot', label: 'Load graph' },
	{ id: 'apply', label: 'Build workspace' },
	{ id: 'subscribe', label: 'Start live updates' },
	{ id: 'ready', label: 'Ready' }
] as const;

export interface WorkbenchLoadingStep {
	id: WorkbenchLoadingStepId;
	label: string;
	status: WorkbenchLoadingStepStatus;
}

export interface WorkbenchLoadingState {
	visible: boolean;
	title: string;
	message: string;
	detail: string | null;
	progress: number;
	tone: WorkbenchLoadingTone;
	steps: WorkbenchLoadingStep[];
}

export interface WorkbenchLoadingStateRequest {
	activeStep: WorkbenchLoadingStepId;
	title: string;
	message: string;
	detail?: string | null;
	progress: number;
	visible?: boolean;
	tone?: WorkbenchLoadingTone;
	stepDefinitions?: readonly WorkbenchLoadingStepDefinition[];
}

export interface WorkbenchLoadingStateOptions {
	preserveProgress?: boolean;
}

export interface WorkbenchLoadingFinishRequest {
	activeStep?: WorkbenchLoadingStepId;
	title?: string;
	message?: string;
	detail?: string | null;
	progress?: number;
	tone?: WorkbenchLoadingTone;
}

export interface WorkbenchLoadingOperation {
	update(next: WorkbenchLoadingStateRequest, options?: WorkbenchLoadingStateOptions): void;
	finish(next?: WorkbenchLoadingFinishRequest): void;
	fail(next: WorkbenchLoadingFinishRequest): void;
	cancel(): void;
}

export interface WorkbenchSession {
	readonly graph: GraphStore;
	readonly client: UiClient;
	readonly httpBaseUrl: string;
	readonly loading: WorkbenchLoadingState;
	readonly logRecords: UiLogRecord[];
	readonly logMaxEntries: number;
	readonly logUiUpdateHz: number;
	readonly engineHz: number | null;
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
	startLoadingOperation(initial: WorkbenchLoadingStateRequest): WorkbenchLoadingOperation;
	mount(): () => void;
}

interface IntentWaiter {
	resolve: () => void;
	reject: (reason?: unknown) => void;
}

interface QueuedIntent {
	intent: UiEditIntent;
	waiters: IntentWaiter[];
}

const DEFAULT_RETRY_MS = 1000;
const MIN_INITIAL_LOADING_VISIBLE_MS = 650;
const UI_LOG_TAG_INTENT = 'intent';
const UI_LOG_TAG_TRANSPORT = 'transport';
const UI_PERF_SLOW_SNAPSHOT_MS = 100;
const PREFERENCES_DECL_ID = 'preferences';
const PREFERENCES_ENGINE_DECL_ID = 'engine';
const PREFERENCES_ENGINE_LOW_FREQUENCY_DECL_ID = 'engine_low_frequency';
const DEFAULT_ENGINE_LOW_FREQUENCY_HZ = 60;

const defaultEventTime: EventTime = { tick: 0, micro: 0, seq: 0 };

const childByDeclId = (
	graph: GraphStore,
	parent: UiNodeDto | null | undefined,
	declId: string
): UiNodeDto | null => {
	if (!parent) {
		return null;
	}
	for (const childId of parent.children) {
		const child = graph.state.nodesById.get(childId);
		if (child?.decl_id === declId) {
			return child;
		}
	}
	return null;
};

const paramFrequencyValue = (value: ParamValue, fallback: number): number => {
	let frequency: number | null = null;
	if (value.kind === 'int' || value.kind === 'float') {
		frequency = Math.round(value.value);
	} else if (value.kind === 'str') {
		frequency = Number.parseInt(value.value.trim(), 10);
	}
	return frequency !== null && Number.isFinite(frequency) && frequency > 0 ? frequency : fallback;
};

const engineLowFrequencyFromGraph = (graph: GraphStore): number => {
	const rootId = graph.state.rootId;
	if (rootId === null) {
		return DEFAULT_ENGINE_LOW_FREQUENCY_HZ;
	}
	const root = graph.state.nodesById.get(rootId);
	const preferences = childByDeclId(graph, root, PREFERENCES_DECL_ID);
	const engine = childByDeclId(graph, preferences, PREFERENCES_ENGINE_DECL_ID);
	const lowFrequency = childByDeclId(graph, engine, PREFERENCES_ENGINE_LOW_FREQUENCY_DECL_ID);
	if (!lowFrequency || lowFrequency.data.kind !== 'parameter') {
		return DEFAULT_ENGINE_LOW_FREQUENCY_HZ;
	}
	return paramFrequencyValue(lowFrequency.data.param.value, DEFAULT_ENGINE_LOW_FREQUENCY_HZ);
};

const toConnectionStatus = (
	transportState: UiTransportConnectionState,
	hasLoadedSnapshot: boolean
): WorkbenchConnectionStatus => {
	if (transportState === 'connected') {
		return hasLoadedSnapshot ? 'connected' : 'connecting';
	}
	if (
		transportState === 'connecting' ||
		transportState === 'reconnecting' ||
		transportState === 'fallbackPolling'
	) {
		return 'connecting';
	}
	return 'disconnected';
};

const clampProgress = (value: number): number => Math.min(1, Math.max(0, value));

const loadingStepIndex = (
	stepDefinitions: readonly WorkbenchLoadingStepDefinition[],
	stepId: WorkbenchLoadingStepId
): number => stepDefinitions.findIndex((step) => step.id === stepId);

const lastLoadingStepId = (
	stepDefinitions: readonly WorkbenchLoadingStepDefinition[]
): WorkbenchLoadingStepId => stepDefinitions[stepDefinitions.length - 1]?.id ?? 'ready';

const createWorkbenchLoadingState = ({
	activeStep,
	title,
	message,
	detail = null,
	progress,
	visible = true,
	tone = 'busy',
	stepDefinitions = WORKBENCH_LOADING_STEP_DEFINITIONS
}: WorkbenchLoadingStateRequest): WorkbenchLoadingState => {
	const activeIndex = loadingStepIndex(stepDefinitions, activeStep);
	return {
		visible,
		title,
		message,
		detail,
		progress: clampProgress(progress),
		tone,
		steps: stepDefinitions.map((step, index) => {
			let status: WorkbenchLoadingStepStatus = 'pending';
			if (index < activeIndex) {
				status = 'complete';
			} else if (index === activeIndex) {
				status = tone === 'attention' ? 'attention' : 'active';
			}
			return {
				id: step.id,
				label: step.label,
				status
			};
		})
	};
};

const controlStateEquals = (
	left: UiParameterControlState,
	right: UiParameterControlState
): boolean => {
	if (left.mode !== right.mode) {
		return false;
	}
	return JSON.stringify(left.spec) === JSON.stringify(right.spec);
};

const isEditableTarget = (target: EventTarget | null): boolean => {
	if (!(target instanceof HTMLElement)) {
		return false;
	}
	if (target.isContentEditable) {
		return true;
	}
	const tagName = target.tagName;
	return tagName === 'INPUT' || tagName === 'TEXTAREA' || tagName === 'SELECT';
};

export const createWorkbenchSession = (options: WorkbenchSessionOptions = {}): WorkbenchSession => {
	const scope = options.scope ?? wholeGraphScope;
	const httpBaseUrl = (options.httpBaseUrl ?? 'http://localhost:7010/api/ui').replace(/\/+$/, '');
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
	let initialLoadingStartedAt = nowMs();
	let loadingHideTimer: ReturnType<typeof setTimeout> | null = null;
	let nextLoadingOperationId = 0;
	let activeLoadingOperationId: number | null = null;
	let loading = $state<WorkbenchLoadingState>(
		createWorkbenchLoadingState({
			activeStep: 'session',
			title: 'Opening workspace',
			message: 'Preparing the interface',
			detail: null,
			progress: 0.06
		})
	);
	const selection = createWorkbenchSelectionStore(graph);
	const warnings = createWorkbenchWarningStore(graph);
	const descriptions = createWorkbenchDescriptionStore();
	const customEvents = createWorkbenchCustomEventStore();
	const footerHover = createWorkbenchFooterHoverStore(graph, {
		getNodeDescription: (node) => descriptions.getNodeDescription(node)
	});
	const history = createWorkbenchHistoryStore();
	const logger = createWorkbenchLoggerStore();

	let mountedCleanup: (() => void) | null = null;
	let resyncInFlight = false;
	let resyncQueued = false;
	let snapshotRequestInFlight: Promise<UiSnapshot> | null = null;
	let hasLoadedSnapshot = false;
	let connectionState: UiTransportConnectionState = 'connecting';
	let engineHz = $state<number | null>(null);
	const pendingIntentQueue: QueuedIntent[] = [];
	let intentQueueProcessing = false;
	// Keep batch payloads bounded while still reducing setParam fan-out.
	const MAX_SET_PARAM_BATCH_SIZE = 512;

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

	const clearLoadingHideTimer = (): void => {
		if (loadingHideTimer === null) {
			return;
		}
		clearTimeout(loadingHideTimer);
		loadingHideTimer = null;
	};

	const setLoadingState = (
		next: WorkbenchLoadingStateRequest,
		options: WorkbenchLoadingStateOptions = {}
	): void => {
		clearLoadingHideTimer();
		const progress =
			options.preserveProgress === false
				? next.progress
				: Math.max(loading.progress, next.progress);
		loading = createWorkbenchLoadingState({
			...next,
			progress
		});
	};

	const scheduleLoadingHide = (startedAt: number, minimumVisibleMs: number): void => {
		const visibleElapsedMs = nowMs() - startedAt;
		const delayMs = Math.max(120, minimumVisibleMs - visibleElapsedMs);
		loadingHideTimer = setTimeout(() => {
			loadingHideTimer = null;
			loading = {
				...loading,
				visible: false
			};
		}, delayMs);
	};

	const completeInitialLoading = (): void => {
		clearLoadingHideTimer();
		loading = createWorkbenchLoadingState({
			activeStep: 'ready',
			title: 'Workspace ready',
			message: 'Live updates are running',
			detail: null,
			progress: 1,
			tone: 'ready'
		});
		scheduleLoadingHide(initialLoadingStartedAt, MIN_INITIAL_LOADING_VISIBLE_MS);
	};

	const resetInitialLoading = (): void => {
		clearLoadingHideTimer();
		activeLoadingOperationId = null;
		initialLoadingStartedAt = nowMs();
		loading = createWorkbenchLoadingState({
			activeStep: 'session',
			title: 'Opening workspace',
			message: 'Preparing the interface',
			detail: null,
			progress: 0.06
		});
	};

	const startLoadingOperation = (
		initial: WorkbenchLoadingStateRequest
	): WorkbenchLoadingOperation => {
		nextLoadingOperationId += 1;
		const operationId = nextLoadingOperationId;
		activeLoadingOperationId = operationId;
		const startedAt = nowMs();
		let stepDefinitions = initial.stepDefinitions ?? WORKBENCH_LOADING_STEP_DEFINITIONS;

		const isActiveOperation = (): boolean => activeLoadingOperationId === operationId;
		const withOperationSteps = (
			next: WorkbenchLoadingStateRequest
		): WorkbenchLoadingStateRequest => {
			stepDefinitions = next.stepDefinitions ?? stepDefinitions;
			return {
				...next,
				stepDefinitions
			};
		};

		setLoadingState(withOperationSteps(initial), { preserveProgress: false });

		return {
			update: (
				next: WorkbenchLoadingStateRequest,
				options: WorkbenchLoadingStateOptions = {}
			): void => {
				if (!isActiveOperation()) {
					return;
				}
				setLoadingState(withOperationSteps(next), options);
			},
			finish: (next: WorkbenchLoadingFinishRequest = {}): void => {
				if (!isActiveOperation()) {
					return;
				}
				activeLoadingOperationId = null;
				clearLoadingHideTimer();
				loading = createWorkbenchLoadingState({
					stepDefinitions,
					activeStep: next.activeStep ?? lastLoadingStepId(stepDefinitions),
					title: next.title ?? 'Done',
					message: next.message ?? 'Operation complete',
					detail: next.detail ?? null,
					progress: next.progress ?? 1,
					tone: next.tone ?? 'ready'
				});
				scheduleLoadingHide(startedAt, MIN_INITIAL_LOADING_VISIBLE_MS);
			},
			fail: (next: WorkbenchLoadingFinishRequest): void => {
				if (!isActiveOperation()) {
					return;
				}
				activeLoadingOperationId = null;
				clearLoadingHideTimer();
				loading = createWorkbenchLoadingState({
					stepDefinitions,
					activeStep: next.activeStep ?? lastLoadingStepId(stepDefinitions),
					title: next.title ?? 'Operation failed',
					message: next.message ?? 'The requested operation could not finish',
					detail: next.detail ?? null,
					progress: next.progress ?? loading.progress,
					tone: next.tone ?? 'attention'
				});
				scheduleLoadingHide(startedAt, 1800);
			},
			cancel: (): void => {
				if (!isActiveOperation()) {
					return;
				}
				activeLoadingOperationId = null;
				clearLoadingHideTimer();
				loading = {
					...loading,
					visible: false
				};
			}
		};
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

	const applyHistoryState = (nextHistory: UiSnapshot['history'] | undefined): void => {
		history.applyHistoryState(nextHistory);
	};

	const syncConnectionStatus = (): void => {
		status = toConnectionStatus(connectionState, hasLoadedSnapshot);
	};

	const formatRetryDelay = (delayMs: number): string => {
		const seconds = Math.max(1, Math.ceil(delayMs / 1000));
		return `${seconds}s`;
	};

	const updateLoadingForTransportState = (
		state: UiTransportConnectionState,
		detail?: string
	): void => {
		if (hasLoadedSnapshot) {
			return;
		}
		if (state === 'connected') {
			setLoadingState({
				activeStep: 'snapshot',
				title: 'Runtime connected',
				message: 'Requesting the workspace graph',
				detail: detail ?? null,
				progress: 0.34
			});
			return;
		}
		if (state === 'fallbackPolling') {
			setLoadingState({
				activeStep: 'snapshot',
				title: 'Using fallback transport',
				message: 'Loading the graph over HTTP',
				detail: detail ?? 'Live updates will reconnect in the background',
				progress: 0.32,
				tone: 'attention'
			});
			return;
		}
		if (state === 'reconnecting') {
			setLoadingState(
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
			setLoadingState(
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
		setLoadingState({
			activeStep: 'transport',
			title: 'Connecting to runtime',
			message: 'Opening the live transport',
			detail: detail ?? null,
			progress: 0.18
		});
	};

	const transportOptions: UiTransportOptions = {
		wsUrl: options.wsUrl,
		httpBaseUrl,
		pollIntervalMs: options.pollIntervalMs,
		onConnectionStateChange: (state, detail) => {
			connectionState = state;
			syncConnectionStatus();
			updateLoadingForTransportState(state, detail);
			if (state === 'fallbackPolling') {
				appendUiLogRecord(
					'warning',
					UI_LOG_TAG_TRANSPORT,
					'WebSocket lost. Using HTTP fallback while reconnecting...'
				);
			}
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
		graph.loadSnapshot(snapshot);
		setProjectFileFormat(snapshot.project_file);
		syncProjectFilePathFromSnapshot(snapshot.project_file.current_path);
		descriptions.applySnapshotSchema(snapshot.schema);
		footerHover.prune();
		hasLoadedSnapshot = true;
		warnings.invalidate();
		selection.restorePersistedSelection();
		selection.reconcileSelection();
		applyHistoryState(snapshot.history);
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

	const requestSnapshot = (): Promise<UiSnapshot> => {
		if (snapshotRequestInFlight) {
			return snapshotRequestInFlight;
		}
		const inFlight = client.snapshot(scope);
		snapshotRequestInFlight = inFlight.finally(() => {
			snapshotRequestInFlight = null;
		});
		return snapshotRequestInFlight;
	};

	const refreshSnapshot = async (): Promise<boolean> => {
		try {
			const snapshot = await requestSnapshot();
			const applyStartedAt = typeof performance !== 'undefined' ? performance.now() : Date.now();
			applySnapshotToState(snapshot);
			logSnapshotTiming('refresh', snapshot, applyStartedAt);
			return true;
		} catch (error) {
			const message = error instanceof Error ? error.message : 'unknown refresh error';
			appendUiLogRecord('error', UI_LOG_TAG_TRANSPORT, `Snapshot refresh failed: ${message}`);
			return false;
		}
	};

	const resyncSnapshot = async (successMessage?: string): Promise<void> => {
		if (resyncInFlight) {
			resyncQueued = true;
			return;
		}
		resyncInFlight = true;
		resyncQueued = false;
		try {
			const snapshot = await requestSnapshot();
			const applyStartedAt = typeof performance !== 'undefined' ? performance.now() : Date.now();
			applySnapshotToState(snapshot);
			if (successMessage) {
				appendUiLogRecord('info', UI_LOG_TAG_TRANSPORT, successMessage);
			}
			logSnapshotTiming('resync', snapshot, applyStartedAt);
		} catch (error) {
			const message = error instanceof Error ? error.message : 'unknown resync error';
			appendUiLogRecord('error', UI_LOG_TAG_TRANSPORT, `Resync failed: ${message}`);
		} finally {
			resyncInFlight = false;
			if (resyncQueued) {
				resyncQueued = false;
				void resyncSnapshot(successMessage);
			}
		}
	};

	const applyBatch = (batch: UiEventBatch): void => {
		const applyStartedAt = nowMs();
		if (batch.runtime) {
			engineHz = batch.runtime.engine_hz;
		}
		const graphEvents = logger.partitionBatchEvents(batch.events);
		customEvents.applyBatchEvents(graphEvents);
		let graphPatchMs = 0;

		if (graphEvents.length > 0) {
			const warningDataChanged = warnings.batchAffectsWarnings({
				from: batch.from,
				to: batch.to,
				events: graphEvents
			});
			const graphPatchStartedAt = nowMs();
			graph.applyBatch({
				from: batch.from,
				to: graphEvents[graphEvents.length - 1]?.time ?? batch.to,
				events: graphEvents
			});
			graphPatchMs = nowMs() - graphPatchStartedAt;
			footerHover.prune();
			if (warningDataChanged) {
				warnings.invalidate();
			}
			selection.reconcileSelection();
			if (graph.state.requiresResync) {
				void resyncSnapshot();
			}
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

	const runIntent = async (intent: UiEditIntent): Promise<void> => {
		let error_logged = false;
		try {
			const ack = await client.sendIntent(intent);
			applyHistoryState(ack.history);
			if (!ack.success) {
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
					if (!controlStateEquals(currentState, intent.state)) {
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
				await resyncSnapshot('Snapshot resynced.');
			}
		} catch (error) {
			const message = error instanceof Error ? error.message : 'unknown error';
			if (!error_logged) {
				appendUiLogRecord('error', UI_LOG_TAG_INTENT, `Error: ${message}`);
			}
			throw error;
		}
	};

	const runIntentBatch = async (intents: UiEditIntent[]): Promise<void> => {
		if (intents.length === 0) {
			return;
		}
		if (intents.length === 1) {
			await runIntent(intents[0]);
			return;
		}
		let error_logged = false;
		try {
			const acks = await client.sendIntents(intents);
			if (acks.length !== intents.length) {
				throw new Error(
					`intent batch acknowledgement count mismatch (${acks.length}/${intents.length})`
				);
			}
			let firstFailure: UiAck | null = null;
			for (const ack of acks) {
				applyHistoryState(ack.history);
				if (!ack.success && firstFailure === null) {
					firstFailure = ack;
				}
			}
			if (firstFailure) {
				const message = firstFailure.error_message ?? firstFailure.error_code ?? 'unknown error';
				appendUiLogRecord('error', UI_LOG_TAG_INTENT, `Error: ${message}`);
				error_logged = true;
				throw new Error(message);
			}
			if (graph.state.requiresResync) {
				await resyncSnapshot('Snapshot resynced.');
			}
		} catch (error) {
			const message = error instanceof Error ? error.message : 'unknown error';
			if (!error_logged) {
				appendUiLogRecord('error', UI_LOG_TAG_INTENT, `Error: ${message}`);
			}
			throw error;
		}
	};

	const findQueuedSetParamIndex = (node: NodeId): number => {
		for (let index = pendingIntentQueue.length - 1; index >= 0; index -= 1) {
			const queued = pendingIntentQueue[index];
			if (queued?.intent.kind === 'setParam' && queued.intent.node === node) {
				return index;
			}
		}
		return -1;
	};

	const drainLeadingSetParamIntents = (): QueuedIntent[] => {
		const drained: QueuedIntent[] = [];
		while (drained.length < MAX_SET_PARAM_BATCH_SIZE && pendingIntentQueue.length > 0) {
			const next = pendingIntentQueue[0];
			if (!next || next.intent.kind !== 'setParam') {
				break;
			}
			const queued = pendingIntentQueue.shift();
			if (queued) {
				drained.push(queued);
			}
		}
		return drained;
	};

	const resolveQueuedIntents = (queuedIntents: QueuedIntent[]): void => {
		for (const queued of queuedIntents) {
			for (const waiter of queued.waiters) {
				waiter.resolve();
			}
		}
	};

	const rejectQueuedIntentsBatch = (queuedIntents: QueuedIntent[], error: unknown): void => {
		for (const queued of queuedIntents) {
			for (const waiter of queued.waiters) {
				waiter.reject(error);
			}
		}
	};

	const processIntentQueue = async (): Promise<void> => {
		if (intentQueueProcessing) {
			return;
		}
		intentQueueProcessing = true;
		try {
			while (pendingIntentQueue.length > 0) {
				const next = pendingIntentQueue[0];
				if (next?.intent.kind === 'setParam') {
					const queued_batch = drainLeadingSetParamIntents();
					if (queued_batch.length === 0) {
						continue;
					}
					try {
						await runIntentBatch(queued_batch.map((entry) => entry.intent));
						resolveQueuedIntents(queued_batch);
					} catch (error) {
						rejectQueuedIntentsBatch(queued_batch, error);
					}
					continue;
				}

				const queued = pendingIntentQueue.shift();
				if (!queued) {
					continue;
				}
				try {
					await runIntent(queued.intent);
					resolveQueuedIntents([queued]);
				} catch (error) {
					rejectQueuedIntentsBatch([queued], error);
				}
			}
		} finally {
			intentQueueProcessing = false;
			if (pendingIntentQueue.length > 0) {
				void processIntentQueue();
			}
		}
	};

	const rejectQueuedIntents = (reason: Error): void => {
		while (pendingIntentQueue.length > 0) {
			const queued = pendingIntentQueue.shift();
			if (!queued) {
				continue;
			}
			for (const waiter of queued.waiters) {
				waiter.reject(reason);
			}
		}
	};

	const sendIntent = (intent: UiEditIntent): Promise<void> => {
		return new Promise<void>((resolve, reject) => {
			if (intent.kind === 'setParam') {
				const queuedSetParamIndex = findQueuedSetParamIndex(intent.node);
				if (queuedSetParamIndex >= 0) {
					const queued = pendingIntentQueue[queuedSetParamIndex];
					if (queued) {
						queued.intent = intent;
						queued.waiters.push({ resolve, reject });
						void processIntentQueue();
						return;
					}
				}
			}
			pendingIntentQueue.push({
				intent,
				waiters: [{ resolve, reject }]
			});
			void processIntentQueue();
		});
	};

	const sendIntents = async (intents: UiEditIntent[]): Promise<void> => {
		await runIntentBatch(intents);
	};

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
			resetInitialLoading();
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
				setLoadingState(
					{
						activeStep: 'transport',
						title: 'Runtime unavailable',
						message: 'Retrying connection',
						detail: `Next attempt in ${formatRetryDelay(delay)}`,
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
			try {
				setLoadingState({
					activeStep: 'snapshot',
					title: 'Loading workspace graph',
					message: 'Requesting the initial snapshot',
					detail: null,
					progress: 0.38
				});
				const fetchStartedAt = typeof performance !== 'undefined' ? performance.now() : Date.now();
				const snapshot = await requestSnapshot();
				const fetchElapsedMs =
					(typeof performance !== 'undefined' ? performance.now() : Date.now()) - fetchStartedAt;
				if (stopped) {
					return;
				}
				setLoadingState({
					activeStep: 'apply',
					title: 'Building workspace',
					message: 'Applying the project graph',
					detail: `${snapshot.nodes.length} nodes loaded`,
					progress: 0.72
				});
				const applyStartedAt = typeof performance !== 'undefined' ? performance.now() : Date.now();
				applySnapshotToState(snapshot);
				const applyElapsedMs =
					(typeof performance !== 'undefined' ? performance.now() : Date.now()) - applyStartedAt;
				logUiPerf(
					`bootstrap snapshot fetch_ms=${fetchElapsedMs.toFixed(1)} apply_ms=${applyElapsedMs.toFixed(
						1
					)} nodes=${snapshot.nodes.length}`
				);
				setLoadingState({
					activeStep: 'subscribe',
					title: 'Starting live updates',
					message: 'Subscribing to runtime events',
					detail: null,
					progress: 0.9
				});
				unsubscribe = client.subscribe(scope, snapshot.at, applyBatch);
				subscribed = true;
				clearRetry();
				retryDelayMs = retryMs;
				completeInitialLoading();
			} catch (error) {
				if (stopped) {
					return;
				}
				const message = error instanceof Error ? error.message : 'unknown connection error';
				connectionState = 'disconnected';
				syncConnectionStatus();
				setLoadingState(
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
			}
		};

		const onWindowKeydown = (event: KeyboardEvent): void => {
			if (isEditableTarget(event.target)) {
				return;
			}
			void handleCommandShortcut(event);
		};

		unregisterCommands = commandSuite.registerCommandHandlers();
		if (enableGlobalShortcuts && typeof window !== 'undefined') {
			window.addEventListener('keydown', onWindowKeydown, true);
		}
		void bootstrap();

		mountedCleanup = (): void => {
			stopped = true;
			clearRetry();
			clearLoadingHideTimer();
			rejectQueuedIntents(new Error('workbench session unmounted'));
			if (enableGlobalShortcuts && typeof window !== 'undefined') {
				window.removeEventListener('keydown', onWindowKeydown, true);
			}
			unregisterCommands();
			unsubscribe();
			graph.reset();
			warnings.reset();
			descriptions.reset();
			customEvents.reset();
			footerHover.reset();
			history.reset();
			selection.reset();
			commandSuite.reset();
			logger.reset();
			resetProjectFileFormat();
			hasLoadedSnapshot = false;
			connectionState = 'connecting';
			engineHz = null;
			syncConnectionStatus();
			resetInitialLoading();
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
		get httpBaseUrl(): string {
			return httpBaseUrl;
		},
		get loading(): WorkbenchLoadingState {
			return loading;
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
		refreshSnapshot,
		startLoadingOperation,
		mount
	};
};
