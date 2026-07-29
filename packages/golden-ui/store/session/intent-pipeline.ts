import type { NodeId, UiEditIntent } from "../../types";

interface IntentWaiter {
  resolve: () => void;
  reject: (reason?: unknown) => void;
}

interface QueuedIntent {
  intent: UiEditIntent;
  snapshotGeneration: number;
  waiters: IntentWaiter[];
}

export interface WorkbenchIntentPipelineOptions {
  getSnapshotGeneration(): number;
  dispatchIntent(intent: UiEditIntent): Promise<void>;
  dispatchIntents(intents: UiEditIntent[]): Promise<void>;
}

export interface WorkbenchIntentPipeline {
  sendIntent(intent: UiEditIntent): Promise<void>;
  sendIntents(intents: UiEditIntent[]): Promise<void>;
  suspendDispatch(): () => void;
  blockDispatch(reason: Error): () => void;
  runProjectGraphTransition<T>(operation: () => Promise<T>): Promise<T>;
  rejectPending(reason: Error): void;
}

const MAX_SET_PARAM_BATCH_SIZE = 512;

export const createWorkbenchIntentPipeline = (
  options: WorkbenchIntentPipelineOptions,
): WorkbenchIntentPipeline => {
  const pending: QueuedIntent[] = [];
  const idleWaiters: Array<() => void> = [];
  const dispatchBlocks: Array<{ id: number; reason: Error }> = [];
  let processing = false;
  let dispatchSuspensions = 0;
  let nextDispatchBlockId = 0;
  let activeDispatches = 0;

  const notifyIdle = (): void => {
    if (processing || activeDispatches > 0) {
      return;
    }
    for (const resolve of idleWaiters.splice(0)) {
      resolve();
    }
  };

  const waitForIdle = (): Promise<void> => {
    if (!processing && activeDispatches === 0) {
      return Promise.resolve();
    }
    return new Promise<void>((resolve) => {
      idleWaiters.push(resolve);
    });
  };

  const dispatch = async (operation: () => Promise<void>): Promise<void> => {
    activeDispatches += 1;
    try {
      await operation();
    } finally {
      activeDispatches = Math.max(0, activeDispatches - 1);
      notifyIdle();
    }
  };

  const resolveQueued = (queued: QueuedIntent[]): void => {
    for (const entry of queued) {
      for (const waiter of entry.waiters) {
        waiter.resolve();
      }
    }
  };

  const rejectQueued = (queued: QueuedIntent[], reason: unknown): void => {
    for (const entry of queued) {
      for (const waiter of entry.waiters) {
        waiter.reject(reason);
      }
    }
  };

  const rejectPending = (reason: Error): void => {
    const queued = pending.splice(0);
    rejectQueued(queued, reason);
  };

  const findQueuedSetParamIndex = (
    node: NodeId,
    snapshotGeneration: number,
  ): number => {
    for (let index = pending.length - 1; index >= 0; index -= 1) {
      const queued = pending[index];
      if (
        queued?.snapshotGeneration === snapshotGeneration &&
        queued.intent.kind === "setParam" &&
        queued.intent.node === node
      ) {
        return index;
      }
    }
    return -1;
  };

  const drainLeadingSetParams = (
    snapshotGeneration: number,
  ): QueuedIntent[] => {
    const drained: QueuedIntent[] = [];
    while (drained.length < MAX_SET_PARAM_BATCH_SIZE && pending.length > 0) {
      const next = pending[0];
      if (
        !next ||
        next.snapshotGeneration !== snapshotGeneration ||
        next.intent.kind !== "setParam"
      ) {
        break;
      }
      const queued = pending.shift();
      if (queued) {
        drained.push(queued);
      }
    }
    return drained;
  };

  const process = async (): Promise<void> => {
    if (processing) {
      return;
    }
    processing = true;
    try {
      while (pending.length > 0) {
        if (dispatchSuspensions > 0 || dispatchBlocks.length > 0) {
          break;
        }

        const next = pending[0];
        if (next?.snapshotGeneration !== options.getSnapshotGeneration()) {
          const stale = pending.shift();
          if (stale) {
            rejectQueued(
              [stale],
              new Error(
                "intent cancelled because the runtime graph snapshot changed",
              ),
            );
          }
          continue;
        }

        if (next?.intent.kind === "setParam") {
          const batch = drainLeadingSetParams(next.snapshotGeneration);
          if (batch.length === 0) {
            continue;
          }
          try {
            await dispatch(() =>
              options.dispatchIntents(batch.map((entry) => entry.intent)),
            );
            resolveQueued(batch);
          } catch (error) {
            rejectQueued(batch, error);
          }
          continue;
        }

        const queued = pending.shift();
        if (!queued) {
          continue;
        }
        try {
          await dispatch(() => options.dispatchIntent(queued.intent));
          resolveQueued([queued]);
        } catch (error) {
          rejectQueued([queued], error);
        }
      }
    } finally {
      processing = false;
      notifyIdle();
      if (
        dispatchSuspensions === 0 &&
        dispatchBlocks.length === 0 &&
        pending.length > 0
      ) {
        void process();
      }
    }
  };

  const suspendDispatch = (): (() => void) => {
    dispatchSuspensions += 1;
    let released = false;
    return () => {
      if (released) {
        return;
      }
      released = true;
      dispatchSuspensions = Math.max(0, dispatchSuspensions - 1);
      if (
        dispatchSuspensions === 0 &&
        dispatchBlocks.length === 0 &&
        pending.length > 0
      ) {
        void process();
      }
    };
  };

  const blockDispatch = (reason: Error): (() => void) => {
    nextDispatchBlockId += 1;
    const id = nextDispatchBlockId;
    dispatchBlocks.push({ id, reason });
    let released = false;
    return () => {
      if (released) {
        return;
      }
      released = true;
      const index = dispatchBlocks.findIndex((block) => block.id === id);
      if (index >= 0) {
        dispatchBlocks.splice(index, 1);
      }
      if (
        dispatchBlocks.length === 0 &&
        dispatchSuspensions === 0 &&
        pending.length > 0
      ) {
        void process();
      }
    };
  };

  const activeDispatchBlockReason = (): Error | null =>
    dispatchBlocks[dispatchBlocks.length - 1]?.reason ?? null;

  const sendIntent = (intent: UiEditIntent): Promise<void> => {
    const blockedReason = activeDispatchBlockReason();
    if (blockedReason) {
      return Promise.reject(blockedReason);
    }
    return new Promise<void>((resolve, reject) => {
      const snapshotGeneration = options.getSnapshotGeneration();
      if (intent.kind === "setParam") {
        const queuedIndex = findQueuedSetParamIndex(
          intent.node,
          snapshotGeneration,
        );
        if (queuedIndex >= 0) {
          const queued = pending[queuedIndex];
          if (queued) {
            queued.intent = intent;
            queued.waiters.push({ resolve, reject });
            void process();
            return;
          }
        }
      }

      pending.push({
        intent,
        snapshotGeneration,
        waiters: [{ resolve, reject }],
      });
      void process();
    });
  };

  const sendIntents = async (intents: UiEditIntent[]): Promise<void> => {
    const blockedReason = activeDispatchBlockReason();
    if (blockedReason) {
      throw blockedReason;
    }
    if (dispatchSuspensions > 0) {
      throw new Error(
        "intent batch cancelled because the runtime graph is synchronizing",
      );
    }
    await dispatch(() => options.dispatchIntents(intents));
  };

  const runProjectGraphTransition = async <T>(
    operation: () => Promise<T>,
  ): Promise<T> => {
    const release = suspendDispatch();
    rejectPending(
      new Error("intent cancelled because a project graph transition started"),
    );
    try {
      await waitForIdle();
      return await operation();
    } finally {
      rejectPending(
        new Error(
          "intent cancelled because it was produced during a project graph transition",
        ),
      );
      release();
    }
  };

  return {
    sendIntent,
    sendIntents,
    suspendDispatch,
    blockDispatch,
    runProjectGraphTransition,
    rejectPending,
  };
};
