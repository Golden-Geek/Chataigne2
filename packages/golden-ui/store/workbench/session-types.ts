import type { UiApplicationInfoDto } from '../../generated/rust_protocol/UiApplicationInfoDto';
import type { UiTransportFactory } from '../../transport';
import type {
	NodeId,
	UiClient,
	UiEditIntent,
	UiLogRecord,
	UiNodeDto,
	UiRuntimeStats,
	UiSubscriptionScope
} from '../../types';
import type { GraphStore } from '../graph.svelte';
import type { UiEditOperations } from '../session/edit-operations';
import type { AppliedIntentObserver } from '../session/intent-sender';
import type { NodeSelectionIdentity } from '../session/pending-selection';
import type { WorkbenchAppOperation, WorkbenchAppOperationValue } from '../session/app-operations';
import type { WorkbenchConnectionStatus } from '../session/connection-state';
import type { WorkbenchUserActionPhase } from '../session/action-activity.svelte';
import type {
	WorkbenchLoadingOperation,
	WorkbenchLoadingState,
	WorkbenchLoadingStateRequest
} from '../session/loading.svelte';
import type { ProjectGraphTransitionControl } from '../session/project-graph-transition.svelte';
import type {
	FooterHoverInfo,
	NodeWarningRecord,
	SelectionMode,
	WorkbenchToast,
	WorkbenchToastLevel
} from '../session/types';

export interface WorkbenchSessionOptions {
	application?: UiApplicationInfoDto;
	wsUrl?: string;
	httpBaseUrl?: string;
	scope?: UiSubscriptionScope;
	bootstrapRetryMs?: number;
	transportFactory?: UiTransportFactory;
	enableGlobalShortcuts?: boolean;
}

export interface WorkbenchSession {
	readonly editOperations: UiEditOperations;
	readonly graph: GraphStore;
	readonly client: UiClient;
	readonly loading: WorkbenchLoadingState;
	readonly logRecords: readonly UiLogRecord[];
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
	/** Monotonic explicit selection intent; graph projection alone does not advance it. */
	readonly selectionRevision: number;
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
	/** Select exact committed identities once they are projected; later user selection cancels it. */
	requestNodeSelection(nodes: readonly NodeSelectionIdentity[]): void;
	clearSelection(): void;
	appendUiLogRecord(
		level: WorkbenchToastLevel,
		tag: string,
		message: string,
		origin?: NodeId
	): void;
	sendIntent(intent: UiEditIntent, applied?: AppliedIntentObserver): Promise<void>;
	sendIntents(intents: UiEditIntent[], applied?: AppliedIntentObserver): Promise<void>;
	invokeAppOperation(
		request: WorkbenchAppOperation,
		applied?: AppliedIntentObserver
	): Promise<WorkbenchAppOperationValue>;
	setLogUiUpdateHz(hz: number): void;
	dismissToast(toastId: number): void;
	undo(): Promise<void>;
	redo(): Promise<void>;
	refreshSnapshot(): Promise<boolean>;
	refreshSnapshotAfterCurrentSynchronization(): Promise<boolean>;
	/** Drain confirmed edits and retire active gestures before save/history or other session work. */
	runEditBarrier<T>(operation: () => Promise<T>): Promise<T>;
	runProjectGraphTransition<T>(
		operation: (control: ProjectGraphTransitionControl) => Promise<T>
	): Promise<T>;
	startLoadingOperation(initial: WorkbenchLoadingStateRequest): WorkbenchLoadingOperation;
	mount(): () => void;
}
