import type { SoundCardUiControlRequest } from '../generated';
import {
	createSoundCardRoutingBinding,
	resolveSoundCardModuleId,
	type SoundCardRoutingControlPort
} from '../sound-card-routing-adapter.svelte';
import type { SoundCardRoutingProjection } from '../sound-card-routing-model';
import type { UiNodeDto } from 'golden_ui';
import { describe, expect, it, vi } from 'vitest';

const projection = (direction: 'input' | 'output'): SoundCardRoutingProjection => ({
	moduleId: 41,
	moduleUuid: 'sound-card-module',
	direction,
	sources: [],
	destinations: [],
	connections: [
		{
			id: 'route-a',
			sourceId: direction === 'input' ? 'device:0' : 'channel-a',
			destinationId: direction === 'input' ? 'channel-a' : 'device:0',
			physicalChannel: 'device:0',
			appChannelUuid: 'channel-a'
		}
	]
});

const createPort = () => {
	const calls: Array<[number, string, SoundCardUiControlRequest]> = [];
	const send = vi.fn(
		async (moduleId: number, moduleUuid: string, request: SoundCardUiControlRequest) => {
			calls.push([moduleId, moduleUuid, request]);
			return true;
		}
	);
	const port: SoundCardRoutingControlPort = { send };
	return { calls, port, send };
};

describe('Sound Card routing adapter', () => {
	it('resolves a stale transient module id through the persisted module uuid', () => {
		const module = {
			node_id: 99,
			uuid: 'sound-card-module',
			node_type: 'sound_card_module'
		} as UiNodeDto;
		const nodes = new Map([[module.node_id, module]]);

		expect(resolveSoundCardModuleId(nodes, 41, 'sound-card-module')).toBe(99);
		expect(resolveSoundCardModuleId(nodes, 41, 'missing-module')).toBeNull();
	});

	it('maps input patch interactions to generated backend controls', async () => {
		const fixture = createPort();
		const binding = createSoundCardRoutingBinding(projection('input'), fixture.port);

		await binding.connect('device:1', 'channel-b');
		await binding.disconnect('route-a');
		await binding.renameEndpoint('destination', 'channel-b', 'Voice');

		expect(fixture.calls).toEqual([
			[
				41,
				'sound-card-module',
				{
					kind: 'connect_route',
					direction: 'input',
					physical_channel: 'device:1',
					app_channel_uuid: 'channel-b'
				}
			],
			[
				41,
				'sound-card-module',
				{
					kind: 'disconnect_route',
					direction: 'input',
					physical_channel: 'device:0',
					app_channel_uuid: 'channel-a'
				}
			],
			[
				41,
				'sound-card-module',
				{
					kind: 'rename_channel',
					direction: 'input',
					app_channel_uuid: 'channel-b',
					label: 'Voice'
				}
			]
		]);
	});

	it('preserves output orientation and rejects an unknown connection locally', async () => {
		const fixture = createPort();
		const binding = createSoundCardRoutingBinding(projection('output'), fixture.port);

		expect(await binding.connect('channel-c', 'device:2')).toBe(true);
		expect(await binding.disconnect('missing-route')).toBe(false);
		expect(fixture.calls).toEqual([
			[
				41,
				'sound-card-module',
				{
					kind: 'connect_route',
					direction: 'output',
					physical_channel: 'device:2',
					app_channel_uuid: 'channel-c'
				}
			]
		]);
	});

	it('reports dispatch failures and rejects renames on the physical side', async () => {
		const port: SoundCardRoutingControlPort = {
			send: vi.fn(async () => false)
		};
		const binding = createSoundCardRoutingBinding(projection('input'), port);

		expect(await binding.renameEndpoint('destination', 'channel-a', 'Rejected')).toBe(false);
		expect(await binding.renameEndpoint('source', 'device:0', 'Physical')).toBe(false);
	});
});
