import type { NodeId } from '../types';

export const nodeContextMenuState = $state({
	nodeId: null as NodeId | null,
	position: { x: 0, y: 0 },
	showChildCreation: true
});

export const openNodeContextMenu = (
	nodeId: NodeId,
	x: number,
	y: number,
	options: { showChildCreation?: boolean } = {}
): void => {
	nodeContextMenuState.nodeId = nodeId;
	nodeContextMenuState.position = { x, y };
	nodeContextMenuState.showChildCreation = options.showChildCreation ?? true;
};

export const closeNodeContextMenu = (): void => {
	nodeContextMenuState.nodeId = null;
	nodeContextMenuState.position = { x: 0, y: 0 };
	nodeContextMenuState.showChildCreation = true;
};
