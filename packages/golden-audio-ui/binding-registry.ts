import type { UiNodeDto } from 'golden_ui';
import type { AudioDeviceInspectorAdapter, AudioDeviceInspectorBinding } from './types';

const adapterFactories = new Map<string, AudioDeviceInspectorAdapter>();

export const normalizeAudioInspectorNodeType = (nodeType: string): string => nodeType.trim();

export const setAudioDeviceInspectorAdapter = (
	nodeType: string,
	adapterFactory: AudioDeviceInspectorAdapter
): void => {
	adapterFactories.set(nodeType, adapterFactory);
};

export const deleteAudioDeviceInspectorAdapter = (nodeType: string): void => {
	adapterFactories.delete(nodeType);
};

export const hasAudioDeviceInspectorAdapter = (nodeType: string): boolean =>
	adapterFactories.has(nodeType);

export const audioDeviceInspectorNodeTypes = (): readonly string[] => [...adapterFactories.keys()];

export const clearAudioDeviceInspectorAdapters = (): void => {
	adapterFactories.clear();
};

export const resolveGoldenAudioDeviceInspectorBinding = (
	node: UiNodeDto
): AudioDeviceInspectorBinding | null =>
	adapterFactories.get(normalizeAudioInspectorNodeType(node.node_type))?.(node) ?? null;
