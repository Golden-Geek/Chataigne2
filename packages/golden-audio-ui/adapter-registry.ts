import { registerNodeInspector, unregisterNodeInspector } from 'golden_ui';
import GoldenAudioDeviceInspector from './GoldenAudioDeviceInspector.svelte';
import {
	audioDeviceInspectorNodeTypes,
	clearAudioDeviceInspectorAdapters,
	deleteAudioDeviceInspectorAdapter,
	hasAudioDeviceInspectorAdapter,
	normalizeAudioInspectorNodeType,
	setAudioDeviceInspectorAdapter
} from './binding-registry';
import type { AudioDeviceInspectorAdapter } from './types';

export { resolveGoldenAudioDeviceInspectorBinding } from './binding-registry';

export const registerGoldenAudioDeviceInspector = (
	nodeType: string,
	adapterFactory: AudioDeviceInspectorAdapter
): void => {
	const normalizedNodeType = normalizeAudioInspectorNodeType(nodeType);
	if (!normalizedNodeType) {
		throw new Error('Golden Audio inspector registration requires a non-empty node type.');
	}
	setAudioDeviceInspectorAdapter(normalizedNodeType, adapterFactory);
	registerNodeInspector(normalizedNodeType, {
		component: GoldenAudioDeviceInspector
	});
};

export const unregisterGoldenAudioDeviceInspector = (nodeType: string): void => {
	const normalizedNodeType = normalizeAudioInspectorNodeType(nodeType);
	if (!hasAudioDeviceInspectorAdapter(normalizedNodeType)) return;
	deleteAudioDeviceInspectorAdapter(normalizedNodeType);
	unregisterNodeInspector(normalizedNodeType);
};

export const resetGoldenAudioDeviceInspectorsForTests = (): void => {
	for (const nodeType of audioDeviceInspectorNodeTypes()) {
		unregisterNodeInspector(nodeType);
	}
	clearAudioDeviceInspectorAdapters();
};
