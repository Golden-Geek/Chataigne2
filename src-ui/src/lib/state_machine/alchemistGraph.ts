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

const valueTypeColor = (typeId: string | null | undefined): string | undefined => {
	switch (typeId) {
		case 'trigger':
			return 'rgb(250 107 56 / 1)';
		case 'int':
			return 'rgb(89 158 242 / 1)';
		case 'float':
			return 'rgb(71 189 133 / 1)';
		case 'bool':
			return 'rgb(219 112 199 / 1)';
		case 'vec2':
			return 'rgb(82 184 235 / 1)';
		case 'vec3':
			return 'rgb(122 148 240 / 1)';
		case 'color':
			return 'rgb(250 92 82 / 1)';
		case 'reference':
		case 'chataigne.module_endpoint':
			return 'rgb(163 133 235 / 1)';
		case 'css_value':
			return 'rgb(158 173 189 / 1)';
		case 'str':
		case 'string':
		case 'file':
		case 'enum':
			return 'rgb(235 173 66 / 1)';
		default:
			return undefined;
	}
};

const componentSpecs = (
	valueType: string | null | undefined
): { component: string; label: string }[] => {
	switch (valueType) {
		case 'vec2':
			return [
				{ component: 'x', label: 'X' },
				{ component: 'y', label: 'Y' }
			];
		case 'vec3':
			return [
				{ component: 'x', label: 'X' },
				{ component: 'y', label: 'Y' },
				{ component: 'z', label: 'Z' }
			];
		case 'color':
			return [
				{ component: 'r', label: 'R' },
				{ component: 'g', label: 'G' },
				{ component: 'b', label: 'B' },
				{ component: 'a', label: 'A' }
			];
		default:
			return [];
	}
};

const socketWithComponents = (
	id: string,
	label: string,
	valueType: string | undefined,
	color: string | undefined
): GraphSocket => {
	const children = componentSpecs(valueType).map(({ component, label }) => ({
		id: `${id}.${component}`,
		label,
		valueType: 'float',
		color: valueTypeColor('float'),
		parentId: id,
		component
	}));
	return {
		id,
		label,
		valueType,
		color,
		children: children.length > 0 ? children : undefined
	};
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
		const valueType = socketParameter(socket, nodesById, '/value_type') ?? undefined;
		const color = metadataColor(socket.meta.presentation?.color) ?? valueTypeColor(valueType);
		return [socketWithComponents(id, socket.meta.label, valueType, color)];
	});
};

const visibleConfigParameterCount = (
	anode: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): number => {
	const config = directChild(anode, nodesById, 'config');
	if (!config) return 0;
	return config.children.reduce((count, childId) => {
		const child = nodesById.get(childId);
		return child?.data.kind === 'parameter' &&
			child.meta.presentation?.show_in_inspector_content !== false
			? count + 1
			: count;
	}, 0);
};

const graphAutomaticSize = (
	inputs: readonly GraphSocket[],
	outputs: readonly GraphSocket[],
	configRows: number
): { width: number; height: number } => {
	const socketRows = Math.max(inputs.length, outputs.length, 1);
	const configHeight = configRows > 0 ? 0.35 + configRows * 1.95 : 0;
	return {
		width: 15,
		height: 2.35 + socketRows * 1.45 + configHeight + 0.45
	};
};

const collectSockets = (
	nodeId: string,
	sockets: readonly GraphSocket[],
	result: Map<string, GraphSocket>
): void => {
	for (const socket of sockets) {
		result.set(`${nodeId}:${socket.id}`, socket);
		if (socket.children) {
			collectSockets(nodeId, socket.children, result);
		}
	}
};

const formulaSocketsByRef = (
	formula: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): Map<string, GraphSocket> => {
	const result = new Map<string, GraphSocket>();
	for (const anode of formulaANodes(formula, nodesById)) {
		const nodeId = String(anode.node_id);
		collectSockets(
			nodeId,
			graphSockets(anode, nodesById, 'inputs', 'alchemist_input_socket'),
			result
		);
		collectSockets(
			nodeId,
			graphSockets(anode, nodesById, 'outputs', 'alchemist_output_socket'),
			result
		);
	}
	return result;
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
		const sizeParameter = parameterChild(anode, nodesById, 'size');
		const size =
			sizeParameter?.data.kind === 'parameter' && sizeParameter.meta.enabled
				? sizeParameter.data.param.value
				: null;
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
		const outputs = graphSockets(anode, nodesById, 'outputs', 'alchemist_output_socket');
		const configRows =
			propertyGetter || managerRef ? 0 : visibleConfigParameterCount(anode, nodesById);
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
			position: {
				x: position?.kind === 'vec2' ? position.value[0] : 0,
				y: position?.kind === 'vec2' ? position.value[1] : 0
			},
			size:
				size?.kind === 'vec2' && size.value[0] > 0 && size.value[1] > 0
					? { width: size.value[0], height: size.value[1] }
					: undefined,
			automaticSize: compactNode ? undefined : graphAutomaticSize(bodyInputs, outputs, configRows),
			resizable: !compactNode,
			invalid: typeId.length === 0,
			inputs: bodyInputs,
			outputs,
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
	const socketsByRef = formulaSocketsByRef(formula, nodesById);
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
				to: { nodeId: String(target), socketId: targetSocket },
				color: socketsByRef.get(`${source}:${sourceSocket}`)?.color,
				targetColor: socketsByRef.get(`${target}:${targetSocket}`)?.color
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
