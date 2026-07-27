import type { AudioRoutingPatchBinding } from 'golden_audio_ui';
import type { NodeId, UiNodeDto } from 'golden_ui';
import { appState } from 'golden_ui/store/workbench.svelte';
import { SOUND_CARD_UI_CONTROL_TOPIC, type SoundCardUiControlRequest } from './generated';
import type { SoundCardRoutingProjection } from './sound-card-routing-model';

export interface SoundCardRoutingControlPort {
	/** Reports transport dispatch only; graph snapshots carry admitted state. */
	send(moduleId: number, moduleUuid: string, request: SoundCardUiControlRequest): Promise<boolean>;
}

export const resolveSoundCardModuleId = (
	nodes: ReadonlyMap<NodeId, UiNodeDto>,
	projectedId: NodeId,
	moduleUuid: string
): NodeId | null => {
	const projected = nodes.get(projectedId);
	if (projected?.uuid === moduleUuid && projected.node_type === 'sound_card_module') {
		return projected.node_id;
	}
	return (
		[...nodes.values()].find(
			(candidate) => candidate.uuid === moduleUuid && candidate.node_type === 'sound_card_module'
		)?.node_id ?? null
	);
};

const defaultPort: SoundCardRoutingControlPort = {
	async send(moduleId, moduleUuid, request) {
		const session = appState.session;
		if (!session) return false;
		const nodes = session.graph.state.nodesById;
		const liveModuleId = resolveSoundCardModuleId(nodes, moduleId, moduleUuid);
		if (liveModuleId === null) return false;
		try {
			await session.sendIntent({
				kind: 'sendNodeEvent',
				node: liveModuleId,
				topic: SOUND_CARD_UI_CONTROL_TOPIC,
				payload: request
			});
			return true;
		} catch (error) {
			console.error('failed to send Sound Card routing control', request, error);
			return false;
		}
	}
};

export const createSoundCardRoutingBinding = (
	projection: SoundCardRoutingProjection,
	port: SoundCardRoutingControlPort = defaultPort
): AudioRoutingPatchBinding => ({
	connect(sourceId, destinationId) {
		const physicalChannel = projection.direction === 'input' ? sourceId : destinationId;
		const appChannelUuid = projection.direction === 'input' ? destinationId : sourceId;
		return port.send(projection.moduleId, projection.moduleUuid, {
			kind: 'connect_route',
			direction: projection.direction,
			physical_channel: physicalChannel,
			app_channel_uuid: appChannelUuid
		});
	},
	disconnect(connectionId) {
		const connection = projection.connections.find((candidate) => candidate.id === connectionId);
		if (!connection) return Promise.resolve(false);
		return port.send(projection.moduleId, projection.moduleUuid, {
			kind: 'disconnect_route',
			direction: projection.direction,
			physical_channel: connection.physicalChannel,
			app_channel_uuid: connection.appChannelUuid
		});
	},
	renameEndpoint(side, endpointId, label) {
		const logicalSide = projection.direction === 'input' ? 'destination' : 'source';
		if (side !== logicalSide) return Promise.resolve(false);
		return port.send(projection.moduleId, projection.moduleUuid, {
			kind: 'rename_channel',
			direction: projection.direction,
			app_channel_uuid: endpointId,
			label
		});
	}
});
