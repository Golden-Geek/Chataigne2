import type { NodeId, UiNodeDto } from '../../../types';

export const DASHBOARD_DRAG_MIME = 'application/x-golden-dashboard-drag';

export type DashboardDragPayload =
	| {
			kind: 'node';
			nodeId: NodeId;
			nodeType: string;
			label: string;
			source: 'outliner';
	  }
	| {
			kind: 'parameter';
			nodeId: NodeId;
			nodeType: string;
			label: string;
			source: 'inspector';
	  };

let activeDashboardDragPayload: DashboardDragPayload | null = null;

const clearActiveDashboardDragPayload = (): void => {
	activeDashboardDragPayload = null;
};

const writePayload = (event: DragEvent, payload: DashboardDragPayload): void => {
	const transfer = event.dataTransfer;
	if (!transfer) {
		return;
	}

	activeDashboardDragPayload = payload;
	if (typeof window !== 'undefined') {
		window.addEventListener('dragend', clearActiveDashboardDragPayload, {
			once: true,
			capture: true
		});
	}

	transfer.effectAllowed = 'copyMove';
	transfer.setData(DASHBOARD_DRAG_MIME, JSON.stringify(payload));
	transfer.setData('text/plain', payload.label);
	transfer.setData('text/golden-node-id', String(payload.nodeId));
	transfer.setData('text/golden-node-type', payload.nodeType);
	transfer.setData('text/golden-drag-kind', payload.kind);
	transfer.dropEffect = 'copy';
};

export const beginDashboardNodeDrag = (event: DragEvent, node: UiNodeDto): void => {
	writePayload(event, {
		kind: 'node',
		nodeId: node.node_id,
		nodeType: node.node_type,
		label: node.meta.label,
		source: 'outliner'
	});
};

export const beginDashboardParameterDrag = (event: DragEvent, node: UiNodeDto): void => {
	writePayload(event, {
		kind: 'parameter',
		nodeId: node.node_id,
		nodeType: node.node_type,
		label: node.meta.label,
		source: 'inspector'
	});
};

export const readDashboardDragPayload = (event: DragEvent): DashboardDragPayload | null => {
	const raw = event.dataTransfer?.getData(DASHBOARD_DRAG_MIME) ?? '';
	if (raw.length === 0) {
		return activeDashboardDragPayload;
	}

	try {
		const parsed = JSON.parse(raw) as Partial<DashboardDragPayload>;
		if (
			(parsed.kind !== 'node' && parsed.kind !== 'parameter') ||
			typeof parsed.nodeId !== 'number' ||
			typeof parsed.nodeType !== 'string' ||
			typeof parsed.label !== 'string'
		) {
			return null;
		}

		if (parsed.kind === 'node') {
			return {
				kind: 'node',
				nodeId: parsed.nodeId,
				nodeType: parsed.nodeType,
				label: parsed.label,
				source: 'outliner'
			};
		}

		return {
			kind: 'parameter',
			nodeId: parsed.nodeId,
			nodeType: parsed.nodeType,
			label: parsed.label,
			source: 'inspector'
		};
	} catch {
		return activeDashboardDragPayload;
	}
};
