import { hasDesktopHost, invokeDesktopCommand } from 'golden_ui/host/desktop';
import { listen } from '@tauri-apps/api/event';
import type { DisplayCatalogSnapshot } from './generated/DisplayCatalogSnapshot';
import type { ScreenNativeRequest } from './generated/ScreenNativeRequest';
import { DISPLAY_CATALOG_EVENT } from './generated/constants';

export interface DisplayCatalogTransport {
	available(): boolean;
	request(refresh: boolean): Promise<DisplayCatalogSnapshot>;
	subscribe(listener: (snapshot: DisplayCatalogSnapshot) => void): Promise<() => void>;
}

export function createDisplayCatalogTransport(): DisplayCatalogTransport {
	return {
		available: hasDesktopHost,
		async request(refresh) {
			const request: ScreenNativeRequest = { kind: 'mapgyverDisplays', refresh };
			const response = await invokeDesktopCommand<string>(
				'native_presentation_control',
				{ request: JSON.stringify(request) },
				'mapgyver-displays'
			);
			if (response === undefined) throw new Error('Display discovery is unavailable');
			return JSON.parse(response) as DisplayCatalogSnapshot;
		},
		subscribe(listener) {
			return listen<DisplayCatalogSnapshot>(DISPLAY_CATALOG_EVENT, (event) =>
				listener(event.payload)
			);
		}
	};
}
