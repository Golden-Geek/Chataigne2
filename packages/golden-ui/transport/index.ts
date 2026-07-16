import type { UiDataPlane } from '../generated/rust_protocol/UiDataPlane';
import type { UiClient, UiSubscriptionScope } from '../types';
import { createWebSocketUiClient, type UiTransportConnectionState } from './ws';

export type { UiTransportConnectionState } from './ws';

export interface UiTransportOptions {
	wsUrl?: string;
	httpBaseUrl?: string;
	fetchImpl?: typeof fetch;
	webSocketImpl?: typeof WebSocket;
	onConnectionStateChange?: (state: UiTransportConnectionState, detail?: string) => void;
	onResyncRequired?: (
		scope: UiSubscriptionScope,
		plane: UiDataPlane | undefined,
		reason: string
	) => void;
}

export type UiTransportFactory = (options?: UiTransportOptions) => UiClient;

export const createDefaultUiClient: UiTransportFactory = (
	options: UiTransportOptions = {}
): UiClient => createWebSocketUiClient(options);
