import type { UiClient } from '../types';
import { createWebSocketUiClient, type UiTransportConnectionState } from './ws';

export type { UiTransportConnectionState } from './ws';

export interface UiTransportOptions {
	wsUrl?: string;
	httpBaseUrl?: string;
	pollIntervalMs?: number;
	fetchImpl?: typeof fetch;
	webSocketImpl?: typeof WebSocket;
	onConnectionStateChange?: (state: UiTransportConnectionState, detail?: string) => void;
}

export type UiTransportFactory = (options?: UiTransportOptions) => UiClient;

export const createDefaultUiClient: UiTransportFactory = (
	options: UiTransportOptions = {}
): UiClient => createWebSocketUiClient(options);
