import { describe, expect, it } from 'vitest';
import { createMockAudioDeviceState } from 'golden_audio_ui/mock';
import type { NodeId, ParamValue, UiNodeDto } from 'golden_ui';
import {
	childByDeclaredKey,
	soundCardChannelLabels,
	soundCardNodeAtPath,
	soundCardPhysicalChannelEndpoints,
	soundCardPlaybackSourceEndpoints,
	soundCardRouteRecords,
	soundCardRouteRows
} from '../sound-card-editor-model';

const node = (
	id: NodeId,
	nodeType: string,
	label: string,
	children: NodeId[] = [],
	declId = nodeType,
	value?: ParamValue
): UiNodeDto =>
	({
		node_id: id,
		uuid: `uuid-${id}`,
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

describe('Sound Card editor model', () => {
	it('resolves declared paths without depending on display labels', () => {
		const root = node(1, 'sound_card_module', 'Renamed Sound Card', [2], 'sound_card');
		const parameters = node(2, 'folder', 'Parameters', [3], 'sound_card/parameters');
		const outputs = node(
			3,
			'sound_card_virtual_output_list',
			'Renamed Outputs',
			[],
			'virtual_outputs'
		);
		const nodes = new Map([
			[1, root],
			[2, parameters],
			[3, outputs]
		]);

		expect(childByDeclaredKey(nodes, root, 'parameters')).toBe(parameters);
		expect(soundCardNodeAtPath(nodes, root, 'parameters/virtual_outputs')).toBe(outputs);
	});

	it('projects sparse routes while retaining missing references', () => {
		const root = node(1, 'sound_card_monitor_route_list', 'Monitoring', [2]);
		const route = node(2, 'sound_card_monitor_route', 'Monitor A', [3, 4, 5]);
		const source = node(3, 'reference', 'Source', [], 'virtual_input', {
			kind: 'reference',
			uuid: 'missing-source',
			cached_name: 'Former Input'
		});
		const destination = node(4, 'reference', 'Destination', [], 'virtual_output', {
			kind: 'reference',
			uuid: 'output-uuid',
			cached_id: 6
		});
		const gain = node(5, 'float', 'Gain', [], 'gain_db', { kind: 'float', value: -4.5 });
		const output = {
			...node(6, 'sound_card_virtual_output', 'Main Output'),
			uuid: 'output-uuid'
		};
		const nodes = new Map([
			[1, root],
			[2, route],
			[3, source],
			[4, destination],
			[5, gain],
			[6, output]
		]);

		expect(
			soundCardRouteRows(nodes, root, 'sound_card_monitor_route', 'virtual_input', 'virtual_output')
		).toEqual([
			{
				id: 2,
				label: 'Monitor A',
				source: 'Former Input',
				destination: 'Main Output',
				gainDb: -4.5
			}
		]);
		expect(
			soundCardRouteRecords(
				nodes,
				root,
				'sound_card_monitor_route',
				'virtual_input',
				'virtual_output'
			)[0]
		).toMatchObject({
			sourceKey: 'reference:missing-source',
			destinationKey: 'reference:output-uuid',
			gainParameterId: 5
		});
	});

	it('maps stable virtual-channel UUIDs to current labels', () => {
		const root = node(1, 'sound_card_module', 'Sound Card', [2, 3]);
		const input = { ...node(2, 'sound_card_virtual_input', 'Mic'), uuid: 'input-id' };
		const output = { ...node(3, 'sound_card_virtual_output', 'Speakers'), uuid: 'output-id' };
		const labels = soundCardChannelLabels(
			new Map([
				[1, root],
				[2, input],
				[3, output]
			]),
			root
		);

		expect(labels.get('input-id')).toBe('Mic');
		expect(labels.get('output-id')).toBe('Speakers');
	});

	it('derives physical and playback axes only from backend-projected limits and descriptors', () => {
		const state = createMockAudioDeviceState();
		const physicalInputs = soundCardPhysicalChannelEndpoints(state, 'input');
		const playbackSources = soundCardPlaybackSourceEndpoints(256);

		expect(physicalInputs.map((endpoint) => endpoint.label)).toEqual(['Input 1', 'Input 2']);
		expect(physicalInputs.map((endpoint) => endpoint.value)).toEqual([
			{ kind: 'str', value: 'input:0' },
			{ kind: 'str', value: 'input:1' }
		]);
		expect(playbackSources).toHaveLength(256);
		expect(playbackSources.at(-1)?.value).toEqual({ kind: 'int', value: 256 });
	});
});
