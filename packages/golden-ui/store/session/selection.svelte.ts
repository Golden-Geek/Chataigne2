import type { NodeId, UiNodeDto } from '../../types';
import type { GraphStore } from '../graph.svelte';
import { loadPersistedSelection, savePersistedSelection } from '../ui-persistence';
import type { SelectionMode } from './types';

export interface WorkbenchSelectionStore {
	readonly selectedNodeIds: NodeId[];
	readonly selectedNodeId: NodeId | null;
	getSelectedNodes(): UiNodeDto[];
	getFirstSelectedNode(): UiNodeDto | null;
	isNodeSelected(nodeId: NodeId): boolean;
	selectNode(nodeId: NodeId | null, selectionMode?: SelectionMode): void;
	selectNodes(nodeIds: NodeId[], selectionMode?: SelectionMode): void;
	clearSelection(): void;
	restorePersistedSelection(): void;
	reconcileSelection(): void;
	reset(): void;
}

export const createWorkbenchSelectionStore = (graph: GraphStore): WorkbenchSelectionStore => {
	let selectedNodeIds = $state<NodeId[]>([]);
	let persistedSelection = loadPersistedSelection();
	let shouldRestorePersistedSelection = persistedSelection.length > 0;

	const persistSelection = (): void => {
		savePersistedSelection(selectedNodeIds);
	};

	const setSelection = (nextSelection: NodeId[]): void => {
		selectedNodeIds = nextSelection;
		persistSelection();
	};

	const getSelectedNodes = (): UiNodeDto[] =>
		selectedNodeIds
			.map((nodeId) => graph.state.nodesById.get(nodeId))
			.filter((node): node is UiNodeDto => node !== undefined);

	const getFirstSelectedNode = (): UiNodeDto | null => {
		const firstId = selectedNodeIds[0];
		if (firstId === undefined) {
			return null;
		}
		return graph.state.nodesById.get(firstId) ?? null;
	};

	const isNodeSelected = (nodeId: NodeId): boolean => selectedNodeIds.includes(nodeId);

	const clearSelection = (): void => {
		setSelection([]);
	};

	const selectNodes = (nodeIds: NodeId[], selectionMode: SelectionMode = 'REPLACE'): void => {
		const validUniqueIds: NodeId[] = [];
		for (const nodeId of nodeIds) {
			if (!graph.state.nodesById.has(nodeId)) {
				continue;
			}
			if (validUniqueIds.includes(nodeId)) {
				continue;
			}
			validUniqueIds.push(nodeId);
		}

		if (selectionMode === 'REPLACE') {
			setSelection(validUniqueIds);
			return;
		}

		if (selectionMode === 'REMOVE') {
			setSelection(selectedNodeIds.filter((id) => !validUniqueIds.includes(id)));
			return;
		}

		if (selectionMode === 'TOGGLE') {
			const toggled = new Set(selectedNodeIds);
			for (const nodeId of validUniqueIds) {
				if (toggled.has(nodeId)) {
					toggled.delete(nodeId);
				} else {
					toggled.add(nodeId);
				}
			}
			setSelection(Array.from(toggled));
			return;
		}

		const merged = [...selectedNodeIds];
		for (const nodeId of validUniqueIds) {
			if (!merged.includes(nodeId)) {
				merged.push(nodeId);
			}
		}
		setSelection(merged);
	};

	const selectNode = (nodeId: NodeId | null, selectionMode: SelectionMode = 'REPLACE'): void => {
		if (nodeId === null) {
			clearSelection();
			return;
		}
		selectNodes([nodeId], selectionMode);
	};

	const restorePersistedSelection = (): void => {
		if (!shouldRestorePersistedSelection) {
			return;
		}

		shouldRestorePersistedSelection = false;
		const validSelection = persistedSelection.filter((nodeId) => graph.state.nodesById.has(nodeId));
		setSelection(validSelection);
	};

	const reconcileSelection = (): void => {
		const nextSelection = selectedNodeIds.filter((nodeId) => graph.state.nodesById.has(nodeId));
		if (
			nextSelection.length === selectedNodeIds.length &&
			nextSelection.every((nodeId, index) => nodeId === selectedNodeIds[index])
		) {
			return;
		}

		setSelection(nextSelection);
	};

	const reset = (): void => {
		selectedNodeIds = [];
		persistedSelection = loadPersistedSelection();
		shouldRestorePersistedSelection = persistedSelection.length > 0;
	};

	return {
		get selectedNodeIds(): NodeId[] {
			return selectedNodeIds;
		},
		get selectedNodeId(): NodeId | null {
			return selectedNodeIds[0] ?? null;
		},
		getSelectedNodes,
		getFirstSelectedNode,
		isNodeSelected,
		selectNode,
		selectNodes,
		clearSelection,
		restorePersistedSelection,
		reconcileSelection,
		reset
	};
};
