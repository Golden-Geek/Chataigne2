import { hasDesktopHost, invokeDesktopCommand } from 'golden_ui/host/desktop';
import type { PreviewRequest } from './generated/PreviewRequest';
import type { PreviewResponse } from './generated/PreviewResponse';

/** Presentation capability only. App editing continues through its ordinary session transport. */
export interface PreviewTransport {
	available(): boolean;
	request(request: PreviewRequest): Promise<PreviewResponse>;
}

export function createDesktopPreviewTransport(): PreviewTransport {
	return {
		available: hasDesktopHost,
		async request(request) {
			const response = await invokeDesktopCommand<string>(
				'native_presentation_control',
				{ request: JSON.stringify(request) },
				'native-preview'
			);
			if (response === undefined) throw new Error('Native preview is unavailable');
			return JSON.parse(response) as PreviewResponse;
		}
	};
}
