import {
	audioDeviceTargetKey,
	type AudioDeviceDescriptor,
	type AudioDeviceInspectorState,
	type AudioDirection,
	type AudioRoutingPatchConnection,
	type AudioRoutingPatchEndpoint
} from 'golden_audio_ui';
import type { NodeId, ParamValue, UiNodeDto } from 'golden_ui';

export interface SoundCardRoutingConnection extends AudioRoutingPatchConnection {
	readonly physicalChannel: string;
	readonly appChannelUuid: string;
}

export interface SoundCardRoutingProjection {
	readonly moduleId: NodeId;
	readonly moduleUuid: string;
	readonly direction: AudioDirection;
	readonly sources: readonly AudioRoutingPatchEndpoint[];
	readonly destinations: readonly AudioRoutingPatchEndpoint[];
	readonly connections: readonly SoundCardRoutingConnection[];
}

const declaredKey = (node: UiNodeDto): string => node.decl_id.split('/').at(-1) ?? node.decl_id;

export const soundCardChildByKey = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	parent: UiNodeDto | null,
	key: string
): UiNodeDto | null => {
	if (!parent) return null;
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
	root: UiNodeDto | null,
	path: string
): UiNodeDto | null => {
	let current = root;
	for (const key of path.split('/')) {
		current = soundCardChildByKey(nodes, current, key);
		if (!current) return null;
	}
	return current;
};

export const soundCardAncestorByType = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	parentById: ReadonlyMap<NodeId, NodeId>,
	node: UiNodeDto,
	nodeType: string
): UiNodeDto | null => {
	let current: UiNodeDto | undefined = node;
	while (current) {
		if (current.node_type === nodeType) return current;
		const parentId = parentById.get(current.node_id);
		current = parentId === undefined ? undefined : nodes.get(parentId);
	}
	return null;
};

const directChildrenByType = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	parent: UiNodeDto | null,
	nodeType: string
): readonly UiNodeDto[] =>
	parent?.children
		.map((childId) => nodes.get(childId))
		.filter((child): child is UiNodeDto => child?.node_type === nodeType) ?? [];

const parameterValue = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	parent: UiNodeDto,
	key: string
): ParamValue | null => {
	const child = soundCardChildByKey(nodes, parent, key);
	return child?.data.kind === 'parameter' ? child.data.param.value : null;
};

const referenceUuid = (value: ParamValue | null): string | null =>
	value?.kind === 'reference' && value.uuid ? value.uuid : null;

const stringValue = (value: ParamValue | null): string | null => {
	if (value?.kind === 'str' || value?.kind === 'enum') return value.value;
	return null;
};

export const soundCardDirectionConfigured = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	module: UiNodeDto | null,
	direction: AudioDirection
): boolean => {
	const connection = soundCardChildByKey(nodes, module, 'connection');
	if (!connection) return false;
	const driver = stringValue(parameterValue(nodes, connection, 'audio_driver'));
	const devices = [
		stringValue(parameterValue(nodes, connection, 'device')),
		stringValue(parameterValue(nodes, connection, `${direction}_device`))
	];
	return (
		driver !== null && driver !== 'none' && devices.some((device) => device && device !== 'none')
	);
};

const selectedDevice = (
	state: AudioDeviceInspectorState | null,
	direction: AudioDirection
): AudioDeviceDescriptor | null => {
	if (!state) return null;
	const stream = direction === 'input' ? state.input : state.output;
	const target = stream.active_target ?? stream.selected_target;
	if (!target) return null;
	if (target.kind === 'system_default') {
		return (
			state.devices.find(
				(device) =>
					device.target.backend === target.backend &&
					(direction === 'input' ? device.is_system_default_input : device.is_system_default_output)
			) ?? null
		);
	}
	const key = audioDeviceTargetKey(target);
	return state.devices.find((device) => audioDeviceTargetKey(device.target) === key) ?? null;
};

const physicalEndpoints = (
	state: AudioDeviceInspectorState | null,
	direction: AudioDirection
): readonly AudioRoutingPatchEndpoint[] => {
	const device = selectedDevice(state, direction);
	const channels =
		direction === 'input' ? (device?.input_channels ?? []) : (device?.output_channels ?? []);
	return channels.map((channel) => ({
		id: channel.key,
		label: channel.label || channel.key
	}));
};

const appChannelEndpoints = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	module: UiNodeDto,
	direction: AudioDirection
): readonly AudioRoutingPatchEndpoint[] => {
	const list = soundCardNodeAtPath(nodes, module, `parameters/${direction}/channels`);
	const channels =
		list?.children
			.map((childId) => nodes.get(childId))
			.filter(
				(channel): channel is UiNodeDto =>
					channel?.data.kind === 'parameter' && channel.data.param.value.kind === 'float'
			) ?? [];
	return channels.map((channel) => ({
		id: channel.uuid,
		label: channel.meta.short_name,
		editable: true
	}));
};

const routeConnections = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	routing: UiNodeDto,
	direction: AudioDirection
): readonly SoundCardRoutingConnection[] => {
	const routes = soundCardChildByKey(nodes, routing, 'routes');
	const routeType = direction === 'input' ? 'sound_card_input_route' : 'sound_card_output_route';
	return directChildrenByType(nodes, routes, routeType).flatMap((route) => {
		const physicalChannel = stringValue(parameterValue(nodes, route, 'physical_channel'));
		const appChannelUuid = referenceUuid(parameterValue(nodes, route, 'channel'));
		if (!physicalChannel || !appChannelUuid) return [];
		return [
			{
				id: route.uuid,
				sourceId: direction === 'input' ? physicalChannel : appChannelUuid,
				destinationId: direction === 'input' ? appChannelUuid : physicalChannel,
				physicalChannel,
				appChannelUuid
			}
		];
	});
};

export const projectSoundCardRouting = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	parentById: ReadonlyMap<NodeId, NodeId>,
	routing: UiNodeDto,
	deviceState: AudioDeviceInspectorState | null
): SoundCardRoutingProjection | null => {
	const direction: AudioDirection | null =
		routing.node_type === 'sound_card_input_routing'
			? 'input'
			: routing.node_type === 'sound_card_output_routing'
				? 'output'
				: null;
	if (direction === null) return null;
	const module = soundCardAncestorByType(nodes, parentById, routing, 'sound_card_module');
	if (!module) return null;
	const appChannels = appChannelEndpoints(nodes, module, direction);
	const physicalChannels = physicalEndpoints(deviceState, direction);
	return {
		moduleId: module.node_id,
		moduleUuid: module.uuid,
		direction,
		sources: direction === 'input' ? physicalChannels : appChannels,
		destinations: direction === 'input' ? appChannels : physicalChannels,
		connections: routeConnections(nodes, routing, direction)
	};
};
