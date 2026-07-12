import type {
	GraphConnectionRequest,
	GraphEdge,
	GraphNode,
	GraphNodeBypassConnection,
	GraphSocket
} from 'golden_alchemist_ui';
import type { NodeId, ParamValue, UiColorDto, UiCreatableUserItem, UiNodeDto } from 'golden_ui';

export const FORMULA_NODE_TYPE = 'alchemist_formula';
export const ANODE_NODE_TYPE = 'alchemist_anode';
export const CONNECTION_NODE_TYPE = 'alchemist_connection';
export const ANODE_CREATE_PREFIX = 'alchemist_anode:';
export const PROPERTIES_DECL_ID = 'properties';
export const PROPERTY_NODE_TYPE = 'alchemist_property';
export const PROPERTY_MANAGER_NODE_TYPE = 'alchemist_property_manager';
export const PROPERTY_FOLDER_NODE_TYPE = 'alchemist_property_folder';

export const ANODE_TYPE_TAG_PREFIX = 'alchemist.anode.type:';
export const VALUE_TYPE_CONFIG_DECL_ID = 'config/value_type';

// A node's value-type selector is rendered in the header (not the body). This is
// either the dedicated `config/value_type` field (Add/Clamp/MapRange/...) or the
// implicit type selector that backs a `runtime_value` config field, whose decl id
// is `config/<field>__type` (e.g. the Constant node's `config/value__type`).
export const isValueTypeConfigDecl = (declId: string | null | undefined): boolean =>
	declId === VALUE_TYPE_CONFIG_DECL_ID ||
	(typeof declId === 'string' && declId.startsWith('config/') && declId.endsWith('__type'));

export const ROUTING_ANODE_TYPE = 'chataigne.routing';

const ROUTING_NODE_SIZE = { width: 5.5, height: 2.7 };

const stableHash = (value: string): number => {
	let hash = 2_166_136_261;
	for (let index = 0; index < value.length; index += 1) {
		hash ^= value.charCodeAt(index) & 0xff;
		hash = Math.imul(hash, 16_777_619) >>> 0;
	}
	return hash >>> 0;
};

const familyHue = (family: string): number => {
	switch (family) {
		case 'Number':
			return 211;
		case 'Geometry':
			return 188;
		case 'String':
		case 'Values':
			return 42;
		case 'Chataigne':
			return 326;
		case 'Logic':
			return 268;
		case 'Flow':
			return 158;
		case 'Debug':
			return 14;
		default:
			return stableHash(family) % 360;
	}
};

const hslToCssRgb = (hue: number, saturation: number, lightness: number): string => {
	const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation;
	const huePrime = hue / 60;
	const x = chroma * (1 - Math.abs((huePrime % 2) - 1));
	let r1 = 0;
	let g1 = 0;
	let b1 = 0;
	if (huePrime < 1) {
		r1 = chroma;
		g1 = x;
	} else if (huePrime < 2) {
		r1 = x;
		g1 = chroma;
	} else if (huePrime < 3) {
		g1 = chroma;
		b1 = x;
	} else if (huePrime < 4) {
		g1 = x;
		b1 = chroma;
	} else if (huePrime < 5) {
		r1 = x;
		b1 = chroma;
	} else {
		r1 = chroma;
		b1 = x;
	}
	const m = lightness - chroma / 2;
	const r = Math.round(clamp01(r1 + m) * 255);
	const g = Math.round(clamp01(g1 + m) * 255);
	const b = Math.round(clamp01(b1 + m) * 255);
	return `rgb(${r} ${g} ${b} / 1)`;
};

export const anodeCategoryColor = (family: string): string =>
	hslToCssRgb(familyHue(family), 0.66, 0.54);

export const anodeDefaultColor = (family: string, typeId: string): string => {
	if (family === 'Routing' || typeId === ROUTING_ANODE_TYPE) {
		return 'rgb(117 122 128 / 1)';
	}
	const variation = stableHash(typeId);
	const hue = (familyHue(family) + (variation % 25) + 348) % 360;
	const saturation = (62 + ((variation >>> 8) % 16)) / 100;
	const lightness = (48 + ((variation >>> 16) % 12)) / 100;
	return hslToCssRgb(hue, saturation, lightness);
};

const clamp01 = (value: number): number => Math.min(1, Math.max(0, value));

const metadataColor = (color: UiColorDto | null | undefined): string | undefined => {
	if (!color) return undefined;
	const r = Math.round(clamp01(color.r) * 255);
	const g = Math.round(clamp01(color.g) * 255);
	const b = Math.round(clamp01(color.b) * 255);
	return `rgb(${r} ${g} ${b} / ${clamp01(color.a)})`;
};

const presentationColor = (
	presentation: UiNodeDto['meta']['presentation'] | null | undefined
): string | undefined => metadataColor(presentation?.color ?? presentation?.default_color);

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
		case 'value_array':
			return 'rgb(130 145 160 / 1)';
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

const normalizeGraphValueType = (typeId: string | null | undefined): string | null => {
	const value = typeId?.trim();
	if (!value) return null;
	switch (value) {
		case 'str':
		case 'file':
		case 'enum':
		case 'css_value':
			return 'string';
		default:
			return value;
	}
};

const CONVERTIBLE_VALUE_TYPES: ReadonlyMap<string, ReadonlySet<string>> = new Map(
	Object.entries({
		unit: ['bool', 'int', 'float', 'string', 'vec2', 'vec3', 'color', 'duration', 'value_array'],
		bool: ['unit', 'int', 'float', 'string', 'vec2', 'vec3', 'color', 'duration', 'value_array'],
		trigger: [
			'unit',
			'bool',
			'int',
			'float',
			'string',
			'vec2',
			'vec3',
			'color',
			'duration',
			'value_array'
		],
		int: ['unit', 'bool', 'float', 'string', 'vec2', 'vec3', 'color', 'duration', 'value_array'],
		float: ['unit', 'bool', 'int', 'string', 'vec2', 'vec3', 'color', 'duration', 'value_array'],
		string: ['unit', 'bool', 'int', 'float', 'vec2', 'vec3', 'color', 'duration', 'value_array'],
		vec2: ['unit', 'bool', 'int', 'float', 'string', 'vec3', 'color', 'duration', 'value_array'],
		vec3: ['unit', 'bool', 'int', 'float', 'string', 'vec2', 'color', 'duration', 'value_array'],
		color: ['unit', 'bool', 'int', 'float', 'string', 'vec2', 'vec3', 'duration', 'value_array'],
		duration: ['unit', 'bool', 'int', 'float', 'string', 'vec2', 'vec3', 'color', 'value_array'],
		value_array: ['string']
	}).map(([source, targets]) => [source, new Set(targets)])
);

export const canCoerceGraphValueType = (
	fromType: string | null | undefined,
	toType: string | null | undefined
): boolean => {
	const from = normalizeGraphValueType(fromType);
	const to = normalizeGraphValueType(toType);
	if (!from || !to) return true;
	return from === to || (CONVERTIBLE_VALUE_TYPES.get(from)?.has(to) ?? false);
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
	color: string | undefined,
	defaultParamId?: string
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
		defaultParamId,
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
	declId: string
): string | null => {
	for (const childId of socket.children) {
		const child = nodesById.get(childId);
		if (
			child?.decl_id === declId &&
			child.data.kind === 'parameter' &&
			child.data.param.value.kind === 'str'
		) {
			return child.data.param.value.value;
		}
	}
	return null;
};

const socketDefaultParamId = (
	socket: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	direction: 'inputs' | 'outputs',
	socketId: string
): string | undefined => {
	if (direction !== 'inputs') return undefined;
	const declId = `${direction}/${socketId}/value`;
	for (const childId of socket.children) {
		const child = nodesById.get(childId);
		if (child?.decl_id === declId && child.data.kind === 'parameter') {
			return String(child.node_id);
		}
	}
	return undefined;
};

const graphSockets = (
	anode: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	folderDeclId: 'inputs' | 'outputs',
	socketNodeType: 'alchemist_input_socket' | 'alchemist_output_socket'
): GraphSocket[] => {
	const folder = directChild(anode, nodesById, folderDeclId);
	if (!folder) return [];
	const socketIds = new Set<string>();
	return folder.children.flatMap((childId) => {
		const socket = nodesById.get(childId);
		if (!socket || socket.node_type !== socketNodeType) return [];
		const declId = socket.decl_id;
		if (!declId.startsWith(`${folderDeclId}/`)) return [];
		const id = socketParameter(socket, nodesById, `${declId}/socket_id`);
		if (!id) return [];
		if (socketIds.has(id)) return [];
		socketIds.add(id);
		const valueType = socketParameter(socket, nodesById, `${declId}/value_type`) ?? undefined;
		const color = presentationColor(socket.meta.presentation) ?? valueTypeColor(valueType);
		const defaultParamId = socketDefaultParamId(socket, nodesById, folderDeclId, id);
		return [socketWithComponents(id, socket.meta.label, valueType, color, defaultParamId)];
	});
};

const socketLabelWidth = (socket: GraphSocket): number =>
	1.25 + Math.max(1.2, socket.label.length * 0.36);

const socketDefaultEditorWidth = (socket: GraphSocket): number => {
	if (!socket.defaultParamId) return 0;
	switch (normalizeGraphValueType(socket.valueType)) {
		case 'vec3':
		case 'color':
			return 8.2;
		case 'vec2':
			return 6.6;
		case 'bool':
			return 1.4;
		case 'int':
		case 'float':
		case 'duration':
			return 8.8;
		default:
			return 5.4;
	}
};

const outputSocketWidth = (socket: GraphSocket): number => socketLabelWidth(socket) + 0.85;

const maxSocketWidth = (
	sockets: readonly GraphSocket[],
	measure: (socket: GraphSocket) => number
): number =>
	sockets.reduce(
		(width, socket) =>
			Math.max(width, measure(socket), maxSocketWidth(socket.children ?? [], measure)),
		0
	);

const alignedInputSocketWidth = (sockets: readonly GraphSocket[]): number => {
	if (sockets.length === 0) return 0;
	const labelWidth = maxSocketWidth(sockets, socketLabelWidth);
	const defaultWidth = maxSocketWidth(sockets, socketDefaultEditorWidth);
	return labelWidth + 0.85 + defaultWidth;
};

const graphAutomaticSize = (
	inputs: readonly GraphSocket[],
	outputs: readonly GraphSocket[],
	configRows: number
): { width: number; height: number } => {
	const socketRows = Math.max(inputs.length, outputs.length, 1);
	const configHeight = configRows > 0 ? 0.35 + configRows * 1.95 : 0;
	const inputWidth = alignedInputSocketWidth(inputs);
	const outputWidth = maxSocketWidth(outputs, outputSocketWidth);
	const socketGap = inputWidth > 0 && outputWidth > 0 ? 0.5 : 0;
	return {
		width: Math.min(28, Math.max(9.5, inputWidth + outputWidth + socketGap + 0.6)),
		height: 2.05 + socketRows * 1.35 + configHeight + 0.25
	};
};

const disabledBypassConnections = (
	enabled: boolean,
	inputs: readonly GraphSocket[],
	outputs: readonly GraphSocket[]
): GraphNodeBypassConnection[] | undefined => {
	if (enabled || inputs.length !== 1 || outputs.length !== 1) {
		return undefined;
	}
	const input = inputs[0];
	const output = outputs[0];
	const inputType = normalizeGraphValueType(input.valueType);
	const outputType = normalizeGraphValueType(output.valueType);
	if (!inputType || inputType !== outputType) {
		return undefined;
	}
	return [
		{
			inputSocketId: input.id,
			outputSocketId: output.id,
			color: output.color ?? input.color
		}
	];
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

export const graphSocketsByRef = (nodes: readonly GraphNode[]): Map<string, GraphSocket> => {
	const result = new Map<string, GraphSocket>();
	for (const node of nodes) {
		collectSockets(node.id, node.inputs, result);
		collectSockets(node.id, node.outputs, result);
	}
	return result;
};

export const canConnectGraphSockets = (
	source: GraphSocket | null | undefined,
	target: GraphSocket | null | undefined
): boolean => {
	if (!source || !target) return false;
	if (source.compatible === false || target.compatible === false) return false;
	return canCoerceGraphValueType(source.valueType, target.valueType);
};

export const canConnectGraphConnection = (
	nodes: readonly GraphNode[],
	connection: GraphConnectionRequest
): boolean => {
	const socketsByRef = graphSocketsByRef(nodes);
	return canConnectGraphSockets(
		socketsByRef.get(`${connection.from.nodeId}:${connection.from.socketId}`),
		socketsByRef.get(`${connection.to.nodeId}:${connection.to.socketId}`)
	);
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

const nodeWarning = (node: UiNodeDto): string | undefined => {
	const warning = node.meta.presentation?.warnings?.[0];
	if (!warning) return undefined;
	return warning.detail?.trim() || warning.message;
};

export const toGraphNodes = (
	formula: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	_catalogItems: readonly UiCreatableUserItem[] = []
): GraphNode[] => {
	return formulaANodes(formula, nodesById).map((anode) => {
		const position = parameterValue(anode, nodesById, 'position');
		const sizeParameter = parameterChild(anode, nodesById, 'size');
		const size =
			sizeParameter?.data.kind === 'parameter' && sizeParameter.meta.enabled
				? sizeParameter.data.param.value
				: null;
		const typeId = anodeType(anode);
		const routingNode = typeId === ROUTING_ANODE_TYPE;
		const compactNode = routingNode;
		const description = anode.meta.description?.trim();
		const inputs = graphSockets(anode, nodesById, 'inputs', 'alchemist_input_socket');
		const outputs = graphSockets(anode, nodesById, 'outputs', 'alchemist_output_socket');
		const configRows = 0;
		const warning = nodeWarning(anode);
		return {
			id: String(anode.node_id),
			label: routingNode ? '' : anode.meta.label,
			subtitle: undefined,
			description: description ? description : undefined,
			canRename: anode.meta.user_permissions?.can_edit_name !== false,
			collapsed: anode.meta.presentation?.collapsed === true,
			enabled: anode.meta.enabled,
			canDisable: anode.meta.can_be_disabled,
			color: presentationColor(anode.meta.presentation),
			position: {
				x: position?.kind === 'vec2' ? position.value[0] : 0,
				y: position?.kind === 'vec2' ? position.value[1] : 0
			},
			size:
				!routingNode && size?.kind === 'vec2' && size.value[0] > 0 && size.value[1] > 0
					? { width: size.value[0], height: size.value[1] }
					: undefined,
			automaticSize: routingNode
				? ROUTING_NODE_SIZE
				: compactNode
					? undefined
					: graphAutomaticSize(inputs, outputs, configRows),
			resizable: !compactNode,
			invalid: typeId.length === 0 || warning !== undefined,
			warning,
			inputs,
			outputs,
			bypassConnections: disabledBypassConnections(anode.meta.enabled, inputs, outputs)
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
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	activeSocketRefs: ReadonlySet<string> = new Set()
): GraphEdge[] => {
	const socketsByRef = formulaSocketsByRef(formula, nodesById);
	const activeNodeIds = new Set(
		Array.from(activeSocketRefs).flatMap((ref) => {
			const separator = ref.indexOf(':');
			return separator > 0 ? [ref.slice(0, separator)] : [];
		})
	);
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
				targetColor: socketsByRef.get(`${target}:${targetSocket}`)?.color,
				active:
					activeSocketRefs.has(`${source}:${sourceSocket}`) ||
					activeSocketRefs.has(`${target}:${targetSocket}`) ||
					activeSocketRefs.has(String(connection.node_id)) ||
					(activeNodeIds.has(String(source)) && activeNodeIds.has(String(target)))
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

export const bodyConfigParameters = (
	anode: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): UiNodeDto[] =>
	configParameters(anode, nodesById).filter(
		(parameter) => !isValueTypeConfigDecl(parameter.decl_id)
	);

export const valueTypeConfigParameter = (
	anode: UiNodeDto,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): UiNodeDto | null =>
	configParameters(anode, nodesById).find((parameter) =>
		isValueTypeConfigDecl(parameter.decl_id)
	) ?? null;
