import type {
	EventTime,
	UiAck,
	UiClient,
	UiEditIntent,
	UiEventBatch,
	UiSubscriptionScope
} from '../types';
import { wholeGraphScope } from '../types';
import {
	createHttpUiClient,
	fromRustEventBatch,
	toRustIntent,
	toRustScope,
	type RustScope,
	type RustUiEventBatch
} from './http';

const DEFAULT_WS_URL = 'ws://localhost:7010/api/ui/ws';
const UI_PROTOCOL_VERSION = '0.1.0';
const INTENT_TIMEOUT_MS = 4000;
const RECONNECT_BASE_MS = 250;
const RECONNECT_MAX_MS = 5000;
const RESYNC_REQUIRED_TOPIC = '__transport.resync_required';

export type UiTransportConnectionState =
	| 'connecting'
	| 'connected'
	| 'disconnected'
	| 'reconnecting'
	| 'fallbackPolling';

interface WebSocketUiClientOptions {
	wsUrl?: string;
	httpBaseUrl?: string;
	pollIntervalMs?: number;
	fetchImpl?: typeof fetch;
	webSocketImpl?: typeof WebSocket;
	onConnectionStateChange?: (state: UiTransportConnectionState, detail?: string) => void;
}

type WsClientMessage =
	| { kind: 'hello'; protocol_version: string }
	| { kind: 'subscribe'; subscription_id: string; scope: RustScope; from?: EventTime }
	| { kind: 'unsubscribe'; subscription_id: string }
	| {
			kind: 'intent';
			request_id: string;
			intent: unknown;
			include_self_events: boolean;
	  };

type WsServerMessage =
	| { kind: 'hello'; protocol_version: string; client_id: number; session_id?: string }
	| { kind: 'batch'; subscription_id: string; batch: RustUiEventBatch }
	| { kind: 'intentAck'; request_id: string; ack: UiAck }
	| { kind: 'resyncRequired'; subscription_id: string; reason: string }
	| { kind: 'error'; message: string; request_id?: string };

interface PendingIntent {
	resolve: (ack: UiAck) => void;
	reject: (error: Error) => void;
	timer: ReturnType<typeof setTimeout>;
}

interface SubscriptionState {
	scope: UiSubscriptionScope;
	onBatch: (batch: UiEventBatch) => void;
	cursor?: EventTime;
	closed: boolean;
	fallbackUnsubscribe: (() => void) | null;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

const toWsUrl = (value?: string): string => {
	if (!value || value.trim().length === 0) {
		return DEFAULT_WS_URL;
	}
	return value.replace(/^http/i, 'ws');
};

const includeSelfEventsForIntent = (_intent: UiEditIntent): boolean => true;

export const createWebSocketUiClient = (options: WebSocketUiClientOptions = {}): UiClient => {
	const wsUrl = toWsUrl(
		options.wsUrl ??
			(options.httpBaseUrl ? `${options.httpBaseUrl.replace(/\/+$/, '')}/ws` : undefined)
	);
	const WebSocketImpl =
		options.webSocketImpl ?? (typeof WebSocket !== 'undefined' ? WebSocket : null);
	const httpClient = createHttpUiClient({
		baseUrl: options.httpBaseUrl,
		pollIntervalMs: options.pollIntervalMs,
		fetchImpl: options.fetchImpl
	});

	let socket: WebSocket | null = null;
	let openPromise: Promise<WebSocket> | null = null;
	let openResolve: ((socket: WebSocket) => void) | null = null;
	let openReject: ((error: Error) => void) | null = null;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let reconnectAttempts = 0;
	let reconnecting = false;
	let lastConnectionState: UiTransportConnectionState | null = null;
	let lastServerSessionId: string | null = null;

	const subscriptions = new Map<string, SubscriptionState>();
	const pendingIntents = new Map<string, PendingIntent>();

	let seq = 0;
	const nextId = (prefix: string): string => {
		seq += 1;
		return `${prefix}-${Date.now().toString(36)}-${seq.toString(36)}`;
	};

	const rejectAllPendingIntents = (message: string): void => {
		for (const [requestId, pending] of pendingIntents) {
			clearTimeout(pending.timer);
			pending.reject(new Error(message));
			pendingIntents.delete(requestId);
		}
	};

	const emitConnectionState = (
		state: UiTransportConnectionState,
		detail?: string,
		force = false
	): void => {
		if (!force && lastConnectionState === state) {
			return;
		}
		lastConnectionState = state;
		options.onConnectionStateChange?.(state, detail);
	};

	const clearReconnectTimer = (): void => {
		if (reconnectTimer) {
			clearTimeout(reconnectTimer);
			reconnectTimer = null;
		}
	};

	const closeSocket = (): void => {
		clearReconnectTimer();
		if (socket && WebSocketImpl && socket.readyState === WebSocketImpl.OPEN) {
			socket.close();
		}
		socket = null;
		openPromise = null;
		openResolve = null;
		openReject = null;
	};

	const startPollingFallback = (subscriptionId: string, state: SubscriptionState): void => {
		if (state.closed || state.fallbackUnsubscribe !== null) {
			return;
		}
		state.fallbackUnsubscribe = httpClient.subscribe(state.scope, state.cursor, (batch) => {
			if (state.closed) {
				return;
			}
			if (batch.to) {
				state.cursor = batch.to;
			}
			state.onBatch(batch);
		});
		emitConnectionState('fallbackPolling', subscriptionId);
		console.warn(`[ui ws] subscription ${subscriptionId} switched to HTTP polling fallback`);
	};

	const stopPollingFallback = (state: SubscriptionState): void => {
		if (state.fallbackUnsubscribe) {
			state.fallbackUnsubscribe();
			state.fallbackUnsubscribe = null;
		}
	};

	const startFallbackForAllSubscriptions = (): void => {
		for (const [subscriptionId, state] of subscriptions) {
			startPollingFallback(subscriptionId, state);
		}
	};

	const stopFallbackForAllSubscriptions = (): void => {
		for (const state of subscriptions.values()) {
			stopPollingFallback(state);
		}
	};

	const scheduleReconnect = (reason: string): void => {
		if (!WebSocketImpl || subscriptions.size === 0 || reconnectTimer !== null || reconnecting) {
			return;
		}

		const delayMs = Math.min(RECONNECT_MAX_MS, RECONNECT_BASE_MS * 2 ** reconnectAttempts);
		reconnectAttempts += 1;
		emitConnectionState('reconnecting', reason, true);
		reconnectTimer = setTimeout(() => {
			reconnectTimer = null;
			void connectAndResubscribe();
		}, delayMs);
		console.warn(`[ui ws] disconnected (${reason}); reconnecting in ${delayMs}ms`);
	};

	const sendRawOnOpenSocket = (message: WsClientMessage): void => {
		if (!socket || !WebSocketImpl || socket.readyState !== WebSocketImpl.OPEN) {
			return;
		}
		socket.send(JSON.stringify(message));
	};

	const replayMissedEvents = async (
		subscriptionId: string,
		state: SubscriptionState
	): Promise<void> => {
		try {
			const replay = await httpClient.replay(state.scope, state.cursor);
			if (state.closed) {
				return;
			}
			if (replay.events.length > 0) {
				state.onBatch(replay);
			}
			if (replay.to) {
				state.cursor = replay.to;
			}
		} catch (error) {
			console.error(`[ui ws] replay failed for subscription ${subscriptionId}`, error);
		}
	};

	const sendSubscribe = (subscriptionId: string, state: SubscriptionState): void => {
		sendRawOnOpenSocket({
			kind: 'subscribe',
			subscription_id: subscriptionId,
			scope: toRustScope(state.scope),
			from: state.cursor
		});
	};

	const resubscribeAll = async (): Promise<void> => {
		stopFallbackForAllSubscriptions();
		for (const [subscriptionId, state] of subscriptions) {
			if (state.closed) {
				continue;
			}
			await replayMissedEvents(subscriptionId, state);
			if (state.closed) {
				continue;
			}
			sendSubscribe(subscriptionId, state);
		}
	};

	const emitResyncMarker = (
		subscriptionId: string,
		state: SubscriptionState,
		reason: string
	): void => {
		const markerTime: EventTime = state.cursor ?? { tick: 0, micro: 0, seq: 0 };
		state.onBatch({
			from: state.cursor,
			to: state.cursor,
			events: [
				{
					time: markerTime,
					kind: {
						kind: 'custom',
						topic: RESYNC_REQUIRED_TOPIC,
						payload: { reason, subscription_id: subscriptionId }
					}
				}
			]
		});
	};

	const forceResyncAll = (reason: string): void => {
		for (const [subscriptionId, state] of subscriptions) {
			if (state.closed) {
				continue;
			}
			emitResyncMarker(subscriptionId, state, reason);
			state.cursor = undefined;
			sendSubscribe(subscriptionId, state);
		}
	};

	const handleResyncRequired = (subscriptionId: string, reason: string): void => {
		const state = subscriptions.get(subscriptionId);
		if (!state || state.closed) {
			return;
		}

		console.warn(`[ui ws] subscription ${subscriptionId} requires resync: ${reason}`);
		emitResyncMarker(subscriptionId, state, reason);

		state.cursor = undefined;
		sendSubscribe(subscriptionId, state);
	};

	const handleServerMessage = (raw: unknown): void => {
		if (!isRecord(raw) || typeof raw.kind !== 'string') {
			return;
		}

		const message = raw as WsServerMessage;
		switch (message.kind) {
			case 'hello': {
				if (typeof message.session_id === 'string' && message.session_id.length > 0) {
					const previous = lastServerSessionId;
					lastServerSessionId = message.session_id;
					if (previous !== null && previous !== message.session_id) {
						console.warn(
							`[ui ws] server session changed (${previous} -> ${message.session_id}); forcing listener resync`
						);
						forceResyncAll('server_session_changed');
					}
				}
				return;
			}
			case 'batch': {
				const state = subscriptions.get(message.subscription_id);
				if (!state || state.closed) {
					return;
				}
				const batch = fromRustEventBatch(message.batch);
				if (batch.to) {
					state.cursor = batch.to;
				}
				state.onBatch(batch);
				return;
			}
			case 'intentAck': {
				const pending = pendingIntents.get(message.request_id);
				if (!pending) {
					return;
				}
				clearTimeout(pending.timer);
				pendingIntents.delete(message.request_id);
				pending.resolve(message.ack);
				return;
			}
			case 'resyncRequired':
				handleResyncRequired(message.subscription_id, message.reason);
				return;
			case 'error': {
				if (message.request_id) {
					const pending = pendingIntents.get(message.request_id);
					if (pending) {
						clearTimeout(pending.timer);
						pendingIntents.delete(message.request_id);
						pending.reject(new Error(message.message));
						return;
					}
				}
				console.error('ui ws server error:', message.message);
			}
		}
	};

	const ensureSocket = async (): Promise<WebSocket> => {
		if (!WebSocketImpl) {
			throw new Error('WebSocket API is unavailable in this environment');
		}
		if (socket && socket.readyState === WebSocketImpl.OPEN) {
			return socket;
		}
		if (openPromise) {
			return openPromise;
		}

		emitConnectionState('connecting');
		socket = new WebSocketImpl(wsUrl);
		openPromise = new Promise<WebSocket>((resolve, reject) => {
			openResolve = resolve;
			openReject = reject;
		});

		const currentSocket = socket;
		currentSocket.onopen = () => {
			sendRawOnOpenSocket({
				kind: 'hello',
				protocol_version: UI_PROTOCOL_VERSION
			});
			reconnectAttempts = 0;
			emitConnectionState('connected', undefined, true);
			openResolve?.(currentSocket);
			openResolve = null;
			openReject = null;
			openPromise = null;
			void resubscribeAll();
		};

		currentSocket.onerror = () => {
			if (openReject) {
				openReject(new Error(`websocket connection failed (${wsUrl})`));
			}
			openPromise = null;
			openResolve = null;
			openReject = null;
		};

		currentSocket.onclose = () => {
			const wasActive = socket === currentSocket;
			if (wasActive) {
				socket = null;
			}
			rejectAllPendingIntents('websocket disconnected');
			openPromise = null;
			openResolve = null;
			openReject = null;
			startFallbackForAllSubscriptions();
			emitConnectionState('disconnected', 'socket closed', true);
			scheduleReconnect('socket closed');
		};

		currentSocket.onmessage = (event) => {
			const text = typeof event.data === 'string' ? event.data : '';
			if (text.length === 0) {
				return;
			}

			try {
				const parsed = JSON.parse(text) as unknown;
				handleServerMessage(parsed);
			} catch (error) {
				console.error('failed to parse ws message', error);
			}
		};

		return openPromise;
	};

	const connectAndResubscribe = async (): Promise<void> => {
		if (!WebSocketImpl || reconnecting) {
			return;
		}
		reconnecting = true;
		try {
			await ensureSocket();
		} catch (error) {
			console.error('[ui ws] reconnect attempt failed', error);
			emitConnectionState('disconnected', 'reconnect failed', true);
			scheduleReconnect('reconnect failed');
		} finally {
			reconnecting = false;
		}
	};

	const sendWsMessage = async (message: WsClientMessage): Promise<void> => {
		const ws = await ensureSocket();
		ws.send(JSON.stringify(message));
	};

	const client: UiClient = {
		async snapshot(scope: UiSubscriptionScope = wholeGraphScope) {
			return httpClient.snapshot(scope);
		},

		subscribe(
			scope: UiSubscriptionScope,
			from: EventTime | undefined,
			onBatch: (batch: UiEventBatch) => void
		): () => void {
			const subscriptionId = nextId('sub');
			const state: SubscriptionState = {
				scope,
				onBatch,
				cursor: from,
				closed: false,
				fallbackUnsubscribe: null
			};
			subscriptions.set(subscriptionId, state);

			if (!WebSocketImpl) {
				startPollingFallback(subscriptionId, state);
			} else {
				void ensureSocket()
					.then(() => {
						if (!state.closed) {
							sendSubscribe(subscriptionId, state);
						}
					})
					.catch((error) => {
						console.error('ws subscribe failed, switching to polling fallback', error);
						startPollingFallback(subscriptionId, state);
						scheduleReconnect('initial subscribe failed');
					});
			}

			return () => {
				state.closed = true;
				stopPollingFallback(state);
				subscriptions.delete(subscriptionId);
				if (socket && WebSocketImpl && socket.readyState === WebSocketImpl.OPEN) {
					sendRawOnOpenSocket({
						kind: 'unsubscribe',
						subscription_id: subscriptionId
					});
				}
				if (subscriptions.size === 0) {
					clearReconnectTimer();
				}
			};
		},

		async sendIntent(intent: UiEditIntent): Promise<UiAck> {
			try {
				const requestId = nextId('intent');
				const includeSelfEvents = includeSelfEventsForIntent(intent);
				const ack = await new Promise<UiAck>(async (resolve, reject) => {
					const timer = setTimeout(() => {
						pendingIntents.delete(requestId);
						reject(new Error(`intent timeout (${requestId})`));
					}, INTENT_TIMEOUT_MS);

					pendingIntents.set(requestId, { resolve, reject, timer });
					try {
						await sendWsMessage({
							kind: 'intent',
							request_id: requestId,
							intent: toRustIntent(intent),
							include_self_events: includeSelfEvents
						});
					} catch (error) {
						clearTimeout(timer);
						pendingIntents.delete(requestId);
						reject(error as Error);
					}
				});
				return ack;
			} catch {
				return httpClient.sendIntent(intent);
			}
		},

		async replay(scope: UiSubscriptionScope, from?: EventTime) {
			return httpClient.replay(scope, from);
		}
	};

	const defer = (callback: () => void): void => {
		if (typeof queueMicrotask === 'function') {
			queueMicrotask(callback);
			return;
		}
		void Promise.resolve().then(callback);
	};

	if (WebSocketImpl) {
		defer(() => {
			void ensureSocket().catch((error) => {
				console.error('initial websocket connect failed', error);
				startFallbackForAllSubscriptions();
				emitConnectionState('disconnected', 'initial connect failed', true);
				scheduleReconnect('initial connect failed');
			});
		});
	} else {
		emitConnectionState('fallbackPolling', 'websocket unavailable', true);
	}

	return client;
};
