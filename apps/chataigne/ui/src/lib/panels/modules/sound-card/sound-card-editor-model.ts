import {
	audioDeviceTargetKey,
	type AudioDeviceDescriptor,
	type AudioDeviceInspectorState,
	type AudioDirection,
	type PhysicalChannelDescriptor
} from 'golden_audio_ui';
import type { NodeId, ParamEventBehaviour, ParamValue, UiNodeDto } from 'golden_ui';

export interface SoundCardRouteRow {
	readonly id: NodeId;
	readonly label: string;
	readonly source: string;
	readonly destination: string;
	readonly gainDb: number | null;
}

export interface SoundCardRouteRecord extends SoundCardRouteRow {
	readonly sourceKey: string;
	readonly destinationKey: string;
	readonly sourceValue: ParamValue | null;
	readonly destinationValue: ParamValue | null;
	readonly gainParameterId: NodeId | null;
	readonly gainEventBehaviour: ParamEventBehaviour;
}

export interface SoundCardMatrixEndpoint {
	readonly key: string;
	readonly label: string;
	readonly value: ParamValue;
}

const declaredKey = (node: UiNodeDto): string => node.decl_id.split('/').at(-1) ?? node.decl_id;

export const childByDeclaredKey = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	parent: UiNodeDto,
	key: string
): UiNodeDto | null => {
	for (const childId of parent.children) {
		const child = nodes.get(childId);
		if (
			child &&
			(child.decl_id === key || declaredKey(child) === key || child.meta.short_name === key)
		) {
			return child;
		}
	}
	return null;
};

export const soundCardNodeAtPath = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	module: UiNodeDto,
	path: string
): UiNodeDto | null => {
	let current: UiNodeDto | null = module;
	for (const segment of path.split('/')) {
		if (!current) return null;
		current = childByDeclaredKey(nodes, current, segment);
	}
	return current;
};

export const descendantsByType = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	root: UiNodeDto | null,
	nodeType: string
): readonly UiNodeDto[] => {
	if (!root) return [];
	const matches: UiNodeDto[] = [];
	const pending = [...root.children];
	while (pending.length > 0) {
		const node = nodes.get(pending.pop()!);
		if (!node) continue;
		if (node.node_type === nodeType) matches.push(node);
		pending.push(...node.children);
	}
	return matches;
};

const parameterValue = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	node: UiNodeDto,
	key: string
): ParamValue | null => {
	const parameter = childByDeclaredKey(nodes, node, key);
	return parameter?.data.kind === 'parameter' ? parameter.data.param.value : null;
};

const parameterNode = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	node: UiNodeDto,
	key: string
): UiNodeDto | null => {
	const parameter = childByDeclaredKey(nodes, node, key);
	return parameter?.data.kind === 'parameter' ? parameter : null;
};

const referencedNodeLabel = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	value: Extract<ParamValue, { kind: 'reference' }>
): string => {
	const cached = value.cached_id !== undefined ? nodes.get(value.cached_id) : null;
	if (cached) return cached.meta.label;
	if (value.cached_name) return value.cached_name;
	return value.uuid ? `Missing (${value.uuid.slice(0, 8)})` : 'Unassigned';
};

const valueLabel = (nodes: ReadonlyMap<NodeId, UiNodeDto>, value: ParamValue | null): string => {
	if (!value) return 'Unassigned';
	switch (value.kind) {
		case 'reference':
			return referencedNodeLabel(nodes, value);
		case 'str':
		case 'file':
		case 'enum':
			return value.value || 'Unassigned';
		case 'int':
		case 'float':
			return String(value.value);
		case 'bool':
			return value.value ? 'Enabled' : 'Disabled';
		default:
			return 'Unassigned';
	}
};

const gainValue = (value: ParamValue | null): number | null =>
	value?.kind === 'float' || value?.kind === 'int' ? value.value : null;

export const soundCardParamValueKey = (value: ParamValue | null): string => {
	if (!value) return '';
	switch (value.kind) {
		case 'reference':
			return `reference:${value.uuid}`;
		case 'str':
		case 'file':
		case 'enum':
			return `${value.kind}:${value.value}`;
		case 'int':
		case 'float':
			return `${value.kind}:${value.value}`;
		case 'bool':
			return `bool:${value.value}`;
		default:
			return '';
	}
};

export const soundCardRouteRecords = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	root: UiNodeDto | null,
	routeType: string,
	sourceKey: string,
	destinationKey: string
): readonly SoundCardRouteRecord[] =>
	descendantsByType(nodes, root, routeType).map((route) => {
		const source = parameterValue(nodes, route, sourceKey);
		const destination = parameterValue(nodes, route, destinationKey);
		const gain = parameterNode(nodes, route, 'gain_db');
		return {
			id: route.node_id,
			label: route.meta.label,
			source: valueLabel(nodes, source),
			destination: valueLabel(nodes, destination),
			gainDb: gainValue(gain?.data.kind === 'parameter' ? gain.data.param.value : null),
			sourceKey: soundCardParamValueKey(source),
			destinationKey: soundCardParamValueKey(destination),
			sourceValue: source,
			destinationValue: destination,
			gainParameterId: gain?.node_id ?? null,
			gainEventBehaviour:
				gain?.data.kind === 'parameter' ? gain.data.param.event_behaviour : 'Coalesce'
		};
	});

export const soundCardRouteRows = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	root: UiNodeDto | null,
	routeType: string,
	sourceKey: string,
	destinationKey: string
): readonly SoundCardRouteRow[] =>
	soundCardRouteRecords(nodes, root, routeType, sourceKey, destinationKey).map(
		({ id, label, source, destination, gainDb }) => ({
			id,
			label,
			source,
			destination,
			gainDb
		})
	);

export const soundCardDirectChildrenByType = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	root: UiNodeDto | null,
	nodeType: string
): readonly UiNodeDto[] =>
	root?.children
		.map((childId) => nodes.get(childId))
		.filter((node): node is UiNodeDto => node?.node_type === nodeType) ?? [];

export const soundCardProfileKey = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	profile: UiNodeDto
): string =>
	valueLabel(nodes, parameterValue(nodes, profile, 'profile_key')) === 'Unassigned'
		? ''
		: valueLabel(nodes, parameterValue(nodes, profile, 'profile_key'));

export const soundCardVirtualChannelEndpoints = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	root: UiNodeDto | null,
	nodeType: 'sound_card_virtual_input' | 'sound_card_virtual_output'
): readonly SoundCardMatrixEndpoint[] =>
	descendantsByType(nodes, root, nodeType).map((node) => ({
		key: soundCardParamValueKey({ kind: 'reference', uuid: node.uuid }),
		label: node.meta.label,
		value: {
			kind: 'reference',
			uuid: node.uuid,
			cached_id: node.node_id,
			cached_name: node.meta.label
		}
	}));

const activeDevice = (
	state: AudioDeviceInspectorState,
	direction: AudioDirection
): AudioDeviceDescriptor | null => {
	const stream = direction === 'input' ? state.input : state.output;
	const targetKey = audioDeviceTargetKey(stream.active_target ?? stream.selected_target);
	return state.devices.find((device) => audioDeviceTargetKey(device.target) === targetKey) ?? null;
};

const physicalEndpoint = (
	channel: PhysicalChannelDescriptor,
	direction: AudioDirection
): SoundCardMatrixEndpoint => ({
	key: soundCardParamValueKey({ kind: 'str', value: channel.key }),
	label: channel.label || `${direction === 'input' ? 'Input' : 'Output'} ${channel.key}`,
	value: { kind: 'str', value: channel.key }
});

export const soundCardPhysicalChannelEndpoints = (
	state: AudioDeviceInspectorState,
	direction: AudioDirection
): readonly SoundCardMatrixEndpoint[] => {
	const device = activeDevice(state, direction);
	const channels =
		direction === 'input' ? (device?.input_channels ?? []) : (device?.output_channels ?? []);
	return channels.map((channel) => physicalEndpoint(channel, direction));
};

export const soundCardPlaybackSourceEndpoints = (
	channelLimit: number
): readonly SoundCardMatrixEndpoint[] =>
	Array.from({ length: Math.max(0, Math.floor(channelLimit)) }, (_, index) => {
		const channel = index + 1;
		const value: ParamValue = { kind: 'int', value: channel };
		return {
			key: soundCardParamValueKey(value),
			label: `Source ${channel}`,
			value
		};
	});

export const soundCardChannelLabels = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	root: UiNodeDto | null
): ReadonlyMap<string, string> => {
	const labels = new Map<string, string>();
	for (const nodeType of ['sound_card_virtual_input', 'sound_card_virtual_output']) {
		for (const node of descendantsByType(nodes, root, nodeType)) {
			labels.set(node.uuid, node.meta.label);
		}
	}
	return labels;
};

export const numericParameterAtPath = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	module: UiNodeDto,
	path: string
): number | null => {
	const node = soundCardNodeAtPath(nodes, module, path);
	if (node?.data.kind !== 'parameter') return null;
	const value = node.data.param.value;
	return value.kind === 'int' || value.kind === 'float' ? value.value : null;
};

export const stringParameterAtPath = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	module: UiNodeDto,
	path: string
): string | null => {
	const node = soundCardNodeAtPath(nodes, module, path);
	if (node?.data.kind !== 'parameter') return null;
	const value = node.data.param.value;
	return value.kind === 'str' || value.kind === 'enum' || value.kind === 'file'
		? value.value
		: null;
};
