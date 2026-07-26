import type { SoundCardUiControlRequest } from '$lib/modules/audio/sound-card/generated';
import type { NodeId, UiEditIntent } from 'golden_ui';
import { appState } from 'golden_ui/store/workbench.svelte';

export const SOUND_CARD_UI_CONTROL_TOPIC = 'chataigne.sound_card.ui.control';

export interface SoundCardPlaybackControlPort {
	send(intent: UiEditIntent): Promise<void>;
}

const defaultPort: SoundCardPlaybackControlPort = {
	send: async (intent) => {
		const session = appState.session;
		if (!session) throw new Error('Sound Card playback control requires an active session.');
		await session.sendIntent(intent);
	}
};

export const sendSoundCardPlaybackControl = async (
	moduleNodeId: NodeId,
	request: SoundCardUiControlRequest,
	port: SoundCardPlaybackControlPort = defaultPort
): Promise<boolean> => {
	try {
		await port.send({
			kind: 'sendNodeEvent',
			node: moduleNodeId,
			topic: SOUND_CARD_UI_CONTROL_TOPIC,
			payload: request
		});
		return true;
	} catch (error) {
		console.error('failed to send Sound Card playback control', request, error);
		return false;
	}
};
