import { describe, expect, it, vi } from 'vitest';
import {
	SOUND_CARD_UI_CONTROL_TOPIC,
	sendSoundCardPlaybackControl
} from '../sound-card-playback-controls';

describe('Sound Card playback controls', () => {
	it('sends a transient control intent to the owning module', async () => {
		const send = vi.fn(async () => undefined);

		expect(
			await sendSoundCardPlaybackControl(44, { kind: 'stop_file', playback_id: 'music' }, { send })
		).toBe(true);
		expect(send).toHaveBeenCalledWith({
			kind: 'sendNodeEvent',
			node: 44,
			topic: SOUND_CARD_UI_CONTROL_TOPIC,
			payload: { kind: 'stop_file', playback_id: 'music' }
		});
	});

	it('reports a rejected backend acknowledgement without retaining optimistic state', async () => {
		const send = vi.fn(async () => {
			throw new Error('rejected');
		});

		expect(await sendSoundCardPlaybackControl(44, { kind: 'stop_all_files' }, { send })).toBe(
			false
		);
	});
});
