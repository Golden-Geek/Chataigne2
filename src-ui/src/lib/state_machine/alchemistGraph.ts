import type { GraphEdge, GraphNode, GraphSocket } from 'golden_alchemist_ui';
import type { NodeId, ParamValue, UiCreatableUserItem, UiNodeDto } from 'golden_ui';

export const FORMULA_NODE_TYPE = 'alchemist_formula';
export const ANODE_NODE_TYPE = 'alchemist_anode';
export const CONNECTION_NODE_TYPE = 'alchemist_connection';
export const ANODE_CREATE_PREFIX = 'alchemist_anode:';

const FAMILY_HUES: Readonly<Record<string, number>> = {
	Math: 211,
	Chataigne: 326,
	Values: 42,
	Logic: 268,
	Flow: 158,
	Debug: 14
};

const stableHash = (value: string): number => {
	let hash = 2166136261;
	for (let index = 0; index < value.length; index += 1) {
		hash ^= value.charCodeAt(index);
		hash = Math.imul(hash, 16777619);
	}
	return hash >>> 0;
};

const familyHue = (family: string): number => FAMILY_HUES[family] ?? stableHash(family) % 360;

export const anodeColor = (family: string, typeId: string): string => {
	const variation = stableHash(typeId);
	const hue = (familyHue(family) + (variation % 25) - 12 + 360) % 360;
	const saturation = 62 + ((variation >>> 8) % 16);
	const lightness = 48 + ((variation >>> 16) % 12);
	return `hsl(${hue} ${saturation}% ${lightness}%)`;
};

export const directChild = (
	node: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	declId: string
): UiNodeDto | null => {
	if (!node) return null;
	for (const childId of node.children) {
		const child = nodesById.get(childId);
		if (child?.decl_id === declId) return child;
	}
	return null;
};

export const parameterChild = (
	node: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	declId: string
): UiNodeDto | null => {
	const child = directChild(node, nodesById, declId);
	return child?.data.kind === 'parameter' ? child : null;
};

export const parameterValue = (
	node: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	declId: string
): ParamValue | null => {
	const child = parameterChild(node, nodesById, declId);
	return child?.data.kind === 'parameter' ? child.data.param.value : null;
};

const stringParameter = (
	node: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	declId: string
): string | null => {
	const value = parameterValue(node, nodesById, declId);
	return value?.kind === 'str' ? value.value : null;
};

const socketParameter = (
	socket: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	suffix: string
): string | null => {
	for (const childId of socket.children) {
		const child = nodesById.get(childId);
		if (
			child?.decl_id.endsWith(suffix) &&
			child.data.kind === 'parameter' &&
			child.data.param.value.kind === 'str'
		) {
			return child.data.param.value.value;
		}
	}
	return null;
};

const graphSockets = (
	anode: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	folderDeclId: 'inputs' | 'outputs',
	socketNodeType: 'alchemist_input_socket' | 'alchemist_output_socket'
): GraphSocket[] => {
	const folder = directChild(anode, nodesById, folderDeclId);
	if (!folder) return [];
	return folder.children.flatMap((childId) => {
		const socket = nodesById.get(childId);
		if (!socket || socket.node_type !== socketNodeType) return [];
		const id = socketParameter(socket, nodesById, '/socket_id');
		if (!id) return [];
		return [
			{
				id,
				label: socket.meta.label,
				valueType: socketParameter(socket, nodesById, '/value_type') ?? undefined
			}
		];
	});
};

export const formulaANodes = (
	formula: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): UiNodeDto[] =>
	formula
		? formula.children.flatMap((childId) => {
				const child = nodesById.get(childId);
				return child?.node_type === ANODE_NODE_TYPE ? [child] : [];
			})
		: [];

export const formulaConnections = (
	formula: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): UiNodeDto[] =>
	formula
		? formula.children.flatMap((childId) => {
				const child = nodesById.get(childId);
				return child?.node_type === CONNECTION_NODE_TYPE ? [child] : [];
			})
		: [];

export const toGraphNodes = (
	formula: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	catalogItems: readonly UiCreatableUserItem[] = []
): GraphNode[] => {
	const familyByType = new Map(
		catalogItems.flatMap((item) => {
			const typeId = item.node_type.startsWith(ANODE_CREATE_PREFIX)
				? item.node_type.slice(ANODE_CREATE_PREFIX.length)
				: null;
			return typeId ? [[typeId, item.menu_path[0] ?? 'General'] as const] : [];
		})
	);
	return formulaANodes(formula, nodesById).map((anode) => {
		const position = parameterValue(anode, nodesById, 'position');
		const width = parameterValue(anode, nodesById, 'width');
		const typeId = stringParameter(anode, nodesById, 'anode_type') ?? '';
		const family = familyByType.get(typeId) ?? 'General';
		return {
			id: String(anode.node_id),
			label: anode.meta.label,
			subtitle: typeId,
			color: anodeColor(family, typeId),
			x: position?.kind === 'vec2' ? position.value[0] : 0,
			y: position?.kind === 'vec2' ? position.value[1] : 0,
			width: width?.kind === 'float' && width.value > 0 ? width.value : undefined,
			resizable: true,
			invalid: typeId.length === 0,
			inputs: graphSockets(anode, nodesById, 'inputs', 'alchemist_input_socket'),
			outputs: graphSockets(anode, nodesById, 'outputs', 'alchemist_output_socket')
		};
	});
};

const referencedNodeId = (
	value: ParamValue | null,
	nodeIdByUuid: ReadonlyMap<string, NodeId>
): NodeId | null => {
	if (value?.kind !== 'reference') return null;
	return value.cached_id ?? nodeIdByUuid.get(value.uuid) ?? null;
};

export const toGraphEdges = (
	formula: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): GraphEdge[] => {
	const nodeIdByUuid = new Map(
		formulaANodes(formula, nodesById).map((node) => [node.uuid, node.node_id])
	);
	return formulaConnections(formula, nodesById).flatMap((connection) => {
		const source = referencedNodeId(
			parameterValue(connection, nodesById, 'source_node'),
			nodeIdByUuid
		);
		const target = referencedNodeId(
			parameterValue(connection, nodesById, 'target_node'),
			nodeIdByUuid
		);
		const sourceSocket = stringParameter(connection, nodesById, 'source_socket');
		const targetSocket = stringParameter(connection, nodesById, 'target_socket');
		if (source === null || target === null || !sourceSocket || !targetSocket) return [];
		return [
			{
				id: String(connection.node_id),
				from: { nodeId: String(source), socketId: sourceSocket },
				to: { nodeId: String(target), socketId: targetSocket }
			}
		];
	});
};

export const configParameters = (
	anode: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): UiNodeDto[] => {
	const config = directChild(anode, nodesById, 'config');
	if (!config) return [];
	return config.children.flatMap((childId) => {
		const child = nodesById.get(childId);
		return child?.data.kind === 'parameter' ? [child] : [];
	});
};
