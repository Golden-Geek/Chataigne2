import type { NodeId, ParamValue, UiNodeDto } from 'golden_ui';
import { appState } from 'golden_ui/store/workbench.svelte';
import { sendSetParamIntent } from 'golden_ui/store/ui-intents';
import type {
	AudioBufferPolicy,
	AudioDeviceInspectorState,
	AudioDeviceTargetId,
	AudioRecoveryPolicy,
	AudioStreamStatus,
	SoundCardUiTelemetryDto
} from './generated';

export const SOUND_CARD_TELEMETRY_TOPIC = 'chataigne.sound_card.telemetry';

export type IntentResult = boolean;

/**
 * Structural contract consumed by the reusable Golden audio inspector in Phase 12.
 *
 * This app adapter intentionally has no registry side effect. It translates
 * Chataigne's persisted node paths into ordinary golden_ui edit intents.
 */
export interface AudioDeviceInspectorBinding {
	readonly state: AudioDeviceInspectorState;
	setInputEnabled(enabled: boolean): Promise<IntentResult>;
	selectInputTarget(target: AudioDeviceTargetId): Promise<IntentResult>;
	setOutputEnabled(enabled: boolean): Promise<IntentResult>;
	selectOutputTarget(target: AudioDeviceTargetId): Promise<IntentResult>;
	setRecoveryPolicy(policy: AudioRecoveryPolicy): Promise<IntentResult>;
	setSampleRate(rate: number): Promise<IntentResult>;
	setBufferPolicy(policy: AudioBufferPolicy): Promise<IntentResult>;
	setFixedBufferFrames(frames: number): Promise<IntentResult>;
	refreshDevices(): Promise<void>;
}

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

export const audioDeviceTargetParamValue = (target: AudioDeviceTargetId): string =>
	target.kind === 'system_default'
		? JSON.stringify({ kind: 'system_default', backend: target.backend })
		: JSON.stringify({ kind: 'device', backend: target.backend, device: target.device });

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
): Promise<IntentResult> => {
	const node = nodeAtPath(moduleId, path);
	if (!node || node.data.kind !== 'parameter') return false;
	return sendSetParamIntent(node.node_id, value, node.data.param.event_behaviour);
};

export class ChataigneAudioDeviceInspectorAdapter implements AudioDeviceInspectorBinding {
	readonly moduleId: NodeId;

	constructor(moduleId: NodeId) {
		this.moduleId = moduleId;
	}

	get state(): AudioDeviceInspectorState {
		return (
			appState.session?.getCustomEventPayload<SoundCardUiTelemetryDto>(
				SOUND_CARD_TELEMETRY_TOPIC,
				this.moduleId
			)?.device ?? emptyState()
		);
	}

	setInputEnabled(enabled: boolean): Promise<IntentResult> {
		return setParameter(this.moduleId, 'connection/input_enabled', { kind: 'bool', value: enabled });
	}

	selectInputTarget(target: AudioDeviceTargetId): Promise<IntentResult> {
		return setParameter(this.moduleId, 'connection/input_device', {
			kind: 'enum',
			value: audioDeviceTargetParamValue(target)
		});
	}

	setOutputEnabled(enabled: boolean): Promise<IntentResult> {
		return setParameter(this.moduleId, 'connection/output_enabled', {
			kind: 'bool',
			value: enabled
		});
	}

	selectOutputTarget(target: AudioDeviceTargetId): Promise<IntentResult> {
		return setParameter(this.moduleId, 'connection/output_device', {
			kind: 'enum',
			value: audioDeviceTargetParamValue(target)
		});
	}

	setRecoveryPolicy(policy: AudioRecoveryPolicy): Promise<IntentResult> {
		return setParameter(this.moduleId, 'connection/recovery_policy', {
			kind: 'enum',
			value: policy
		});
	}

	setSampleRate(rate: number): Promise<IntentResult> {
		return setParameter(this.moduleId, 'connection/engine_sample_rate', {
			kind: 'int',
			value: Math.round(rate)
		});
	}

	setBufferPolicy(policy: AudioBufferPolicy): Promise<IntentResult> {
		return setParameter(this.moduleId, 'connection/buffer_policy', {
			kind: 'enum',
			value: policy.kind
		});
	}

	setFixedBufferFrames(frames: number): Promise<IntentResult> {
		return setParameter(this.moduleId, 'connection/fixed_buffer_frames', {
			kind: 'int',
			value: Math.round(frames)
		});
	}

	async refreshDevices(): Promise<void> {
		await setParameter(this.moduleId, 'connection/refresh_devices', { kind: 'trigger' });
	}
}

export const createSoundCardAudioDeviceInspectorAdapter = (
	moduleId: NodeId
): AudioDeviceInspectorBinding => new ChataigneAudioDeviceInspectorAdapter(moduleId);
