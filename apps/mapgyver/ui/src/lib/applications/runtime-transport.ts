import { hasDesktopHost } from 'golden_ui/host/desktop';
import { listen } from '@tauri-apps/api/event';
import type { ApplicationRuntimeSnapshot } from '../media/generated/ApplicationRuntimeSnapshot';
import { APPLICATION_RUNTIME_EVENT } from '../media/generated/constants';
import type { RuntimeTransport } from './runtime';

export function createRuntimeTransport(): RuntimeTransport {
	return {
		available: hasDesktopHost,
		subscribe: (listener) =>
			listen<ApplicationRuntimeSnapshot>(APPLICATION_RUNTIME_EVENT, (event) =>
				listener(event.payload)
			)
	};
}
