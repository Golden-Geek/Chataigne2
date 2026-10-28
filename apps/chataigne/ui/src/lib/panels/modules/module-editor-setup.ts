import generatorsIconUrl from '#lib/assets/icons/module/generators.svg';
import audioIconUrl from '#lib/assets/icons/module/audio.svg';
import streamDeckIconUrl from '#lib/assets/icons/nodes/streamdeck_module.svg';
import StreamDeckEditorPanel from './StreamDeckEditorPanel.svelte';
import SoundCardEditorPanel from './SoundCardEditorPanel.svelte';
import SpatializerEditorPanel from './SpatializerEditorPanel.svelte';
import {
	moduleEditorPanelDefinitions,
	registerModuleEditor,
	type ModuleEditorDescriptor
} from './module-editor-registry';

const defaultModuleEditors: readonly ModuleEditorDescriptor[] = [
	{
		nodeType: 'streamdeck_module',
		panelType: 'streamdeck-editor',
		panelIdPrefix: 'streamdeck-editor',
		panelComponent: StreamDeckEditorPanel,
		description: 'Visual editor for Stream Deck modules.',
		actionLabel: 'Stream Deck',
		iconUrl: streamDeckIconUrl,
		title: (node) => `Stream Deck: ${node.meta.label}`
	},
	{
		nodeType: 'spatializer_module',
		panelType: 'spatializerEditor',
		panelIdPrefix: 'spatializer-editor',
		panelComponent: SpatializerEditorPanel,
		description: '2D editor for Spatializer modules',
		actionLabel: 'Edit Spatializer',
		iconUrl: generatorsIconUrl,
		title: (node) => `Spatializer: ${node.meta.label}`
	},
	{
		nodeType: 'sound_card_module',
		panelType: 'soundCardEditor',
		panelIdPrefix: 'sound-card-editor',
		panelComponent: SoundCardEditorPanel,
		description: 'Connection, routing, channel levels, and processing for Sound Card modules',
		actionLabel: 'Edit Sound Card',
		iconUrl: audioIconUrl,
		title: (node) => `Sound Card: ${node.meta.label}`
	}
];

export const registerDefaultModuleEditors = (): void => {
	for (const descriptor of defaultModuleEditors) registerModuleEditor(descriptor);
};

export { moduleEditorPanelDefinitions };
