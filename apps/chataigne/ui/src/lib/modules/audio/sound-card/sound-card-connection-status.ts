import type {
	AudioDeviceInspectorState,
	AudioDeviceReadiness,
	AudioStreamStatus
} from './generated';

export type SoundCardConnectionTone = 'success' | 'error' | 'pending' | 'idle';

export interface SoundCardConnectionHint {
	readonly tone: SoundCardConnectionTone;
	readonly message: string;
}

const FAILURE_READINESS = new Set<AudioDeviceReadiness>([
	'missing',
	'unavailable',
	'busy',
	'permission_denied',
	'failed'
]);

const directionLabel = (stream: AudioStreamStatus): string =>
	stream.direction === 'input' ? 'Input' : 'Output';

const streamSelection = (stream: AudioStreamStatus): string =>
	`${directionLabel(stream)}: ${stream.selected_label?.trim() || 'selected device'}`;

const formatSampleRate = (sampleRate: number): string => {
	const kiloHertz = sampleRate / 1_000;
	return `${Number.isInteger(kiloHertz) ? kiloHertz : kiloHertz.toFixed(1)} kHz`;
};

const formatSummary = (streams: readonly AudioStreamStatus[]): string[] => {
	const formats = streams.map((stream) => stream.format).filter((format) => format !== null);
	const sampleRates = [...new Set(formats.map((format) => format.sample_rate))];
	const bufferFrames = [...new Set(formats.map((format) => format.buffer_frames))];
	const summary: string[] = [];

	if (sampleRates.length === 1 && sampleRates[0] !== undefined) {
		summary.push(formatSampleRate(sampleRates[0]));
	}
	if (bufferFrames.length === 1 && bufferFrames[0] !== undefined) {
		summary.push(`${bufferFrames[0]}-frame buffer`);
	}
	return summary;
};

const failureMessage = (stream: AudioStreamStatus): string => {
	const reported = stream.error?.message.trim();
	if (reported) return reported;

	switch (stream.readiness) {
		case 'missing':
			return 'the selected device is unavailable';
		case 'unavailable':
			return 'the audio backend is unavailable';
		case 'busy':
			return 'the selected device is busy';
		case 'permission_denied':
			return 'permission to use the device was denied';
		default:
			return 'the audio stream could not be opened';
	}
};

export const soundCardConnectionHint = (
	connected: boolean | null,
	device: AudioDeviceInspectorState | null
): SoundCardConnectionHint => {
	if (!device) {
		if (connected === true) {
			return {
				tone: 'success',
				message: 'Current configuration is connected.'
			};
		}
		if (connected === false) {
			return {
				tone: 'error',
				message: 'Current configuration is not connected.'
			};
		}
		return {
			tone: 'idle',
			message: 'Current configuration status is unavailable.'
		};
	}

	const activeStreams = [device.input, device.output].filter((stream) => stream.enabled);
	const failedStream = activeStreams.find(
		(stream) => stream.error !== null || FAILURE_READINESS.has(stream.readiness)
	);
	if (failedStream) {
		return {
			tone: 'error',
			message: `Current configuration: ${streamSelection(failedStream)} — ${failureMessage(failedStream)}.`
		};
	}

	if (activeStreams.length > 0 && activeStreams.every((stream) => stream.readiness === 'ready')) {
		const details = [...activeStreams.map(streamSelection), ...formatSummary(activeStreams)];
		return {
			tone: 'success',
			message: `Current configuration: ${details.join(' · ')}.`
		};
	}

	if (activeStreams.length > 0) {
		return {
			tone: 'pending',
			message: `Current configuration: ${activeStreams.map(streamSelection).join(' · ')} — connecting…`
		};
	}

	return {
		tone: connected === true ? 'success' : 'error',
		message:
			connected === true
				? 'Current configuration is connected.'
				: 'Current configuration has no active audio device.'
	};
};
