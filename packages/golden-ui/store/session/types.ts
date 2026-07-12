import type { NodeId } from '../../types';

export type SelectionMode = 'REPLACE' | 'ADD' | 'REMOVE' | 'TOGGLE';

export interface NodeWarningRecord {
	targetNodeId: number;
	targetNodeLabel: string;
	sourceNodeId: number;
	sourceNodeLabel: string;
	warningId: string;
	message: string;
	detail?: string;
	distance: number;
}

export type WorkbenchToastLevel = 'info' | 'success' | 'warning' | 'error';

export interface WorkbenchToast {
	id: number;
	level: WorkbenchToastLevel;
	message: string;
}

export interface FooterHoverInfo {
	node_id: NodeId;
	label: string;
	node_type: string;
	description: string;
}
