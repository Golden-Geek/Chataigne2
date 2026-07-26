import type { NodeId, ParamValue, UiNodeDto } from 'golden_ui';
import { appState } from 'golden_ui/store/workbench.svelte';
import { sendSetParamIntent } from 'golden_ui/store/ui-intents';
import {
	createGoldenAudioDeviceParameterBinding,
	type AudioDeviceInspectorBinding,
	type AudioDeviceInspectorState,
	type AudioStreamStatus
} from 'golden_audio_ui';
import type { SoundCardUiTelemetryDto } from './generated';

export const SOUND_CARD_TELEMETRY_TOPIC = 'chataigne.sound_card.telemetry';

const disabledStream = (direction: 'input' | 'output'): AudioStreamStatus => ({
	direction,
	enabled: false,
	selected_target: null,
	selected_label: null,
	profile_key: null,
	active_target: null,
	readiness: 'disabled',
	permission: 'unknown',
	recovery_policy: 'wait_for_selected',
	retry_attempt: 0,
	next_retry_ms: null,
	format: null,
	error: null
});

const emptyState = (): AudioDeviceInspectorState => ({
	discovery_in_progress: true,
	backends: [],
	devices: [],
	input: disabledStream('input'),
	output: disabledStream('output'),
	engine_sample_rate: 48_000,
	buffer_policy: { kind: 'automatic' }
});

const childByKey = (parent: UiNodeDto, key: string): UiNodeDto | null => {
	const nodes = appState.session?.graph.state.nodesById;
	if (!nodes) return null;
	for (const childId of parent.children) {
		const child = nodes.get(childId);
		if (
			child &&
			(child.decl_id === key ||
				child.decl_id.split('/').at(-1) === key ||
				child.meta.short_name === key)
		) {
			return child;
		}
	}
	return null;
};

const nodeAtPath = (moduleId: NodeId, path: string): UiNodeDto | null => {
	let node = appState.session?.getNodeData(moduleId) ?? null;
	for (const segment of path.split('/')) {
		if (!node) return null;
		node = childByKey(node, segment);
	}
	return node;
};

const setParameter = async (
	moduleId: NodeId,
	path: string,
	value: ParamValue
): Promise<boolean> => {
	const node = nodeAtPath(moduleId, path);
	if (!node || node.data.kind !== 'parameter') return false;
	return sendSetParamIntent(node.node_id, value, node.data.param.event_behaviour);
};

const parameterTargets = {
	inputEnabled: 'connection/input_enabled',
	inputTarget: 'connection/input_device',
	outputEnabled: 'connection/output_enabled',
	outputTarget: 'connection/output_device',
	recoveryPolicy: 'connection/recovery_policy',
	sampleRate: 'connection/engine_sample_rate',
	bufferPolicy: 'connection/buffer_policy',
	fixedBufferFrames: 'connection/fixed_buffer_frames',
	refreshDevices: 'connection/refresh_devices'
} as const;

export const createSoundCardAudioDeviceInspectorAdapter = (
	moduleId: NodeId
): AudioDeviceInspectorBinding =>
	createGoldenAudioDeviceParameterBinding(
		{
			get state() {
				return (
					appState.session?.getCustomEventPayload<SoundCardUiTelemetryDto>(
						SOUND_CARD_TELEMETRY_TOPIC,
						moduleId
					)?.device ?? emptyState()
				);
			},
			get fixedBufferFrames() {
				const parameter = nodeAtPath(moduleId, parameterTargets.fixedBufferFrames);
				if (parameter?.data.kind !== 'parameter') return undefined;
				const value = parameter.data.param.value;
				return value.kind === 'int' ? value.value : undefined;
			},
			setParameter(target, value) {
				return typeof target === 'string'
					? setParameter(moduleId, target, value)
					: Promise.resolve(false);
			}
		},
		parameterTargets,
		['connection']
	);
