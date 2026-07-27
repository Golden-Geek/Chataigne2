import { createMockAudioDeviceState } from 'golden_audio_ui/mock';
import type { NodeId, ParamValue, UiNodeDto } from 'golden_ui';
import { describe, expect, it } from 'vitest';
import { projectSoundCardRouting, soundCardDirectionConfigured } from '../sound-card-routing-model';

const node = (
	id: NodeId,
	nodeType: string,
	label: string,
	children: NodeId[] = [],
	declId = nodeType,
	value?: ParamValue,
	uuid = `uuid-${id}`
): UiNodeDto =>
	({
		node_id: id,
		uuid,
		decl_id: declId,
		node_type: nodeType,
		meta: { label, short_name: declId.split('/').at(-1) ?? declId },
		data: value
			? {
					kind: 'parameter',
					param: { value }
				}
			: { kind: 'node', node_type: nodeType },
		children
	}) as UiNodeDto;

describe('Sound Card routing projection', () => {
	it('keeps direction visibility tied to authored device choices during recovery', () => {
		const driver = node(3, 'enum', 'Audio Driver', [], 'audio_driver', {
			kind: 'enum',
			value: 'wasapi'
		});
		const inputDevice = node(4, 'enum', 'Input Device', [], 'input_device', {
			kind: 'enum',
			value: 'none'
		});
		const outputDevice = node(5, 'enum', 'Output Device', [], 'output_device', {
			kind: 'enum',
			value: 'platform_default:system_default:output'
		});
		const device = node(6, 'enum', 'Device', [], 'device', {
			kind: 'enum',
			value: 'none'
		});
		const connection = node(2, 'folder', 'Connection', [3, 4, 5, 6], 'connection');
		const module = node(1, 'sound_card_module', 'Sound Card', [2]);
		const nodes = new Map(
			[module, connection, driver, device, inputDevice, outputDevice].map((item) => [
				item.node_id,
				item
			])
		);

		expect(soundCardDirectionConfigured(nodes, module, 'input')).toBe(false);
		expect(soundCardDirectionConfigured(nodes, module, 'output')).toBe(true);

		driver.data =
			driver.data.kind === 'parameter'
				? {
						...driver.data,
						param: { ...driver.data.param, value: { kind: 'enum', value: 'none' } }
					}
				: driver.data;
		expect(soundCardDirectionConfigured(nodes, module, 'output')).toBe(false);

		driver.data =
			driver.data.kind === 'parameter'
				? {
						...driver.data,
						param: { ...driver.data.param, value: { kind: 'enum', value: 'asio' } }
					}
				: driver.data;
		device.data =
			device.data.kind === 'parameter'
				? {
						...device.data,
						param: { ...device.data.param, value: { kind: 'enum', value: 'asio-device' } }
					}
				: device.data;
		expect(soundCardDirectionConfigured(nodes, module, 'input')).toBe(true);
		expect(soundCardDirectionConfigured(nodes, module, 'output')).toBe(true);
	});

	it('projects backend-owned input channels and routes without creating defaults', () => {
		const module = node(1, 'sound_card_module', 'Sound Card', [2, 6]);
		const connection = node(2, 'folder', 'Connection', [3], 'connection');
		const routing = node(3, 'sound_card_input_routing', 'Input Routing', [4], 'input_routing');
		const routes = node(4, 'sound_card_input_route_list', 'Routes', [5], 'routes');
		const route = node(5, 'sound_card_input_route', 'Route', [10, 11], 'route_1');
		const parameters = node(6, 'folder', 'Parameters', [7], 'parameters');
		const input = node(7, 'sound_card_input_parameters', 'Input', [8], 'input');
		const channels = node(8, 'sound_card_input_channel_list', 'Channels', [9], 'channels');
		const channel = node(
			9,
			'float',
			'Microphone',
			[],
			'input_1',
			{ kind: 'float', value: 0 },
			'input-channel'
		);
		const physical = node(10, 'string', 'Physical Channel', [], 'physical_channel', {
			kind: 'str',
			value: 'input:0'
		});
		const target = node(11, 'reference', 'Channel', [], 'channel', {
			kind: 'reference',
			uuid: 'input-channel'
		});
		const nodes = new Map(
			[
				module,
				connection,
				routing,
				routes,
				route,
				parameters,
				input,
				channels,
				channel,
				physical,
				target
			].map((item) => [item.node_id, item])
		);
		const parentById = new Map<NodeId, NodeId>([
			[2, 1],
			[3, 2],
			[4, 3],
			[5, 4],
			[6, 1],
			[7, 6],
			[8, 7],
			[9, 8],
			[10, 5],
			[11, 5]
		]);

		const projection = projectSoundCardRouting(
			nodes,
			parentById,
			routing,
			createMockAudioDeviceState()
		);

		expect(projection).toMatchObject({
			moduleId: 1,
			moduleUuid: 'uuid-1',
			direction: 'input',
			sources: [
				{ id: 'input:0', label: 'Input 1' },
				{ id: 'input:1', label: 'Input 2' }
			],
			destinations: [{ id: 'input-channel', label: 'Microphone', editable: true }],
			connections: [
				{
					sourceId: 'input:0',
					destinationId: 'input-channel',
					physicalChannel: 'input:0',
					appChannelUuid: 'input-channel'
				}
			]
		});
	});

	it('does not invent physical endpoints when the selected device is absent', () => {
		const module = node(1, 'sound_card_module', 'Sound Card', [2]);
		const routing = node(2, 'sound_card_output_routing', 'Output Routing', [], 'output_routing');
		const nodes = new Map([
			[1, module],
			[2, routing]
		]);
		const state = createMockAudioDeviceState();
		state.output = {
			...state.output,
			selected_target: null,
			active_target: null,
			enabled: false
		};

		expect(projectSoundCardRouting(nodes, new Map([[2, 1]]), routing, state)?.destinations).toEqual(
			[]
		);
	});
});
