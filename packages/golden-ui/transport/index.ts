import type { UiDataPlane } from '../generated/rust_protocol/UiDataPlane';
import type { EventTime, UiClient, UiSubscriptionScope } from '../types';
import { createWebSocketUiClient } from './ws';
import type { UiTransportConnectionState } from './ws-contracts';
import type { UiSessionProvider } from './session';
export { createUiSessionProvider, type UiSessionProvider } from './session';

export type { UiTransportConnectionState } from './ws-contracts';

export interface UiTransportOptions {
	wsUrl?: string;
	httpBaseUrl?: string;
	fetchImpl?: typeof fetch;
	webSocketImpl?: typeof WebSocket;
	sessionProvider?: UiSessionProvider;
	onConnectionStateChange?: (state: UiTransportConnectionState, detail?: string) => void;
	onResyncRequired?: (
		scope: UiSubscriptionScope,
		plane: UiDataPlane | undefined,
		reason: string
	) => EventTime | Promise<EventTime>;
}

export type UiTransportFactory = (options?: UiTransportOptions) => UiClient;

export const createDefaultUiClient: UiTransportFactory = (
	options: UiTransportOptions = {}
): UiClient => createWebSocketUiClient(options);
export { ControlRequestError } from './control-errors';
