import { createMockAudioDeviceState } from 'golden_audio_ui/mock';
import type { SoundCardUiTelemetryDto } from '$lib/modules/audio/sound-card/generated';
import type { SoundCardMatrixEndpoint, SoundCardRouteRecord } from './sound-card-editor-model';

export const createMockSoundCardTelemetry = (): SoundCardUiTelemetryDto => ({
	generation: 7,
	render_frame: 96_000,
	device: createMockAudioDeviceState(),
	inputs: [
		{
			channel: 'mock-input-left',
			rms_linear: 0.25,
			rms_dbfs: -12,
			peak_dbfs: -6,
			clipped: false
		},
		{
			channel: 'mock-input-right',
			rms_linear: 0.18,
			rms_dbfs: -14.9,
			peak_dbfs: -7.2,
			clipped: false
		}
	],
	outputs: [
		{
			channel: 'mock-output-left',
			rms_linear: 0.31,
			rms_dbfs: -10.2,
			peak_dbfs: -3.4,
			clipped: false
		},
		{
			channel: 'mock-output-right',
			rms_linear: 0.29,
			rms_dbfs: -10.8,
			peak_dbfs: -3.8,
			clipped: false
		}
	],
	input_global_max_rms: 0.25,
	output_global_max_rms: 0.31,
	global_max_rms: 0.31,
	active_voice_count: 2,
	loading_voice_count: 1,
	playback_source_channel_limit: 256,
	playback_voices: [
		{
			playback_id: 'mock-bed',
			path: 'C:\\audio\\ambient-bed.wav',
			voice: '0:1',
			lifecycle: 'playing'
		},
		{
			playback_id: 'mock-hit',
			path: 'C:\\audio\\impact.wav',
			voice: '1:4',
			lifecycle: 'playing'
		}
	],
	dropped_event_count: 0,
	queue_pressure_count: 0,
	analysis: {
		generation: 7,
		render_frame: 96_000,
		inputs: [],
		outputs: [],
		input_global_max_rms: 0.25,
		output_global_max_rms: 0.31,
		global_max_rms: 0.31,
		taps: [
			{
				tap: 'mock-pitch',
				source: 'mock-input-left',
				enabled: true,
				result: {
					kind: 'pitch',
					value: {
						valid: true,
						frequency_hz: 440,
						confidence: 0.98,
						midi_note: 69,
						note_name: 'A4',
						cents: 0
					}
				}
			},
			{
				tap: 'mock-spectrum',
				source: 'mock-output-left',
				enabled: true,
				result: {
					kind: 'spectrum',
					value: {
						fft_size: 2048,
						bands: Array.from({ length: 24 }, (_, index) => ({
							index,
							low_hz: index * 100,
							center_hz: index * 100 + 50,
							high_hz: index * 100 + 100,
							amplitude_linear: Math.max(0, 0.8 - index * 0.025),
							amplitude_dbfs: -8 - index * 2.8
						}))
					}
				}
			}
		],
		diagnostics: {
			captured_frames: 750,
			processed_frames: 750,
			dropped_frames: 0,
			stale_frames: 0,
			worker_time_micros: 320,
			maximum_worker_time_micros: 610
		}
	}
});

export const mockSoundCardRoutes = (): readonly SoundCardRouteRecord[] => [
	{
		id: 1,
		label: 'Input Left',
		source: 'Physical Input 1',
		destination: 'Input Left',
		gainDb: 0,
		sourceKey: 'str:channel_1',
		destinationKey: 'reference:mock-input-left',
		sourceValue: { kind: 'str', value: 'channel_1' },
		destinationValue: { kind: 'reference', uuid: 'mock-input-left' },
		gainParameterId: 11,
		gainEventBehaviour: 'Coalesce'
	},
	{
		id: 2,
		label: 'Input Right',
		source: 'Physical Input 2',
		destination: 'Input Right',
		gainDb: -1.5,
		sourceKey: 'str:channel_2',
		destinationKey: 'reference:mock-input-right',
		sourceValue: { kind: 'str', value: 'channel_2' },
		destinationValue: { kind: 'reference', uuid: 'mock-input-right' },
		gainParameterId: 12,
		gainEventBehaviour: 'Coalesce'
	}
];

export const mockSoundCardMatrixSources = (): readonly SoundCardMatrixEndpoint[] => [
	{
		key: 'str:channel_1',
		label: 'Physical Input 1',
		value: { kind: 'str', value: 'channel_1' }
	},
	{
		key: 'str:channel_2',
		label: 'Physical Input 2',
		value: { kind: 'str', value: 'channel_2' }
	}
];

export const mockSoundCardMatrixDestinations = (): readonly SoundCardMatrixEndpoint[] => [
	{
		key: 'reference:mock-input-left',
		label: 'Input Left',
		value: { kind: 'reference', uuid: 'mock-input-left' }
	},
	{
		key: 'reference:mock-input-right',
		label: 'Input Right',
		value: { kind: 'reference', uuid: 'mock-input-right' }
	}
];
