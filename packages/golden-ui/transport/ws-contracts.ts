import type {
	UiReplayCursor,
	UiAck,
	UiControlLifecycle,
	UiTransportBatch,
	UiEventBatchHandler,
	UiSnapshot,
	UiSubscriptionScope
} from '../types';
import type { UiDataPlane } from '../generated/rust_protocol/UiDataPlane';
import type { UiInterest } from '../generated/rust_protocol/UiInterest';
import type { UiSessionProvider } from './session';
import type { SubscriptionResyncState } from './subscription-resync';

export type UiTransportConnectionState =
	'connecting' | 'connected' | 'disconnected' | 'reconnecting';

export interface WebSocketUiClientOptions {
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
	) => UiReplayCursor | Promise<UiReplayCursor>;
}

export interface PendingIntent {
	resolve: (ack: UiAck) => void;
	reject: (error: Error) => void;
	timer: ReturnType<typeof setTimeout> | null;
	receiptConfirmed: boolean;
	onLifecycle?: (phase: UiControlLifecycle) => void;
}

export interface PendingIntentBatch {
	resolve: (acks: UiAck[]) => void;
	reject: (error: Error) => void;
	timer: ReturnType<typeof setTimeout> | null;
	receiptConfirmed: boolean;
	onLifecycle?: (phase: UiControlLifecycle) => void;
}

export interface PendingSnapshot {
	resolve: (snapshot: UiSnapshot) => void;
	reject: (error: Error) => void;
	timer: ReturnType<typeof setTimeout>;
}

export interface PendingReplay {
	resolve: (batch: UiTransportBatch) => void;
	reject: (error: Error) => void;
	timer: ReturnType<typeof setTimeout>;
}

export interface SubscriptionState extends SubscriptionResyncState {
	interest: UiInterest;
	onBatch: UiEventBatchHandler;
}

export interface SocketOpenAttempt {
	socket: WebSocket;
	generation: number;
	promise: Promise<WebSocket>;
	resolve: (socket: WebSocket) => void;
	reject: (error: Error) => void;
}

export interface WsMessageTiming {
	bytes: number;
	receivedAtMs: number;
	parseMs: number;
}
