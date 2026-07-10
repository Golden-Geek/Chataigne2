import type { NodeId, UiNodeDto } from '../../types';

export interface NodeTreeClipboardNode {
	sourceId: NodeId;
	sourceUuid: string;
	node_type: string;
	user_item_kind: string;
	decl_id: string;
	label: string;
	data: UiNodeDto['data'];
	meta: {
		label: string;
		enabled: boolean;
		can_be_disabled: boolean;
		presentation: UiNodeDto['meta']['presentation'];
	};
	children: NodeTreeClipboardNode[];
}

export interface NodeTreeClipboardPayload {
	kind: 'golden-ui.node-tree';
	version: 1;
	nodes: NodeTreeClipboardNode[];
}

export const NODE_TREE_CLIPBOARD_KIND = 'golden-ui.node-tree';
export const NODE_TREE_CLIPBOARD_VERSION = 1;

const nodeTreeFromNode = (
	node: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): NodeTreeClipboardNode => ({
	sourceId: node.node_id,
	sourceUuid: node.uuid,
	node_type: node.node_type,
	user_item_kind: node.user_item_kind,
	decl_id: node.decl_id,
	label: node.meta.label.trim().length > 0 ? node.meta.label.trim() : node.node_type,
	data: node.data,
	meta: {
		label: node.meta.label,
		enabled: node.meta.enabled,
		can_be_disabled: node.meta.can_be_disabled,
		presentation: node.meta.presentation
	},
	children: node.children.flatMap((childId): NodeTreeClipboardNode[] => {
		const child = nodesById.get(childId);
		return child ? [nodeTreeFromNode(child, nodesById)] : [];
	})
});

export const buildNodeTreeClipboardPayload = (
	nodes: readonly UiNodeDto[],
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): NodeTreeClipboardPayload => ({
	kind: NODE_TREE_CLIPBOARD_KIND,
	version: NODE_TREE_CLIPBOARD_VERSION,
	nodes: nodes.map((node) => nodeTreeFromNode(node, nodesById))
});

export const nodeTreeClipboardJson = (payload: NodeTreeClipboardPayload): string =>
	JSON.stringify(payload, null, 2);

const isRecord = (candidate: unknown): candidate is Record<string, unknown> =>
	typeof candidate === 'object' && candidate !== null;

const numberField = (record: Record<string, unknown>, field: string): number | null => {
	const value = record[field];
	return typeof value === 'number' && Number.isSafeInteger(value) ? value : null;
};

const stringField = (record: Record<string, unknown>, field: string): string | null => {
	const value = record[field];
	return typeof value === 'string' ? value : null;
};

const nodeTreeFromJson = (candidate: unknown): NodeTreeClipboardNode | null => {
	if (!isRecord(candidate)) return null;
	const sourceId = numberField(candidate, 'sourceId');
	const sourceUuid = stringField(candidate, 'sourceUuid');
	const nodeType = stringField(candidate, 'node_type');
	const declId = stringField(candidate, 'decl_id');
	const label = stringField(candidate, 'label');
	const data = candidate.data;
	const meta = candidate.meta;
	const children = candidate.children;
	if (
		sourceId === null ||
		sourceUuid === null ||
		nodeType === null ||
		declId === null ||
		label === null ||
		!isRecord(data) ||
		!isRecord(meta) ||
		!Array.isArray(children)
	) {
		return null;
	}
	return {
		sourceId,
		sourceUuid,
		node_type: nodeType,
		user_item_kind: stringField(candidate, 'user_item_kind') ?? '',
		decl_id: declId,
		label,
		data: data as UiNodeDto['data'],
		meta: {
			label: typeof meta.label === 'string' ? meta.label : label,
			enabled: typeof meta.enabled === 'boolean' ? meta.enabled : true,
			can_be_disabled: typeof meta.can_be_disabled === 'boolean' ? meta.can_be_disabled : false,
			presentation: isRecord(meta.presentation)
				? (meta.presentation as UiNodeDto['meta']['presentation'])
				: undefined
		},
		children: children.flatMap((child): NodeTreeClipboardNode[] => {
			const tree = nodeTreeFromJson(child);
			return tree ? [tree] : [];
		})
	};
};

export const nodeTreeClipboardFromJson = (text: string): NodeTreeClipboardPayload | null => {
	try {
		const payload: unknown = JSON.parse(text);
		if (
			!isRecord(payload) ||
			payload.kind !== NODE_TREE_CLIPBOARD_KIND ||
			payload.version !== NODE_TREE_CLIPBOARD_VERSION ||
			!Array.isArray(payload.nodes)
		) {
			return null;
		}
		const nodes = payload.nodes
			.map(nodeTreeFromJson)
			.filter((node): node is NodeTreeClipboardNode => node !== null);
		return nodes.length > 0
			? {
					kind: NODE_TREE_CLIPBOARD_KIND,
					version: NODE_TREE_CLIPBOARD_VERSION,
					nodes
				}
			: null;
	} catch {
		return null;
	}
};
