import type { NodeId } from '../types';
import type { CreationSelectionPolicy } from './ui-intents';

export const nodeContextMenuState = $state({
	nodeId: null as NodeId | null,
	position: { x: 0, y: 0 },
	showChildCreation: true,
	creationSelection: 'created' as CreationSelectionPolicy
});

export const openNodeContextMenu = (
	nodeId: NodeId,
	x: number,
	y: number,
	options: { showChildCreation?: boolean; creationSelection?: CreationSelectionPolicy } = {}
): void => {
	nodeContextMenuState.nodeId = nodeId;
	nodeContextMenuState.position = { x, y };
	nodeContextMenuState.showChildCreation = options.showChildCreation ?? true;
	nodeContextMenuState.creationSelection = options.creationSelection ?? 'created';
};

export const closeNodeContextMenu = (): void => {
	nodeContextMenuState.nodeId = null;
	nodeContextMenuState.position = { x: 0, y: 0 };
	nodeContextMenuState.showChildCreation = true;
	nodeContextMenuState.creationSelection = 'created';
};
