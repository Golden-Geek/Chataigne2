import type { SerializedDockview } from 'dockview-core';
import type { NodeId } from '../types';
import {
	loadPersistedDockLayout,
	savePersistedDockLayout,
	savePersistedSelection
} from './ui-persistence';

export interface ProjectUiState {
	dock_layout?: SerializedDockview;
	selected_node_ids: NodeId[];
}

export interface ProjectUiStateHost {
	captureDockLayout(): SerializedDockview | null;
	restoreDockLayout(layout: SerializedDockview): boolean;
}

let projectUiStateHost: ProjectUiStateHost | null = null;

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

const asSerializedDockview = (value: unknown): SerializedDockview | null => {
	if (!isRecord(value)) {
		return null;
	}
	if (!isRecord(value.grid) || !isRecord(value.panels)) {
		return null;
	}
	return value as unknown as SerializedDockview;
};

const asNodeIdArray = (value: unknown): NodeId[] => {
	if (!Array.isArray(value)) {
		return [];
	}

	const uniqueNodeIds = new Set<NodeId>();
	for (const candidate of value) {
		if (typeof candidate !== 'number' || !Number.isFinite(candidate)) {
			continue;
		}
		uniqueNodeIds.add(candidate);
	}

	return Array.from(uniqueNodeIds);
};

const asProjectUiState = (value: unknown): ProjectUiState | null => {
	if (!isRecord(value)) {
		return null;
	}

	const dockLayout = asSerializedDockview(value.dock_layout);
	return {
		...(dockLayout ? { dock_layout: dockLayout } : {}),
		selected_node_ids: asNodeIdArray(value.selected_node_ids)
	};
};

export const registerProjectUiStateHost = (host: ProjectUiStateHost): (() => void) => {
	projectUiStateHost = host;
	return () => {
		if (projectUiStateHost === host) {
			projectUiStateHost = null;
		}
	};
};

export const captureProjectUiState = (selectedNodeIds: NodeId[]): ProjectUiState => {
	const dockLayout = projectUiStateHost?.captureDockLayout() ?? loadPersistedDockLayout();
	const normalizedSelection = asNodeIdArray(selectedNodeIds);
	savePersistedSelection(normalizedSelection);

	return {
		...(dockLayout ? { dock_layout: dockLayout } : {}),
		selected_node_ids: normalizedSelection
	};
};

export const restoreProjectUiState = (value: unknown): void => {
	const uiState = asProjectUiState(value);
	if (!uiState) {
		savePersistedSelection([]);
		return;
	}

	savePersistedSelection(uiState.selected_node_ids);
	if (!uiState.dock_layout) {
		return;
	}

	savePersistedDockLayout(uiState.dock_layout);
	projectUiStateHost?.restoreDockLayout(uiState.dock_layout);
};
