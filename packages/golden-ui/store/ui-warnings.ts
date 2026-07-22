import type { NodeId } from '../types';
import type { NodeWarningRecord } from './workbench.svelte';
import { appState } from './workbench.svelte';

export const getNodeVisibleWarnings = (nodeId: NodeId): NodeWarningRecord[] => {
	return appState.session?.getNodeVisibleWarnings(nodeId) ?? [];
};

export const getActiveWarnings = (): NodeWarningRecord[] => {
	return appState.session?.getActiveWarnings() ?? [];
};

export const nodeHasWarnings = (nodeId: NodeId): boolean => {
	return appState.session?.hasNodeWarnings(nodeId) ?? false;
};
