export interface ProjectGraphTransitionControl {
  markRuntimeGraphReplaced(): void;
}

export interface ProjectGraphTransitionOptions {
  runIntentBarrier<T>(operation: () => Promise<T>): Promise<T>;
  blockIntentDispatch(reason: Error): () => void;
  rejectPendingIntents(reason: Error): void;
  waitForSnapshotIdle(): Promise<void>;
  afterTransitionOperation(): Promise<void>;
}

export interface ProjectGraphTransitionController {
  readonly revision: number;
  run<T>(
    operation: (control: ProjectGraphTransitionControl) => Promise<T>,
  ): Promise<T>;
  requiredSnapshotFinished(success: boolean): void;
  snapshotApplied(): void;
}

type PersistentGatePhase = "required" | "recovery";

interface PersistentGate {
  phase: PersistentGatePhase;
  release: () => void;
}

export const createProjectGraphTransitionController = (
  options: ProjectGraphTransitionOptions,
): ProjectGraphTransitionController => {
  let revision = $state(0);
  let activeTransitions = 0;
  let persistentGate: PersistentGate | null = null;

  const markRuntimeGraphReplaced = (): void => {
    if (persistentGate) {
      persistentGate.phase = "required";
      return;
    }
    persistentGate = {
      phase: "required",
      release: options.blockIntentDispatch(
        new Error(
          "intent rejected because the runtime graph is not synchronized",
        ),
      ),
    };
  };

  const completeRecovery = (): void => {
    const gate = persistentGate;
    if (!gate) {
      return;
    }
    options.rejectPendingIntents(
      new Error(
        "intent cancelled because the runtime graph required snapshot recovery",
      ),
    );
    persistentGate = null;
    gate.release();
    if (activeTransitions === 0) {
      revision += 1;
    }
  };

  const requiredSnapshotFinished = (success: boolean): void => {
    if (!persistentGate || persistentGate.phase !== "required") {
      return;
    }
    if (success) {
      completeRecovery();
    } else {
      persistentGate.phase = "recovery";
    }
  };

  const snapshotApplied = (): void => {
    if (persistentGate?.phase === "recovery") {
      completeRecovery();
    }
  };

  const run = async <T>(
    operation: (control: ProjectGraphTransitionControl) => Promise<T>,
  ): Promise<T> => {
    activeTransitions += 1;
    let controlActive = true;
    const control: ProjectGraphTransitionControl = {
      markRuntimeGraphReplaced: () => {
        if (!controlActive) {
          throw new Error(
            "project graph transition control is no longer active",
          );
        }
        markRuntimeGraphReplaced();
      },
    };

    try {
      return await options.runIntentBarrier(async () => {
        await options.waitForSnapshotIdle();
        try {
          return await operation(control);
        } finally {
          if (persistentGate?.phase === "required") {
            await options.waitForSnapshotIdle();
            if (persistentGate?.phase === "required") {
              persistentGate.phase = "recovery";
            }
          }
          await options.afterTransitionOperation();
        }
      });
    } finally {
      controlActive = false;
      activeTransitions = Math.max(0, activeTransitions - 1);
      if (!persistentGate && activeTransitions === 0) {
        revision += 1;
      }
    }
  };

  return {
    get revision(): number {
      return revision;
    },
    run,
    requiredSnapshotFinished,
    snapshotApplied,
  };
};
