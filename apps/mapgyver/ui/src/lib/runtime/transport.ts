import { listen } from '@tauri-apps/api/event';
import { hasDesktopHost, invokeDesktopCommand } from 'golden_ui/host/desktop';
import type { VideoRuntimeStatus } from '../preview/generated/VideoRuntimeStatus';
import type { VideoRuntimeRequest } from '../preview/generated/VideoRuntimeRequest';
import { VIDEO_RUNTIME_EVENT } from '../preview/generated/constants';

export interface VideoStatusTransport {
	available(): boolean;
	read(): Promise<VideoRuntimeStatus>;
	subscribe(receive: (status: VideoRuntimeStatus) => void): Promise<() => void>;
}

/** Native host adapter; status lifecycle and UI depend only on VideoStatusTransport. */
export function createDesktopVideoStatusTransport(): VideoStatusTransport {
	return {
		available: hasDesktopHost,
		async read() {
			const request: VideoRuntimeRequest = { kind: 'mapgyverRuntimeStatus' };
			const value = await invokeDesktopCommand<string>(
				'native_presentation_control',
				{ request: JSON.stringify(request) },
				'video-status'
			);
			if (value === undefined) throw new Error('Video runtime observations are unavailable');
			return JSON.parse(value) as VideoRuntimeStatus;
		},
		subscribe: (receive) =>
			listen<VideoRuntimeStatus>(VIDEO_RUNTIME_EVENT, (event) => receive(event.payload))
	};
}
