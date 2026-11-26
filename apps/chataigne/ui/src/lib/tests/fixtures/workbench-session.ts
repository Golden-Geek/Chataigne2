import { createWorkbenchSession } from 'golden_ui/store/workbench.svelte';
import type { UiClient } from 'golden_ui/types';

export const createConnectedWorkbenchSession = (client: UiClient) =>
	createWorkbenchSession({
		enableGlobalShortcuts: false,
		transportFactory: (options) => {
			queueMicrotask(() => options?.onConnectionStateChange?.('connected'));
			return client;
		}
	});
