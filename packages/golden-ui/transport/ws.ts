import type {
  EventTime,
  UiAck,
  UiClient,
  UiControlLifecycle,
  UiEditIntent,
  UiEventBatch,
  UiSnapshot,
  UiSubscriptionScope,
} from "../types";
import type { UiClientMessage as WsClientMessage } from "../generated/rust_protocol/UiClientMessage";
import type { UiDataPlane } from "../generated/rust_protocol/UiDataPlane";
import type { UiInterest } from "../generated/rust_protocol/UiInterest";
import type { UiServerMessage as WsServerMessage } from "../generated/rust_protocol/UiServerMessage";
import { wholeGraphScope } from "../types";
import {
  createHttpAuxiliaryClient,
  fromRustAck,
  fromRustEventBatch,
  fromRustSnapshot,
  toRustIntent,
  toRustScope,
} from "./http";
import { getUiClientInstanceId } from "./client-instance";

const DEFAULT_WS_URL = "ws://localhost:7010/api/ui/ws";
const UI_PROTOCOL_VERSION = "0.2.0";
const INTENT_TIMEOUT_MS = 4000;
const SNAPSHOT_TIMEOUT_MS = 120000;
const RECONNECT_BASE_MS = 250;
const RECONNECT_MAX_MS = 5000;

export type UiTransportConnectionState =
  "connecting" | "connected" | "disconnected" | "reconnecting";

interface WebSocketUiClientOptions {
  wsUrl?: string;
  httpBaseUrl?: string;
  fetchImpl?: typeof fetch;
  webSocketImpl?: typeof WebSocket;
  onConnectionStateChange?: (
    state: UiTransportConnectionState,
    detail?: string,
  ) => void;
  onResyncRequired?: (
    scope: UiSubscriptionScope,
    plane: UiDataPlane | undefined,
    reason: string,
  ) => void;
}

interface PendingIntent {
  resolve: (ack: UiAck) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
  onLifecycle?: (phase: UiControlLifecycle) => void;
}

interface PendingIntentBatch {
  resolve: (acks: UiAck[]) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
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

interface SubscriptionState {
  interest: UiInterest;
  scope: UiSubscriptionScope;
  onBatch: (batch: UiEventBatch) => void;
  cursor?: EventTime;
  closed: boolean;
  stagedBatches: UiEventBatch[];
  frameScheduled: boolean;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const toWsUrl = (value?: string): string => {
  if (!value || value.trim().length === 0) {
    return DEFAULT_WS_URL;
  }
  return value.replace(/^http/i, "ws");
};

const includeSelfEventsForIntent = (_intent: UiEditIntent): boolean => true;

const nowMs = (): number =>
  typeof performance !== "undefined" ? performance.now() : Date.now();

const shouldLogUiPerf = (): boolean => {
  if (typeof window === "undefined") {
    return false;
  }
  try {
    return window.localStorage.getItem("gc_ui_perf") === "1";
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

const compareEventTime = (left: EventTime, right: EventTime): number =>
  left.tick - right.tick || left.micro - right.micro || left.seq - right.seq;

const stageFrameBatch = (
  state: SubscriptionState,
  batch: UiEventBatch,
): void => {
  state.stagedBatches.push(batch);
  if (state.frameScheduled) {
    return;
  }
  state.frameScheduled = true;
  const flush = (): void => {
    state.frameScheduled = false;
    if (state.closed || state.stagedBatches.length === 0) {
      state.stagedBatches.length = 0;
      return;
    }
    const staged = state.stagedBatches.splice(0);
    const events = staged
      .flatMap((entry) => entry.events)
      .sort((left, right) => compareEventTime(left.time, right.time));
    const merged: UiEventBatch = {
      from: staged.find((entry) => entry.from !== undefined)?.from,
      to: staged
        .map((entry) => entry.to)
        .filter((value): value is EventTime => value !== undefined)
        .sort(compareEventTime)
        .at(-1),
      runtime: staged
        .map((entry) => entry.runtime)
        .filter((value) => value !== undefined)
        .at(-1),
      events,
    };
    if (merged.to) {
      state.cursor = merged.to;
    }
    state.onBatch(merged);
  };
  if (typeof requestAnimationFrame === "function") {
    requestAnimationFrame(flush);
  } else {
    setTimeout(flush, 0);
  }
};

export const createWebSocketUiClient = (
  options: WebSocketUiClientOptions = {},
): UiClient => {
  const wsUrl = toWsUrl(
    options.wsUrl ??
      (options.httpBaseUrl
        ? `${options.httpBaseUrl.replace(/\/+$/, "")}/ws`
        : undefined),
  );
  const WebSocketImpl =
    options.webSocketImpl ??
    (typeof WebSocket !== "undefined" ? WebSocket : null);
  const clientInstanceId = getUiClientInstanceId();
  const httpClient = createHttpAuxiliaryClient({
    baseUrl: options.httpBaseUrl,
    fetchImpl: options.fetchImpl,
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
  const pendingIntentBatches = new Map<string, PendingIntentBatch>();
  const pendingSnapshots = new Map<string, PendingSnapshot>();
  const pendingReplays = new Map<string, PendingReplay>();

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
    for (const [requestId, pending] of pendingIntentBatches) {
      clearTimeout(pending.timer);
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
    force = false,
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

  const scheduleReconnect = (reason: string): void => {
    if (
      !WebSocketImpl ||
      subscriptions.size === 0 ||
      reconnectTimer !== null ||
      reconnecting
    ) {
      return;
    }

    const delayMs = Math.min(
      RECONNECT_MAX_MS,
      RECONNECT_BASE_MS * 2 ** reconnectAttempts,
    );
    reconnectAttempts += 1;
    emitConnectionState("reconnecting", reason, true);
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      void connectAndResubscribe();
    }, delayMs);
    console.warn(
      `[ui ws] disconnected (${reason}); reconnecting in ${delayMs}ms`,
    );
  };

  const sendRawOnOpenSocket = (message: WsClientMessage): void => {
    if (!socket || !WebSocketImpl || socket.readyState !== WebSocketImpl.OPEN) {
      return;
    }
    socket.send(JSON.stringify(message));
  };

  const sendSubscribe = (
    subscriptionId: string,
    state: SubscriptionState,
  ): void => {
    sendRawOnOpenSocket({
      kind: "subscribe",
      subscription_id: subscriptionId,
      interest: state.interest,
      from: state.cursor,
    });
  };

  const resubscribeAll = async (): Promise<void> => {
    for (const [subscriptionId, state] of subscriptions) {
      if (state.closed) {
        continue;
      }
      sendSubscribe(subscriptionId, state);
    }
  };

  const forceResyncAll = (reason: string): void => {
    for (const [subscriptionId, state] of subscriptions) {
      if (state.closed) {
        continue;
      }
      options.onResyncRequired?.(state.scope, undefined, reason);
      state.cursor = undefined;
      sendSubscribe(subscriptionId, state);
    }
  };

  const handleResyncRequired = (
    subscriptionId: string,
    plane: UiDataPlane | undefined,
    reason: string,
  ): void => {
    const state = subscriptions.get(subscriptionId);
    if (!state || state.closed) {
      return;
    }

    console.warn(
      `[ui ws] subscription ${subscriptionId} requires resync: ${reason}`,
    );
    options.onResyncRequired?.(state.scope, plane, reason);

    // The server advances its subscription cursor to the snapshot boundary
    // before emitting this message. Keep that server-side subscription alive;
    // resubscribing from an empty cursor would replay the same invalidation and
    // trigger a second full snapshot.
  };

  const handleServerMessage = (
    raw: unknown,
    timing?: WsMessageTiming,
  ): void => {
    if (!isRecord(raw) || typeof raw.kind !== "string") {
      return;
    }

    const message = raw as WsServerMessage;
    switch (message.kind) {
      case "hello": {
        if (
          typeof message.session_id === "string" &&
          message.session_id.length > 0
        ) {
          const previous = lastServerSessionId;
          lastServerSessionId = message.session_id;
          if (previous !== null && previous !== message.session_id) {
            console.warn(
              `[ui ws] server session changed (${previous} -> ${message.session_id}); forcing listener resync`,
            );
            forceResyncAll("server_session_changed");
          }
        }
        return;
      }
      case "delta": {
        const state = subscriptions.get(message.subscription_id);
        if (!state || state.closed) {
          return;
        }
        const convertStartedAt = nowMs();
        const batch = fromRustEventBatch(message.delta.batch);
        const convertMs = nowMs() - convertStartedAt;
        const applyStartedAt = nowMs();
        stageFrameBatch(state, batch);
        const applyMs = nowMs() - applyStartedAt;
        const totalMs = timing
          ? nowMs() - timing.receivedAtMs
          : convertMs + applyMs;
        logUiPerf(
          `[ui] ws_batch subscription=${message.subscription_id} events=${batch.events.length} bytes=${
            timing?.bytes ?? 0
          } ws_batch_parse_ms=${(timing?.parseMs ?? 0).toFixed(1)} ws_batch_convert_ms=${convertMs.toFixed(
            1,
          )} ws_batch_apply_ms=${applyMs.toFixed(1)} total_ms=${totalMs.toFixed(1)}`,
        );
        return;
      }
      case "snapshot": {
        const pending = pendingSnapshots.get(message.request_id);
        if (!pending) {
          return;
        }
        clearTimeout(pending.timer);
        pendingSnapshots.delete(message.request_id);
        pending.resolve(fromRustSnapshot(message.snapshot));
        return;
      }
      case "replay": {
        const pending = pendingReplays.get(message.request_id);
        if (!pending) {
          return;
        }
        clearTimeout(pending.timer);
        pendingReplays.delete(message.request_id);
        pending.resolve(fromRustEventBatch(message.batch));
        return;
      }
      case "control": {
        const { update } = message;
        const pending = pendingIntents.get(update.request_id);
        if (pending) {
          pending.onLifecycle?.(update.phase);
          if (update.phase === "applied" || update.phase === "rejected") {
            clearTimeout(pending.timer);
            pendingIntents.delete(update.request_id);
            if (update.acknowledgement) {
              pending.resolve(fromRustAck(update.acknowledgement));
            } else {
              pending.reject(
                new Error(
                  "final control update did not include an acknowledgement",
                ),
              );
            }
          }
          return;
        }

        const pendingBatch = pendingIntentBatches.get(update.request_id);
        if (!pendingBatch) {
          return;
        }
        pendingBatch.onLifecycle?.(update.phase);
        if (update.phase === "applied" || update.phase === "rejected") {
          clearTimeout(pendingBatch.timer);
          pendingIntentBatches.delete(update.request_id);
          pendingBatch.resolve(
            (update.acknowledgements ?? []).map(fromRustAck),
          );
        }
        return;
      }
      case "resyncRequired":
        handleResyncRequired(
          message.subscription_id,
          message.plane ?? undefined,
          message.reason,
        );
        return;
      case "error": {
        if (message.request_id) {
          const pending = pendingIntents.get(message.request_id);
          if (pending) {
            clearTimeout(pending.timer);
            pendingIntents.delete(message.request_id);
            pending.reject(new Error(message.message));
            return;
          }
          const pendingBatch = pendingIntentBatches.get(message.request_id);
          if (pendingBatch) {
            clearTimeout(pendingBatch.timer);
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
        console.error("ui ws server error:", message.message);
      }
    }
  };

  const ensureSocket = async (): Promise<WebSocket> => {
    if (!WebSocketImpl) {
      throw new Error("WebSocket API is unavailable in this environment");
    }
    if (socket && socket.readyState === WebSocketImpl.OPEN) {
      return socket;
    }
    if (openPromise) {
      return openPromise;
    }

    emitConnectionState("connecting");
    socket = new WebSocketImpl(wsUrl);
    openPromise = new Promise<WebSocket>((resolve, reject) => {
      openResolve = resolve;
      openReject = reject;
    });

    const currentSocket = socket;
    currentSocket.onopen = () => {
      sendRawOnOpenSocket({
        kind: "hello",
        protocol_version: UI_PROTOCOL_VERSION,
        client_instance_id: clientInstanceId,
      });
      reconnectAttempts = 0;
      emitConnectionState("connected", undefined, true);
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
      rejectAllPendingIntents("websocket disconnected");
      openPromise = null;
      openResolve = null;
      openReject = null;
      emitConnectionState("disconnected", "socket closed", true);
      scheduleReconnect("socket closed");
    };

    currentSocket.onmessage = (event) => {
      const receivedAtMs = nowMs();
      const text = typeof event.data === "string" ? event.data : "";
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
          parseMs,
        });
      } catch (error) {
        console.error("failed to parse ws message", error);
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
      console.error("[ui ws] reconnect attempt failed", error);
      emitConnectionState("disconnected", "reconnect failed", true);
      scheduleReconnect("reconnect failed");
    } finally {
      reconnecting = false;
    }
  };

  const sendWsMessage = async (message: WsClientMessage): Promise<void> => {
    const ws = await ensureSocket();
    ws.send(JSON.stringify(message));
  };

  const subscribeWithInterest = (
    interest: UiInterest,
    scope: UiSubscriptionScope,
    from: EventTime | undefined,
    onBatch: (batch: UiEventBatch) => void,
  ): (() => void) => {
    const subscriptionId = nextId("sub");
    const state: SubscriptionState = {
      interest,
      scope,
      onBatch,
      cursor: from,
      closed: false,
      stagedBatches: [],
      frameScheduled: false,
    };
    subscriptions.set(subscriptionId, state);

    if (!WebSocketImpl) {
      emitConnectionState("disconnected", "websocket unavailable", true);
    } else {
      void ensureSocket()
        .then(() => {
          if (!state.closed) {
            sendSubscribe(subscriptionId, state);
          }
        })
        .catch((error) => {
          console.error("ws subscribe failed", error);
          scheduleReconnect("initial subscribe failed");
        });
    }

    return () => {
      state.closed = true;
      subscriptions.delete(subscriptionId);
      if (socket && WebSocketImpl && socket.readyState === WebSocketImpl.OPEN) {
        sendRawOnOpenSocket({
          kind: "unsubscribe",
          subscription_id: subscriptionId,
        });
      }
      if (subscriptions.size === 0) {
        clearReconnectTimer();
      }
    };
  };

  const client: UiClient = {
    async snapshot(scope: UiSubscriptionScope = wholeGraphScope) {
      const requestId = nextId("snapshot");
      return new Promise<UiSnapshot>(async (resolve, reject) => {
        const timer = setTimeout(() => {
          pendingSnapshots.delete(requestId);
          reject(new Error(`snapshot timeout (${requestId})`));
        }, SNAPSHOT_TIMEOUT_MS);
        pendingSnapshots.set(requestId, { resolve, reject, timer });
        try {
          await sendWsMessage({
            kind: "snapshot",
            request_id: requestId,
            scope: toRustScope(scope),
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
      onBatch: (batch: UiEventBatch) => void,
    ): () => void {
      return subscribeWithInterest(
        {
          view_id: "workbench",
          scope: toRustScope(scope),
          planes: [
            "structure",
            "value",
            "trigger",
            "observation",
            "catalog",
            "preview",
          ],
        },
        scope,
        from,
        onBatch,
      );
    },

    subscribeInterest(
      viewId: string,
      scope: UiSubscriptionScope,
      planes: UiDataPlane[],
      from: EventTime | undefined,
      onBatch: (batch: UiEventBatch) => void,
    ): () => void {
      return subscribeWithInterest(
        { view_id: viewId, scope: toRustScope(scope), planes },
        scope,
        from,
        onBatch,
      );
    },

    async sendIntent(
      intent: UiEditIntent,
      onLifecycle?: (phase: UiControlLifecycle) => void,
    ): Promise<UiAck> {
      let sent = false;
      try {
        const requestId = nextId("intent");
        const includeSelfEvents = includeSelfEventsForIntent(intent);
        const ack = await new Promise<UiAck>(async (resolve, reject) => {
          const timer = setTimeout(() => {
            pendingIntents.delete(requestId);
            reject(new Error(`intent timeout (${requestId})`));
          }, INTENT_TIMEOUT_MS);

          pendingIntents.set(requestId, {
            resolve,
            reject,
            timer,
            onLifecycle,
          });
          try {
            await sendWsMessage({
              kind: "intent",
              request_id: requestId,
              intent: toRustIntent(intent),
              include_self_events: includeSelfEvents,
            });
            sent = true;
          } catch (error) {
            clearTimeout(timer);
            pendingIntents.delete(requestId);
            reject(error as Error);
          }
        });
        return ack;
      } catch (error) {
        if (sent) {
          throw error;
        }
        throw error;
      }
    },

    async sendIntents(
      intents: UiEditIntent[],
      onLifecycle?: (phase: UiControlLifecycle) => void,
    ): Promise<UiAck[]> {
      if (intents.length === 0) {
        return [];
      }
      if (intents.length === 1) {
        return [await client.sendIntent(intents[0], onLifecycle)];
      }
      let sent = false;
      try {
        const requestId = nextId("intent-batch");
        const includeSelfEvents = true;
        const timeoutMs =
          INTENT_TIMEOUT_MS + Math.min(16000, intents.length * 16);
        const acks = await new Promise<UiAck[]>(async (resolve, reject) => {
          const timer = setTimeout(() => {
            pendingIntentBatches.delete(requestId);
            reject(new Error(`intent batch timeout (${requestId})`));
          }, timeoutMs);
          pendingIntentBatches.set(requestId, {
            resolve,
            reject,
            timer,
            onLifecycle,
          });
          try {
            await sendWsMessage({
              kind: "intentBatch",
              request_id: requestId,
              intents: intents.map((intent) => toRustIntent(intent)),
              include_self_events: includeSelfEvents,
            });
            sent = true;
          } catch (error) {
            clearTimeout(timer);
            pendingIntentBatches.delete(requestId);
            reject(error as Error);
          }
        });
        return acks;
      } catch (error) {
        if (sent) {
          throw error;
        }
        throw error;
      }
    },

    async replay(scope: UiSubscriptionScope, from?: EventTime) {
      const requestId = nextId("replay");
      return new Promise<UiEventBatch>(async (resolve, reject) => {
        const timer = setTimeout(() => {
          pendingReplays.delete(requestId);
          reject(new Error(`replay timeout (${requestId})`));
        }, INTENT_TIMEOUT_MS);
        pendingReplays.set(requestId, { resolve, reject, timer });
        try {
          await sendWsMessage({
            kind: "replay",
            request_id: requestId,
            scope: toRustScope(scope),
            from,
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
    },
  };

  const defer = (callback: () => void): void => {
    if (typeof queueMicrotask === "function") {
      queueMicrotask(callback);
      return;
    }
    void Promise.resolve().then(callback);
  };

  if (WebSocketImpl) {
    defer(() => {
      void ensureSocket().catch((error) => {
        console.error("initial websocket connect failed", error);
        emitConnectionState("disconnected", "initial connect failed", true);
        scheduleReconnect("initial connect failed");
      });
    });
  } else {
    emitConnectionState("disconnected", "websocket unavailable", true);
  }

  return client;
};
