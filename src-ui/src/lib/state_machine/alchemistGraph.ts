import type { GraphEdge, GraphNode, GraphSocket } from 'golden_alchemist_ui';
import type { NodeId, ParamValue, UiColorDto, UiCreatableUserItem, UiNodeDto } from 'golden_ui';

export const FORMULA_NODE_TYPE = 'alchemist_formula';
export const ANODE_NODE_TYPE = 'alchemist_anode';
export const CONNECTION_NODE_TYPE = 'alchemist_connection';
export const ANODE_CREATE_PREFIX = 'alchemist_anode:';
export const PROPERTIES_DECL_ID = 'properties';
export const PROPERTY_NODE_TYPE = 'alchemist_property';
export const PROPERTY_MANAGER_NODE_TYPE = 'alchemist_property_manager';
export const PROPERTY_FOLDER_NODE_TYPE = 'alchemist_property_folder';

export const TRIGGER_SOCKET_ID = '__trigger';
export const ANODE_TYPE_TAG_PREFIX = 'alchemist.anode.type:';

export const MANAGER_REF_TYPE_CONDITIONS = 'chataigne.conditions_manager';
export const MANAGER_REF_TYPE_CONSEQUENCES = 'chataigne.consequences_manager';
export const MANAGER_REF_TYPE_INPUTS = 'chataigne.inputs_manager';
export const MANAGER_REF_TYPE_OUTPUTS = 'chataigne.outputs_manager';
export const MANAGER_REF_TYPE_FILTERS = 'chataigne.filters_manager';

const MANAGER_REF_TYPES = new Set([
	MANAGER_REF_TYPE_CONDITIONS,
	MANAGER_REF_TYPE_CONSEQUENCES,
	MANAGER_REF_TYPE_INPUTS,
	MANAGER_REF_TYPE_OUTPUTS,
	MANAGER_REF_TYPE_FILTERS
]);

export const managerAnodeType = (role: string): string => {
	switch (role) {
		case 'condition':
			return MANAGER_REF_TYPE_CONDITIONS;
		case 'consequence':
			return MANAGER_REF_TYPE_CONSEQUENCES;
		case 'input':
			return MANAGER_REF_TYPE_INPUTS;
		case 'output':
			return MANAGER_REF_TYPE_OUTPUTS;
		case 'filter':
			return MANAGER_REF_TYPE_FILTERS;
		default:
			return '';
	}
};

const clamp01 = (value: number): number => Math.min(1, Math.max(0, value));

const metadataColor = (color: UiColorDto | null | undefined): string | undefined => {
	if (!color) return undefined;
	const r = Math.round(clamp01(color.r) * 255);
	const g = Math.round(clamp01(color.g) * 255);
	const b = Math.round(clamp01(color.b) * 255);
	return `rgb(${r} ${g} ${b} / ${clamp01(color.a)})`;
};

const tagValue = (tags: readonly string[], prefix: string): string | undefined =>
	tags
		.find((tag) => tag.startsWith(prefix))
		?.slice(prefix.length)
		.trim() || undefined;

export const anodeType = (node: UiNodeDto): string =>
	tagValue(node.meta.tags, ANODE_TYPE_TAG_PREFIX) ?? '';

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
				valueType: socketParameter(socket, nodesById, '/value_type') ?? undefined,
				color: metadataColor(socket.meta.presentation?.color)
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

const formulaPropertiesByUuid = (
	formula: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): ReadonlyMap<string, UiNodeDto> => {
	const propertiesRoot = directChild(formula, nodesById, PROPERTIES_DECL_ID);
	const byUuid = new Map<string, UiNodeDto>();
	const pending = propertiesRoot ? [...propertiesRoot.children] : [];
	while (pending.length > 0) {
		const node = nodesById.get(pending.pop() as NodeId);
		if (!node) continue;
		if (node.node_type === PROPERTY_NODE_TYPE) {
			byUuid.set(node.uuid, node);
		}
		pending.push(...node.children);
	}
	return byUuid;
};

export const toGraphNodes = (
	formula: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	_catalogItems: readonly UiCreatableUserItem[] = []
): GraphNode[] => {
	const propertiesByUuid = formulaPropertiesByUuid(formula, nodesById);
	return formulaANodes(formula, nodesById).map((anode) => {
		const position = parameterValue(anode, nodesById, 'position');
		const width = parameterValue(anode, nodesById, 'width');
		const typeId = anodeType(anode);
		const propertyGetter = typeId === 'property';
		const managerRef = MANAGER_REF_TYPES.has(typeId);
		const compactNode = propertyGetter || managerRef;
		const description = anode.meta.description?.trim();
		const allInputs = graphSockets(anode, nodesById, 'inputs', 'alchemist_input_socket');
		const triggerSocket = allInputs.find((s) => s.id === TRIGGER_SOCKET_ID);
		const bodyInputs = triggerSocket
			? allInputs.filter((s) => s.id !== TRIGGER_SOCKET_ID)
			: allInputs;
		const config = directChild(anode, nodesById, 'config');
		const propertyId = propertyGetter
			? stringParameter(config, nodesById, 'config/property_id')
			: null;
		const propertyNode = propertyId ? propertiesByUuid.get(propertyId) : undefined;
		return {
			id: String(anode.node_id),
			label: anode.meta.label,
			subtitle: compactNode ? undefined : typeId,
			description: description ? description : undefined,
			canRename: anode.meta.user_permissions?.can_edit_name !== false,
			collapsed: anode.meta.presentation?.collapsed === true,
			color: metadataColor(
				propertyNode?.meta.presentation?.color ?? anode.meta.presentation?.color
			),
			x: position?.kind === 'vec2' ? position.value[0] : 0,
			y: position?.kind === 'vec2' ? position.value[1] : 0,
			width: width?.kind === 'float' && width.value > 0 ? width.value : undefined,
			resizable: !compactNode,
			invalid: typeId.length === 0,
			inputs: bodyInputs,
			outputs: graphSockets(anode, nodesById, 'outputs', 'alchemist_output_socket'),
			headerInputs: triggerSocket ? [triggerSocket] : undefined
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
