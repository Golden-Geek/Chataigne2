import { beforeEach, describe, expect, it } from 'vitest';
import {
	clearCustomNodeInspectors,
	registerNodeInspector,
	resolveNodeInspector,
	type UiNodeDto
} from 'golden_ui';
import {
	GoldenAudioDeviceInspector,
	MockAudioDeviceInspectorAdapter,
	registerGoldenAudioDeviceInspector,
	resetGoldenAudioDeviceInspectorsForTests,
	resolveGoldenAudioDeviceInspectorBinding,
	unregisterGoldenAudioDeviceInspector
} from '../index';

const soundCardNode = {
	node_id: 41,
	node_type: 'sound_card_module',
	user_item_kind: 'module'
} as UiNodeDto;

describe('Golden Audio inspector registration', () => {
	beforeEach(() => {
		resetGoldenAudioDeviceInspectorsForTests();
		clearCustomNodeInspectors();
	});

	it('wins by exact node type and restores the generic item-kind inspector', () => {
		const genericModuleInspector = {};
		const adapter = new MockAudioDeviceInspectorAdapter();
		registerNodeInspector('module', { component: genericModuleInspector });

		registerGoldenAudioDeviceInspector('sound_card_module', () => adapter);
		expect(resolveNodeInspector(soundCardNode)?.component).toBe(GoldenAudioDeviceInspector);
		expect(resolveGoldenAudioDeviceInspectorBinding(soundCardNode)).toBe(adapter);

		unregisterGoldenAudioDeviceInspector('sound_card_module');
		expect(resolveNodeInspector(soundCardNode)?.component).toBe(genericModuleInspector);
		expect(resolveGoldenAudioDeviceInspectorBinding(soundCardNode)).toBeNull();
	});

	it('resets every package registration deterministically', () => {
		registerGoldenAudioDeviceInspector(
			'sound_card_module',
			() => new MockAudioDeviceInspectorAdapter()
		);
		registerGoldenAudioDeviceInspector(
			'other_audio_module',
			() => new MockAudioDeviceInspectorAdapter()
		);

		resetGoldenAudioDeviceInspectorsForTests();

		expect(resolveNodeInspector('sound_card_module')).toBeNull();
		expect(resolveNodeInspector('other_audio_module')).toBeNull();
	});

	it('has no implicit registration side effect', () => {
		expect(resolveNodeInspector('sound_card_module')).toBeNull();
	});

	it('does not remove an unrelated inspector when asked to unregister an unknown audio type', () => {
		const unrelatedInspector = {};
		registerNodeInspector('unrelated_node', { component: unrelatedInspector });

		unregisterGoldenAudioDeviceInspector('unrelated_node');

		expect(resolveNodeInspector('unrelated_node')?.component).toBe(unrelatedInspector);
	});
});
