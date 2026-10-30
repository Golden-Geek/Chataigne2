import { hasDesktopHost, invokeDesktopCommand } from 'golden_ui/host/desktop';
import { listen } from '@tauri-apps/api/event';
import type { MediaNativeRequest } from './generated/MediaNativeRequest';
import type { SenderDiscoverySnapshot } from './generated/SenderDiscoverySnapshot';
import { SENDER_DISCOVERY_EVENT } from './generated/constants';

export interface SenderDiscoveryTransport {
	available(): boolean;
	request(refresh: boolean): Promise<SenderDiscoverySnapshot>;
	subscribe(listener: (snapshot: SenderDiscoverySnapshot) => void): Promise<() => void>;
}

export function createSenderDiscoveryTransport(): SenderDiscoveryTransport {
	return {
		available: hasDesktopHost,
		async request(refresh) {
			const request: MediaNativeRequest = { kind: 'mapgyverSenders', refresh };
			const response = await invokeDesktopCommand<string>(
				'native_presentation_control',
				{ request: JSON.stringify(request) },
				'mapgyver-senders'
			);
			if (response === undefined) throw new Error('Sender discovery is unavailable');
			return JSON.parse(response) as SenderDiscoverySnapshot;
		},
		async subscribe(listener) {
			return listen<SenderDiscoverySnapshot>(SENDER_DISCOVERY_EVENT, (event) =>
				listener(event.payload)
			);
		}
	};
}
