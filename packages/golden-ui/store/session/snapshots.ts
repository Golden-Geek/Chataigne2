import type { UiSnapshot } from "../../types";

export interface WorkbenchSnapshotControllerOptions {
  requestSnapshot(): Promise<UiSnapshot>;
  applySnapshot(snapshot: UiSnapshot): void;
  logSnapshotTiming(
    context: "refresh" | "resync",
    snapshot: UiSnapshot,
    applyStartedAt: number,
  ): void;
  appendTransportLog(level: "info" | "error", message: string): void;
  suspendIntentDispatch(): () => void;
  snapshotApplied(): void;
  nowMs(): number;
}

export interface WorkbenchSnapshotController {
  request(): Promise<UiSnapshot>;
  refresh(): Promise<boolean>;
  refreshAfterCurrent(): Promise<boolean>;
  resync(successMessage?: string): Promise<void>;
  beginSynchronization(): () => void;
  waitForIdle(): Promise<void>;
}

export const createWorkbenchSnapshotController = (
  options: WorkbenchSnapshotControllerOptions,
): WorkbenchSnapshotController => {
  let requestInFlight: Promise<UiSnapshot> | null = null;
  let synchronizationsInFlight = 0;
  let resyncInFlight = false;
  let resyncQueued = false;
  const idleWaiters: Array<() => void> = [];

  const notifyIdle = (): void => {
    if (synchronizationsInFlight > 0 || requestInFlight !== null) {
      return;
    }
    for (const resolve of idleWaiters.splice(0)) {
      resolve();
    }
  };

  const beginSynchronization = (): (() => void) => {
    synchronizationsInFlight += 1;
    let ended = false;
    return () => {
      if (ended) {
        return;
      }
      ended = true;
      synchronizationsInFlight = Math.max(0, synchronizationsInFlight - 1);
      notifyIdle();
    };
  };

  const waitForIdle = (): Promise<void> => {
    if (synchronizationsInFlight === 0 && requestInFlight === null) {
      return Promise.resolve();
    }
    return new Promise<void>((resolve) => {
      idleWaiters.push(resolve);
    });
  };

  const request = (): Promise<UiSnapshot> => {
    if (requestInFlight) {
      return requestInFlight;
    }
    const inFlight = options.requestSnapshot();
    requestInFlight = inFlight.finally(() => {
      requestInFlight = null;
      notifyIdle();
    });
    return requestInFlight;
  };

  const refresh = async (): Promise<boolean> => {
    const endSynchronization = beginSynchronization();
    try {
      const snapshot = await request();
      const applyStartedAt = options.nowMs();
      options.applySnapshot(snapshot);
      options.snapshotApplied();
      options.logSnapshotTiming("refresh", snapshot, applyStartedAt);
      return true;
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "unknown refresh error";
      options.appendTransportLog(
        "error",
        `Snapshot refresh failed: ${message}`,
      );
      return false;
    } finally {
      endSynchronization();
    }
  };

  const refreshAfterCurrent = async (): Promise<boolean> => {
    await waitForIdle();
    return refresh();
  };

  const resync = async (successMessage?: string): Promise<void> => {
    if (resyncInFlight) {
      resyncQueued = true;
      return;
    }
    resyncInFlight = true;
    resyncQueued = false;
    const endSynchronization = beginSynchronization();
    const releaseIntentDispatch = options.suspendIntentDispatch();
    try {
      const snapshot = await request();
      const applyStartedAt = options.nowMs();
      options.applySnapshot(snapshot);
      options.snapshotApplied();
      if (successMessage) {
        options.appendTransportLog("info", successMessage);
      }
      options.logSnapshotTiming("resync", snapshot, applyStartedAt);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "unknown resync error";
      options.appendTransportLog("error", `Resync failed: ${message}`);
    } finally {
      resyncInFlight = false;
      if (resyncQueued) {
        resyncQueued = false;
        void resync(successMessage);
      }
      releaseIntentDispatch();
      endSynchronization();
    }
  };

  return {
    request,
    refresh,
    refreshAfterCurrent,
    resync,
    beginSynchronization,
    waitForIdle,
  };
};
