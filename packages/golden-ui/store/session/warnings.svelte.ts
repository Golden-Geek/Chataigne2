import type { NodeId, UiEventBatch, UiNodeDto, UiNodeWarningDto } from '../../types';
import type { GraphStore } from '../graph.svelte';
import type { NodeWarningRecord } from './types';

interface NormalizedNodeWarning {
	id: string;
	message: string;
	detail?: string;
}

interface VisibleWarningCacheEntry {
	version: number;
	warnings: NodeWarningRecord[];
}

export interface WorkbenchWarningStore {
	getNodeVisibleWarnings(nodeId: NodeId): NodeWarningRecord[];
	getActiveWarnings(): NodeWarningRecord[];
	hasNodeWarnings(nodeId: NodeId): boolean;
	batchAffectsWarnings(batch: UiEventBatch): boolean;
	invalidate(): void;
	reset(): void;
}

const DEFAULT_WARNING_ID = '';

const normalizeWarningId = (warningId: string | undefined): string =>
	typeof warningId === 'string' ? warningId : DEFAULT_WARNING_ID;

const normalizeNodeWarning = (warning: UiNodeWarningDto): NormalizedNodeWarning | null => {
	const message = String(warning.message ?? '').trim();
	if (message.length === 0) {
		return null;
	}

	const detailText = typeof warning.detail === 'string' ? warning.detail.trim() : '';
	return {
		id: normalizeWarningId(warning.id),
		message,
		detail: detailText.length > 0 ? detailText : undefined
	};
};

const metaPatchAffectsWarnings = (patch: Partial<UiNodeDto['meta']>): boolean =>
	patch.label !== undefined || patch.presentation !== undefined;

const normalizeWarningDepth = (value: unknown): number => {
	const rawDepth = Number(value);
	if (!Number.isFinite(rawDepth)) {
		return 0;
	}
	return Math.max(0, Math.floor(rawDepth));
};

export const createWorkbenchWarningStore = (graph: GraphStore): WorkbenchWarningStore => {
	let warningCacheVersion = 0;
	let activeWarningsCacheVersion = -1;
	let activeWarningsCache: NodeWarningRecord[] = [];
	const visibleWarningsCache = new Map<NodeId, VisibleWarningCacheEntry>();

	const invalidate = (): void => {
		warningCacheVersion += 1;
		activeWarningsCacheVersion = -1;
		activeWarningsCache = [];
		visibleWarningsCache.clear();
	};

	const getMetaWarningsForNode = (node: UiNodeDto): Map<string, NormalizedNodeWarning> => {
		const rawEntries = node.meta.presentation?.warnings;
		const entries = Array.isArray(rawEntries) ? rawEntries : [];
		const warningsById = new Map<string, NormalizedNodeWarning>();
		for (const rawWarning of entries) {
			const normalized = normalizeNodeWarning(rawWarning);
			if (!normalized) {
				continue;
			}
			warningsById.set(normalized.id, normalized);
		}
		return warningsById;
	};

	const getNodeOwnWarnings = (nodeId: NodeId): NormalizedNodeWarning[] => {
		const node = graph.state.nodesById.get(nodeId);
		if (!node) {
			return [];
		}
		return [...getMetaWarningsForNode(node).values()];
	};

	const getNodeWarningChildDepth = (nodeId: NodeId): number => {
		const node = graph.state.nodesById.get(nodeId);
		if (!node) {
			return 0;
		}
		return normalizeWarningDepth(node.meta.presentation?.show_child_warnings_max_depth ?? 0);
	};

	const labelForNode = (nodeId: NodeId): string => {
		return graph.state.nodesById.get(nodeId)?.meta.label ?? `Node ${nodeId}`;
	};

	const computeNodeVisibleWarnings = (nodeId: NodeId): NodeWarningRecord[] => {
		const targetNode = graph.state.nodesById.get(nodeId);
		if (!targetNode) {
			return [];
		}

		const maxDepth = getNodeWarningChildDepth(nodeId);
		const pending: Array<{ nodeId: NodeId; depth: number }> = [{ nodeId, depth: 0 }];
		const seenMinDepth = new Map<NodeId, number>();
		const warningRecordsByKey = new Map<string, NodeWarningRecord>();

		while (pending.length > 0) {
			const current = pending.shift();
			if (!current) {
				continue;
			}

			const knownDepth = seenMinDepth.get(current.nodeId);
			if (knownDepth !== undefined && knownDepth <= current.depth) {
				continue;
			}
			seenMinDepth.set(current.nodeId, current.depth);

			const ownWarnings = getNodeOwnWarnings(current.nodeId);
			for (const warning of ownWarnings) {
				const key = `${current.nodeId}:${warning.id}`;
				if (warningRecordsByKey.has(key)) {
					continue;
				}
				warningRecordsByKey.set(key, {
					targetNodeId: nodeId,
					targetNodeLabel: targetNode.meta.label,
					sourceNodeId: current.nodeId,
					sourceNodeLabel: labelForNode(current.nodeId),
					warningId: warning.id,
					message: warning.message,
					detail: warning.detail,
					distance: current.depth
				});
			}

			if (current.depth >= maxDepth) {
				continue;
			}

			const children = graph.state.childrenById.get(current.nodeId) ?? [];
			for (const childNodeId of children) {
				pending.push({ nodeId: childNodeId, depth: current.depth + 1 });
			}
		}

		return [...warningRecordsByKey.values()].sort((left, right) => {
			if (left.distance !== right.distance) {
				return left.distance - right.distance;
			}
			const byLabel = left.sourceNodeLabel.localeCompare(right.sourceNodeLabel);
			if (byLabel !== 0) {
				return byLabel;
			}
			const byWarningId = left.warningId.localeCompare(right.warningId);
			if (byWarningId !== 0) {
				return byWarningId;
			}
			return left.message.localeCompare(right.message);
		});
	};

	const getNodeVisibleWarnings = (nodeId: NodeId): NodeWarningRecord[] => {
		void graph.state.lastEventTime;

		const cached = visibleWarningsCache.get(nodeId);
		if (cached && cached.version === warningCacheVersion) {
			return cached.warnings;
		}

		const warnings = computeNodeVisibleWarnings(nodeId);
		visibleWarningsCache.set(nodeId, {
			version: warningCacheVersion,
			warnings
		});
		return warnings;
	};

	const computeActiveWarnings = (): NodeWarningRecord[] => {
		const allWarnings: NodeWarningRecord[] = [];
		for (const node of graph.state.nodesById.values()) {
			for (const warning of getNodeOwnWarnings(node.node_id)) {
				allWarnings.push({
					targetNodeId: node.node_id,
					targetNodeLabel: node.meta.label,
					sourceNodeId: node.node_id,
					sourceNodeLabel: node.meta.label,
					warningId: warning.id,
					message: warning.message,
					detail: warning.detail,
					distance: 0
				});
			}
		}

		return allWarnings.sort((left, right) => {
			const byLabel = left.sourceNodeLabel.localeCompare(right.sourceNodeLabel);
			if (byLabel !== 0) {
				return byLabel;
			}
			const byWarningId = left.warningId.localeCompare(right.warningId);
			if (byWarningId !== 0) {
				return byWarningId;
			}
			return left.message.localeCompare(right.message);
		});
	};

	const getActiveWarnings = (): NodeWarningRecord[] => {
		void graph.state.lastEventTime;

		if (activeWarningsCacheVersion === warningCacheVersion) {
			return activeWarningsCache;
		}

		activeWarningsCache = computeActiveWarnings();
		activeWarningsCacheVersion = warningCacheVersion;
		return activeWarningsCache;
	};

	const hasNodeWarnings = (nodeId: NodeId): boolean => {
		return getNodeVisibleWarnings(nodeId).length > 0;
	};

	const batchAffectsWarnings = (batch: UiEventBatch): boolean => {
		for (const event of batch.events) {
			switch (event.kind.kind) {
				case 'metaChanged':
				case 'childAdded':
				case 'childRemoved':
				case 'childReplaced':
				case 'childMoved':
				case 'childReordered':
				case 'nodeCreated':
				case 'nodeDeleted':
					return true;
				case 'graphTransaction':
					return event.kind.ops.some((op) => {
						switch (op.kind) {
							case 'nodeCreated':
							case 'subtreeInserted':
							case 'subtreeRemoved':
							case 'nodeMoved':
							case 'childrenReordered':
								return true;
							case 'nodeMetaPatched':
								return metaPatchAffectsWarnings(op.patch);
							default:
								return false;
						}
					});
				default:
					break;
			}
		}
		return false;
	};

	const reset = (): void => {
		warningCacheVersion = 0;
		activeWarningsCacheVersion = -1;
		activeWarningsCache = [];
		visibleWarningsCache.clear();
	};

	return {
		getNodeVisibleWarnings,
		getActiveWarnings,
		hasNodeWarnings,
		batchAffectsWarnings,
		invalidate,
		reset
	};
};
