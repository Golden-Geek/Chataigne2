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
	});

	it('provides Spatializer and Sound Card panel definitions from one descriptor source', () => {
		registerDefaultModuleEditors();
		const definitions = moduleEditorPanelDefinitions();

		expect(Object.keys(definitions).sort()).toEqual(['soundCardEditor', 'spatializerEditor']);
		expect(resolveModuleEditor('spatializer_module')?.actionLabel).toBe('Edit Spatializer');
		expect(resolveModuleEditor('sound_card_module')?.actionLabel).toBe('Edit Sound Card');
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

	it('unregisters one editor without changing the other', () => {
		registerDefaultModuleEditors();
		unregisterModuleEditor('sound_card_module');

		expect(resolveModuleEditor('sound_card_module')).toBeNull();
		expect(resolveModuleEditor('spatializer_module')).not.toBeNull();
	});
});
