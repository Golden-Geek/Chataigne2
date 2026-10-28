import { beforeEach, describe, expect, it } from 'vitest';
import type { UiNodeDto } from 'golden_ui';
import { moduleEditorPanelDefinitions, registerDefaultModuleEditors } from '../module-editor-setup';
import {
	moduleEditorPanelRequest,
	resetModuleEditorsForTests,
	resolveModuleEditor,
	unregisterModuleEditor
} from '../module-editor-registry';

const moduleNode = (nodeType: string, nodeId: number, label: string) =>
	({
		node_id: nodeId,
		node_type: nodeType,
		meta: { label }
	}) as UiNodeDto;

describe('module editor registry', () => {
	beforeEach(() => resetModuleEditorsForTests());

	it('has no implicit registration side effect', () => {
		expect(resolveModuleEditor('spatializer_module')).toBeNull();
		expect(resolveModuleEditor('sound_card_module')).toBeNull();
		expect(resolveModuleEditor('streamdeck_module')).toBeNull();
	});

	it('registers app-owned Spatializer, Sound Card and Stream Deck panel definitions', () => {
		registerDefaultModuleEditors();
		const definitions = moduleEditorPanelDefinitions();

		expect(Object.keys(definitions).sort()).toEqual([
			'soundCardEditor',
			'spatializerEditor',
			'streamdeck-editor'
		]);
		expect(resolveModuleEditor('spatializer_module')?.actionLabel).toBe('Edit Spatializer');
		expect(resolveModuleEditor('sound_card_module')?.actionLabel).toBe('Edit Sound Card');
		const streamDeckNode = moduleNode('streamdeck_module', 74, 'Desk');
		const streamDeck = resolveModuleEditor(streamDeckNode)!;
		expect(streamDeck.panelType).toBe('streamdeck-editor');
		expect(streamDeck.actionLabel).toBe('Stream Deck');
		expect(moduleEditorPanelRequest(streamDeck, streamDeckNode)).toMatchObject({
			panelId: 'streamdeck-editor-74',
			panelType: 'streamdeck-editor',
			title: 'Stream Deck: Desk',
			params: { moduleNodeId: 74 }
		});
	});

	it('builds stable per-module panel identities and titles', () => {
		registerDefaultModuleEditors();
		const node = moduleNode('sound_card_module', 73, 'Studio');
		const descriptor = resolveModuleEditor(node)!;

		expect(moduleEditorPanelRequest(descriptor, node)).toMatchObject({
			panelId: 'sound-card-editor-73',
			panelType: 'soundCardEditor',
			title: 'Sound Card: Studio',
			params: { moduleNodeId: 73 }
		});
	});

	it('unregisters one editor without changing the others', () => {
		registerDefaultModuleEditors();
		unregisterModuleEditor('sound_card_module');

		expect(resolveModuleEditor('sound_card_module')).toBeNull();
		expect(resolveModuleEditor('spatializer_module')).not.toBeNull();
		expect(resolveModuleEditor('streamdeck_module')).not.toBeNull();
	});
});
