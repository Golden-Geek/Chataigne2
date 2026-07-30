import type {
	EventTime,
	UiAck,
	UiClient,
	UiControlLifecycle,
	UiEditIntent,
	UiEventBatch,
	UiEventBatchHandler,
	UiSnapshot,
	UiSubscriptionBatchOptions,
	UiSubscriptionScope
} from '../types';
import type { UiClientMessage as WsClientMessage } from '../generated/rust_protocol/UiClientMessage';
import type { UiDataPlane } from '../generated/rust_protocol/UiDataPlane';
import type { UiInterest } from '../generated/rust_protocol/UiInterest';
import type { UiServerMessage as WsServerMessage } from '../generated/rust_protocol/UiServerMessage';
import { UI_PROTOCOL_VERSION } from '../generated/rust_protocol/protocol-version';
import { wholeGraphScope } from '../types';
import {
	createHttpAuxiliaryClient,
	fromRustAck,
	fromRustEventBatch,
	fromRustSnapshot,
	toRustIntent,
	toRustScope
} from './http';
import { getUiClientInstanceId } from './client-instance';
import { createStagedFrameBatchScheduler } from './staged-frame-batches';
import {
	createSubscriptionResyncCoordinator,
	type SubscriptionResyncState
} from './subscription-resync';

const DEFAULT_WS_URL = 'ws://localhost:7010/api/ui/ws';
const INTENT_SEND_TIMEOUT_MS = 4000;
const INTENT_RECEIPT_TIMEOUT_MS = 4000;
const INTENT_PROCESSING_TIMEOUT_MS = 120000;
const REPLAY_TIMEOUT_MS = 4000;
const SNAPSHOT_TIMEOUT_MS = 120000;
const RECONNECT_BASE_MS = 250;
const RECONNECT_MAX_MS = 5000;
const RESYNC_RETRY_MS = 1000;

export type UiTransportConnectionState =
	'connecting' | 'connected' | 'disconnected' | 'reconnecting';

interface WebSocketUiClientOptions {
	wsUrl?: string;
	httpBaseUrl?: string;
	fetchImpl?: typeof fetch;
	webSocketImpl?: typeof WebSocket;
	onConnectionStateChange?: (state: UiTransportConnectionState, detail?: string) => void;
	onResyncRequired?: (
		scope: UiSubscriptionScope,
		plane: UiDataPlane | undefined,
		reason: string
	) => EventTime | Promise<EventTime>;
}

interface PendingIntent {
	resolve: (ack: UiAck) => void;
	reject: (error: Error) => void;
	timer: ReturnType<typeof setTimeout> | null;
	receiptConfirmed: boolean;
	onLifecycle?: (phase: UiControlLifecycle) => void;
}

interface PendingIntentBatch {
	resolve: (acks: UiAck[]) => void;
	reject: (error: Error) => void;
	timer: ReturnType<typeof setTimeout> | null;
	receiptConfirmed: boolean;
	onLifecycle?: (phase: UiControlLifecycle) => void;
}

interface PendingSnapshot {
	resolve: (snapshot: UiSnapshot) => void;
	reject: (error: Error) => void;
	timer: ReturnType<typeof setTimeout>;
}

interface PendingReplay {
	resolve: (batch: UiEventBatch) => void;
	reject: (error: Error) => void;
	timer: ReturnType<typeof setTimeout>;
}

interface SubscriptionState extends SubscriptionResyncState {
	interest: UiInterest;
	onBatch: UiEventBatchHandler;
}

interface SocketOpenAttempt {
	socket: WebSocket;
	generation: number;
	promise: Promise<WebSocket>;
	resolve: (socket: WebSocket) => void;
	reject: (error: Error) => void;
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

const nowMs = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());

const shouldLogUiPerf = (): boolean => {
	if (typeof window === 'undefined') {
		return false;
	}
	try {
		return window.localStorage.getItem('gc_ui_perf') === '1';
	} catch {
		return false;
	}
};

const logUiPerf = (message: string): void => {
	if (!shouldLogUiPerf()) {
		return;
	}
	console.info(`[ui-perf] ${message}`);
};

interface WsMessageTiming {
	bytes: number;
	receivedAtMs: number;
	parseMs: number;
}

export const createWebSocketUiClient = (options: WebSocketUiClientOptions = {}): UiClient => {
	const wsUrl = toWsUrl(
		options.wsUrl ??
			(options.httpBaseUrl ? `${options.httpBaseUrl.replace(/\/+$/, '')}/ws` : undefined)
	);
	const WebSocketImpl =
		options.webSocketImpl ?? (typeof WebSocket !== 'undefined' ? WebSocket : null);
	const clientInstanceId = getUiClientInstanceId();
	const httpClient = createHttpAuxiliaryClient({
		baseUrl: options.httpBaseUrl,
		fetchImpl: options.fetchImpl
	});

	let socket: WebSocket | null = null;
	let socketGeneration = 0;
	let nextSocketGeneration = 0;
	let openAttempt: SocketOpenAttempt | null = null;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let reconnectAttempts = 0;
	let reconnecting = false;
	let lastConnectionState: UiTransportConnectionState | null = null;
	let lastServerSessionId: string | null = null;

	const subscriptions = new Map<string, SubscriptionState>();
	const pendingIntents = new Map<string, PendingIntent>();
	const pendingIntentBatches = new Map<string, PendingIntentBatch>();
	const pendingSnapshots = new Map<string, PendingSnapshot>();
	const pendingReplays = new Map<string, PendingReplay>();

	const clearControlTimer = (pending: PendingIntent | PendingIntentBatch): void => {
		if (pending.timer !== null) {
			clearTimeout(pending.timer);
			pending.timer = null;
		}
	};

	const armProcessingWatchdog = <TPending extends PendingIntent | PendingIntentBatch>(
		requestId: string,
		pending: TPending,
		registry: Map<string, TPending>,
		description: string
	): void => {
		clearControlTimer(pending);
		pending.timer = setTimeout(() => {
			if (registry.get(requestId) !== pending) {
				return;
			}
			registry.delete(requestId);
			pending.timer = null;
			pending.reject(
				new Error(
					`${description} processing timeout after 120 seconds (${requestId}); runtime outcome is unknown`
				)
			);
		}, INTENT_PROCESSING_TIMEOUT_MS);
	};

	const armSendDeadline = <TPending extends PendingIntent | PendingIntentBatch>(
		requestId: string,
		pending: TPending,
		registry: Map<string, TPending>,
		description: string
	): void => {
		clearControlTimer(pending);
		pending.timer = setTimeout(() => {
			if (registry.get(requestId) !== pending) {
				return;
			}
			registry.delete(requestId);
			pending.timer = null;
			pending.reject(
				new Error(
					`${description} send timeout after 4 seconds (${requestId}); request was not sent`
				)
			);
		}, INTENT_SEND_TIMEOUT_MS);
	};

	const armReceiptDeadline = <TPending extends PendingIntent | PendingIntentBatch>(
		requestId: string,
		pending: TPending,
		registry: Map<string, TPending>,
		description: string
	): void => {
		if (pending.receiptConfirmed || registry.get(requestId) !== pending) {
			return;
		}
		clearControlTimer(pending);
		pending.timer = setTimeout(() => {
			if (registry.get(requestId) !== pending) {
				return;
			}
			registry.delete(requestId);
			pending.timer = null;
			pending.reject(
				new Error(
					`${description} receipt timeout after 4 seconds (${requestId}); runtime did not confirm receipt`
				)
			);
		}, INTENT_RECEIPT_TIMEOUT_MS);
	};

	let seq = 0;
	const nextId = (prefix: string): string => {
		seq += 1;
		return `${prefix}-${Date.now().toString(36)}-${seq.toString(36)}`;
	};

	const rejectAllPendingIntents = (message: string): void => {
		for (const [requestId, pending] of pendingIntents) {
			clearControlTimer(pending);
			pending.reject(new Error(message));
			pendingIntents.delete(requestId);
		}
		for (const [requestId, pending] of pendingIntentBatches) {
			clearControlTimer(pending);
			pending.reject(new Error(message));
			pendingIntentBatches.delete(requestId);
		}
		for (const [requestId, pending] of pendingSnapshots) {
			clearTimeout(pending.timer);
			pending.reject(new Error(message));
			pendingSnapshots.delete(requestId);
		}
		for (const [requestId, pending] of pendingReplays) {
			clearTimeout(pending.timer);
			pending.reject(new Error(message));
			pendingReplays.delete(requestId);
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

	const scheduleReconnect = (reason: string): void => {
		if (!WebSocketImpl || subscriptions.size === 0 || reconnectTimer !== null) {
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

	const sendRawOnSocketGeneration = (message: WsClientMessage, generation: number): boolean => {
		if (
			!socket ||
			!WebSocketImpl ||
			socketGeneration !== generation ||
			socket.readyState !== WebSocketImpl.OPEN
		) {
			return false;
		}
		socket.send(JSON.stringify(message));
		return true;
	};

	const sendSubscribe = (
		subscriptionId: string,
		state: SubscriptionState,
		generation: number
	): void => {
		if (state.closed || state.resyncing || state.subscribedSocketGeneration === generation) {
			return;
		}
		if (
			sendRawOnSocketGeneration(
				{
					kind: 'subscribe',
					subscription_id: subscriptionId,
					interest: state.interest,
					from: state.cursor
				},
				generation
			)
		) {
			state.subscribedSocketGeneration = generation;
		}
	};

	const subscriptionResync = createSubscriptionResyncCoordinator<SubscriptionState>({
		retryMs: RESYNC_RETRY_MS,
		getSocketGeneration: () => socketGeneration,
		isSocketGenerationOpen: (generation) =>
			socketGeneration === generation &&
			socket !== null &&
			WebSocketImpl !== null &&
			socket.readyState === WebSocketImpl.OPEN,
		requestSnapshot: options.onResyncRequired,
		sendSubscribe,
		sendUnsubscribe: (subscriptionId, generation) => {
			sendRawOnSocketGeneration(
				{
					kind: 'unsubscribe',
					subscription_id: subscriptionId
				},
				generation
			);
		}
	});

	const resubscribeAll = (generation: number): void => {
		for (const [subscriptionId, state] of subscriptions) {
			if (state.closed) {
				continue;
			}
			state.subscribedSocketGeneration = null;
			if (!subscriptionResync.resumeAfterSocketOpen(subscriptionId, state, generation)) {
				sendSubscribe(subscriptionId, state, generation);
			}
		}
	};

	const forceResyncAll = (reason: string): void => {
		for (const [subscriptionId, state] of subscriptions) {
			if (state.closed) {
				continue;
			}
			subscriptionResync.begin(subscriptionId, state, undefined, reason);
		}
	};

	const handleResyncRequired = (
		subscriptionId: string,
		plane: UiDataPlane | undefined,
		reason: string
	): void => {
		const state = subscriptions.get(subscriptionId);
		if (!state || state.closed) {
			return;
		}

		console.warn(`[ui ws] subscription ${subscriptionId} requires resync: ${reason}`);
		subscriptionResync.begin(subscriptionId, state, plane, reason);
	};

	const handleServerMessage = (raw: unknown, timing?: WsMessageTiming): void => {
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
			case 'delta': {
				const state = subscriptions.get(message.subscription_id);
				if (!state || state.closed || state.resyncing) {
					return;
				}
				const convertStartedAt = nowMs();
				const batches = message.deltas.map((delta) => ({
					batch: fromRustEventBatch(delta.batch),
					plane: delta.plane
				}));
				const convertMs = nowMs() - convertStartedAt;
				const applyStartedAt = nowMs();
				state.stagedFrames.stageEnvelope(batches);
				const applyMs = nowMs() - applyStartedAt;
				const eventCount = batches.reduce((count, delta) => count + delta.batch.events.length, 0);
				const totalMs = timing ? nowMs() - timing.receivedAtMs : convertMs + applyMs;
				logUiPerf(
					`[ui] ws_batch subscription=${message.subscription_id} planes=${batches.length} events=${eventCount} bytes=${
						timing?.bytes ?? 0
					} ws_batch_parse_ms=${(timing?.parseMs ?? 0).toFixed(1)} ws_batch_convert_ms=${convertMs.toFixed(
						1
					)} ws_batch_apply_ms=${applyMs.toFixed(1)} total_ms=${totalMs.toFixed(1)}`
				);
				return;
			}
			case 'snapshot': {
				const pending = pendingSnapshots.get(message.request_id);
				if (!pending) {
					return;
				}
				clearTimeout(pending.timer);
				pendingSnapshots.delete(message.request_id);
				pending.resolve(fromRustSnapshot(message.snapshot));
				return;
			}
			case 'replay': {
				const pending = pendingReplays.get(message.request_id);
				if (!pending) {
					return;
				}
				clearTimeout(pending.timer);
				pendingReplays.delete(message.request_id);
				pending.resolve(fromRustEventBatch(message.batch));
				return;
			}
			case 'control': {
				const { update } = message;
				const pending = pendingIntents.get(update.request_id);
				if (pending) {
					pending.onLifecycle?.(update.phase);
					if (update.phase === 'received' || update.phase === 'accepted') {
						pending.receiptConfirmed = true;
						armProcessingWatchdog(update.request_id, pending, pendingIntents, 'intent');
					}
					if (update.phase === 'applied' || update.phase === 'rejected') {
						clearControlTimer(pending);
						pendingIntents.delete(update.request_id);
						if (update.acknowledgement) {
							pending.resolve(fromRustAck(update.acknowledgement));
						} else {
							pending.reject(new Error('final control update did not include an acknowledgement'));
						}
					}
					return;
				}

				const pendingBatch = pendingIntentBatches.get(update.request_id);
				if (!pendingBatch) {
					return;
				}
				pendingBatch.onLifecycle?.(update.phase);
				if (update.phase === 'received' || update.phase === 'accepted') {
					pendingBatch.receiptConfirmed = true;
					armProcessingWatchdog(
						update.request_id,
						pendingBatch,
						pendingIntentBatches,
						'intent batch'
					);
				}
				if (update.phase === 'applied' || update.phase === 'rejected') {
					clearControlTimer(pendingBatch);
					pendingIntentBatches.delete(update.request_id);
					pendingBatch.resolve((update.acknowledgements ?? []).map(fromRustAck));
				}
				return;
			}
			case 'resyncRequired':
				handleResyncRequired(message.subscription_id, message.plane ?? undefined, message.reason);
				return;
			case 'error': {
				if (message.request_id) {
					const pending = pendingIntents.get(message.request_id);
					if (pending) {
						clearControlTimer(pending);
						pendingIntents.delete(message.request_id);
						pending.reject(new Error(message.message));
						return;
					}
					const pendingBatch = pendingIntentBatches.get(message.request_id);
					if (pendingBatch) {
						clearControlTimer(pendingBatch);
						pendingIntentBatches.delete(message.request_id);
						pendingBatch.reject(new Error(message.message));
						return;
					}
					const pendingSnapshot = pendingSnapshots.get(message.request_id);
					if (pendingSnapshot) {
						clearTimeout(pendingSnapshot.timer);
						pendingSnapshots.delete(message.request_id);
						pendingSnapshot.reject(new Error(message.message));
						return;
					}
					const pendingReplay = pendingReplays.get(message.request_id);
					if (pendingReplay) {
						clearTimeout(pendingReplay.timer);
						pendingReplays.delete(message.request_id);
						pendingReplay.reject(new Error(message.message));
						return;
					}
				}
				console.error('ui ws server error:', message.message);
			}
		}
	};

	const handleSocketTermination = (
		currentSocket: WebSocket,
		generation: number,
		reason: string
	): void => {
		if (socket !== currentSocket || socketGeneration !== generation) {
			return;
		}

		socket = null;
		socketGeneration = 0;
		if (openAttempt?.generation === generation) {
			const attempt = openAttempt;
			openAttempt = null;
			attempt.reject(new Error(`websocket connection failed (${wsUrl}): ${reason}`));
		}
		rejectAllPendingIntents('websocket disconnected');
		for (const state of subscriptions.values()) {
			subscriptionResync.socketDisconnected(state);
		}
		emitConnectionState('disconnected', reason, true);
		scheduleReconnect(reason);
	};

	const ensureSocket = async (): Promise<WebSocket> => {
		if (!WebSocketImpl) {
			throw new Error('WebSocket API is unavailable in this environment');
		}
		if (socket && socket.readyState === WebSocketImpl.OPEN) {
			return socket;
		}
		if (
			openAttempt &&
			socket === openAttempt.socket &&
			socketGeneration === openAttempt.generation
		) {
			return openAttempt.promise;
		}

		emitConnectionState('connecting');
		const currentSocket = new WebSocketImpl(wsUrl);
		nextSocketGeneration += 1;
		const generation = nextSocketGeneration;
		let resolveOpen!: (openSocket: WebSocket) => void;
		let rejectOpen!: (error: Error) => void;
		const promise = new Promise<WebSocket>((resolve, reject) => {
			resolveOpen = resolve;
			rejectOpen = reject;
		});
		const attempt: SocketOpenAttempt = {
			socket: currentSocket,
			generation,
			promise,
			resolve: resolveOpen,
			reject: rejectOpen
		};
		socket = currentSocket;
		socketGeneration = generation;
		openAttempt = attempt;

		currentSocket.onopen = () => {
			if (socket !== currentSocket || socketGeneration !== generation || openAttempt !== attempt) {
				return;
			}
			openAttempt = null;
			sendRawOnSocketGeneration(
				{
					kind: 'hello',
					protocol_version: UI_PROTOCOL_VERSION,
					client_instance_id: clientInstanceId
				},
				generation
			);
			reconnectAttempts = 0;
			clearReconnectTimer();
			emitConnectionState('connected', undefined, true);
			resubscribeAll(generation);
			attempt.resolve(currentSocket);
		};

		currentSocket.onerror = () => {
			handleSocketTermination(currentSocket, generation, 'socket error');
		};

		currentSocket.onclose = () => {
			handleSocketTermination(currentSocket, generation, 'socket closed');
		};

		currentSocket.onmessage = (event) => {
			if (socket !== currentSocket || socketGeneration !== generation) {
				return;
			}
			const receivedAtMs = nowMs();
			const text = typeof event.data === 'string' ? event.data : '';
			if (text.length === 0) {
				return;
			}

			try {
				const parseStartedAt = nowMs();
				const parsed = JSON.parse(text) as unknown;
				const parseMs = nowMs() - parseStartedAt;
				handleServerMessage(parsed, {
					bytes: text.length,
					receivedAtMs,
					parseMs
				});
			} catch (error) {
				console.error('failed to parse ws message', error);
			}
		};

		return promise;
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

	const sendWsMessage = async (
		message: WsClientMessage,
		shouldSend: () => boolean = () => true
	): Promise<boolean> => {
		const ws = await ensureSocket();
		if (!shouldSend()) {
			return false;
		}
		if (!WebSocketImpl || socket !== ws || ws.readyState !== WebSocketImpl.OPEN) {
			throw new Error('websocket disconnected before request could be sent');
		}
		ws.send(JSON.stringify(message));
		return true;
	};

	const subscribeWithInterest = (
		interest: UiInterest,
		scope: UiSubscriptionScope,
		from: EventTime | undefined,
		onBatch: UiEventBatchHandler,
		batchOptions?: UiSubscriptionBatchOptions
	): (() => void) => {
		const subscriptionId = nextId('sub');
		let state: SubscriptionState;
		const stagedFrames = createStagedFrameBatchScheduler({
			onBatch: (batch) => {
				onBatch(batch, {
					requestResync: (reason) => {
						handleResyncRequired(subscriptionId, undefined, reason);
					}
				});
			},
			onOrderingViolation: () => {
				handleResyncRequired(subscriptionId, undefined, 'out_of_order_plane_delta');
			},
			onBacklogOverflow: () => {
				handleResyncRequired(subscriptionId, undefined, 'staged_backlog_overflow');
			},
			createEventWork: batchOptions?.createEventWork,
			isClosed: () => state.closed,
			getCursor: () => state.cursor,
			setCursor: (cursor) => {
				state.cursor = cursor;
			}
		});
		state = {
			interest,
			scope,
			onBatch,
			cursor: from,
			closed: false,
			stagedFrames,
			subscribedSocketGeneration: null,
			resyncing: false,
			resyncGeneration: 0,
			resyncPlane: undefined,
			resyncReason: '',
			resyncRetryTimer: null
		};
		subscriptions.set(subscriptionId, state);

		if (!WebSocketImpl) {
			emitConnectionState('disconnected', 'websocket unavailable', true);
		} else if (socket && socket.readyState === WebSocketImpl.OPEN) {
			sendSubscribe(subscriptionId, state, socketGeneration);
		} else {
			// The successful `onopen` pass subscribes every state that was
			// registered while this socket was connecting.
			void ensureSocket().catch((error) => {
				console.error('ws subscribe failed', error);
				scheduleReconnect('initial subscribe failed');
			});
		}

		return () => {
			state.closed = true;
			subscriptionResync.dispose(state);
			state.stagedFrames.reset();
			subscriptions.delete(subscriptionId);
			const subscribedGeneration = state.subscribedSocketGeneration;
			state.subscribedSocketGeneration = null;
			if (subscribedGeneration !== null) {
				sendRawOnSocketGeneration(
					{
						kind: 'unsubscribe',
						subscription_id: subscriptionId
					},
					subscribedGeneration
				);
			}
			if (subscriptions.size === 0) {
				clearReconnectTimer();
			}
		};
	};

	const client: UiClient = {
		async snapshot(scope: UiSubscriptionScope = wholeGraphScope) {
			const requestId = nextId('snapshot');
			return new Promise<UiSnapshot>(async (resolve, reject) => {
				const timer = setTimeout(() => {
					pendingSnapshots.delete(requestId);
					reject(new Error(`snapshot timeout (${requestId})`));
				}, SNAPSHOT_TIMEOUT_MS);
				pendingSnapshots.set(requestId, { resolve, reject, timer });
				try {
					await sendWsMessage({
						kind: 'snapshot',
						request_id: requestId,
						scope: toRustScope(scope)
					});
				} catch (error) {
					clearTimeout(timer);
					pendingSnapshots.delete(requestId);
					reject(error as Error);
				}
			});
		},

		subscribe(
			scope: UiSubscriptionScope,
			from: EventTime | undefined,
			onBatch: UiEventBatchHandler,
			batchOptions?: UiSubscriptionBatchOptions
		): () => void {
			return subscribeWithInterest(
				{
					view_id: 'workbench',
					scope: toRustScope(scope),
					planes: ['structure', 'value', 'trigger', 'observation', 'catalog', 'preview']
				},
				scope,
				from,
				onBatch,
				batchOptions
			);
		},

		subscribeInterest(
			viewId: string,
			scope: UiSubscriptionScope,
			planes: UiDataPlane[],
			from: EventTime | undefined,
			onBatch: UiEventBatchHandler,
			batchOptions?: UiSubscriptionBatchOptions
		): () => void {
			return subscribeWithInterest(
				{ view_id: viewId, scope: toRustScope(scope), planes },
				scope,
				from,
				onBatch,
				batchOptions
			);
		},

		async sendIntent(
			intent: UiEditIntent,
			onLifecycle?: (phase: UiControlLifecycle) => void
		): Promise<UiAck> {
			const requestId = nextId('intent');
			const includeSelfEvents = includeSelfEventsForIntent(intent);
			return new Promise<UiAck>(async (resolve, reject) => {
				const pending: PendingIntent = {
					resolve,
					reject,
					timer: null,
					receiptConfirmed: false,
					onLifecycle
				};
				pendingIntents.set(requestId, pending);
				armSendDeadline(requestId, pending, pendingIntents, 'intent');
				try {
					const sent = await sendWsMessage(
						{
							kind: 'intent',
							request_id: requestId,
							intent: toRustIntent(intent),
							include_self_events: includeSelfEvents
						},
						() => pendingIntents.get(requestId) === pending
					);
					if (!sent) {
						return;
					}
					armReceiptDeadline(requestId, pending, pendingIntents, 'intent');
				} catch (error) {
					clearControlTimer(pending);
					if (pendingIntents.get(requestId) === pending) {
						pendingIntents.delete(requestId);
					}
					reject(error as Error);
				}
			});
		},

		async sendIntents(
			intents: UiEditIntent[],
			onLifecycle?: (phase: UiControlLifecycle) => void
		): Promise<UiAck[]> {
			if (intents.length === 0) {
				return [];
			}
			if (intents.length === 1) {
				return [await client.sendIntent(intents[0], onLifecycle)];
			}
			const requestId = nextId('intent-batch');
			const includeSelfEvents = true;
			return new Promise<UiAck[]>(async (resolve, reject) => {
				const pending: PendingIntentBatch = {
					resolve,
					reject,
					timer: null,
					receiptConfirmed: false,
					onLifecycle
				};
				pendingIntentBatches.set(requestId, pending);
				armSendDeadline(requestId, pending, pendingIntentBatches, 'intent batch');
				try {
					const sent = await sendWsMessage(
						{
							kind: 'intentBatch',
							request_id: requestId,
							intents: intents.map((intent) => toRustIntent(intent)),
							include_self_events: includeSelfEvents
						},
						() => pendingIntentBatches.get(requestId) === pending
					);
					if (!sent) {
						return;
					}
					armReceiptDeadline(requestId, pending, pendingIntentBatches, 'intent batch');
				} catch (error) {
					clearControlTimer(pending);
					if (pendingIntentBatches.get(requestId) === pending) {
						pendingIntentBatches.delete(requestId);
					}
					reject(error as Error);
				}
			});
		},

		async replay(scope: UiSubscriptionScope, from?: EventTime) {
			const requestId = nextId('replay');
			return new Promise<UiEventBatch>(async (resolve, reject) => {
				const timer = setTimeout(() => {
					pendingReplays.delete(requestId);
					reject(new Error(`replay timeout (${requestId})`));
				}, REPLAY_TIMEOUT_MS);
				pendingReplays.set(requestId, { resolve, reject, timer });
				try {
					await sendWsMessage({
						kind: 'replay',
						request_id: requestId,
						scope: toRustScope(scope),
						from
					});
				} catch (error) {
					clearTimeout(timer);
					pendingReplays.delete(requestId);
					reject(error as Error);
				}
			});
		},

		async referenceTargets(paramNodeId: number) {
			return httpClient.referenceTargets(paramNodeId);
		},

		async paramControlInfo(paramNodeId: number) {
			return httpClient.paramControlInfo(paramNodeId);
		},

		async scriptState(nodeId: number) {
			return httpClient.scriptState(nodeId);
		},

		async setScriptConfig(nodeId, config, forceReload = false) {
			return httpClient.setScriptConfig(nodeId, config, forceReload);
		},

		async reloadScript(nodeId) {
			return httpClient.reloadScript(nodeId);
		},

		async projectNew() {
			return httpClient.projectNew();
		},

		async projectSave(path, uiState) {
			return httpClient.projectSave(path, uiState);
		},

		async projectLoad(path) {
			return httpClient.projectLoad(path);
		},

		async projectUploadLoad(fileName, contents) {
			return httpClient.projectUploadLoad(fileName, contents);
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
				emitConnectionState('disconnected', 'initial connect failed', true);
				scheduleReconnect('initial connect failed');
			});
		});
	} else {
		emitConnectionState('disconnected', 'websocket unavailable', true);
	}

	return client;
};
