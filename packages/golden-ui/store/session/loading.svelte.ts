export type WorkbenchLoadingTone = "busy" | "attention" | "ready";
export type WorkbenchLoadingStepStatus =
  "pending" | "active" | "complete" | "attention";
export type WorkbenchLoadingStepId = string;

export interface WorkbenchLoadingStepDefinition {
  id: WorkbenchLoadingStepId;
  label: string;
}

export interface WorkbenchLoadingStep {
  id: WorkbenchLoadingStepId;
  label: string;
  status: WorkbenchLoadingStepStatus;
}

export interface WorkbenchLoadingState {
  visible: boolean;
  title: string;
  message: string;
  detail: string | null;
  progress: number;
  tone: WorkbenchLoadingTone;
  steps: WorkbenchLoadingStep[];
}

export interface WorkbenchLoadingStateRequest {
  activeStep: WorkbenchLoadingStepId;
  title: string;
  message: string;
  detail?: string | null;
  progress: number;
  visible?: boolean;
  tone?: WorkbenchLoadingTone;
  stepDefinitions?: readonly WorkbenchLoadingStepDefinition[];
}

export interface WorkbenchLoadingStateOptions {
  preserveProgress?: boolean;
}

export interface WorkbenchLoadingFinishRequest {
  activeStep?: WorkbenchLoadingStepId;
  title?: string;
  message?: string;
  detail?: string | null;
  progress?: number;
  tone?: WorkbenchLoadingTone;
}

export interface WorkbenchLoadingOperation {
  update(
    next: WorkbenchLoadingStateRequest,
    options?: WorkbenchLoadingStateOptions,
  ): void;
  finish(next?: WorkbenchLoadingFinishRequest): void;
  fail(next: WorkbenchLoadingFinishRequest): void;
  cancel(): void;
}

export interface WorkbenchLoadingController {
  readonly state: WorkbenchLoadingState;
  clearHideTimer(): void;
  setState(
    next: WorkbenchLoadingStateRequest,
    options?: WorkbenchLoadingStateOptions,
  ): void;
  completeInitial(): void;
  resetInitial(): void;
  startOperation(
    initial: WorkbenchLoadingStateRequest,
  ): WorkbenchLoadingOperation;
}

const DEFAULT_STEP_DEFINITIONS: readonly WorkbenchLoadingStepDefinition[] = [
  { id: "session", label: "Prepare interface" },
  { id: "transport", label: "Connect runtime" },
  { id: "snapshot", label: "Load graph" },
  { id: "apply", label: "Build workspace" },
  { id: "subscribe", label: "Start live updates" },
  { id: "ready", label: "Ready" },
] as const;

const clampProgress = (value: number): number =>
  Math.min(1, Math.max(0, value));

const loadingStepIndex = (
  stepDefinitions: readonly WorkbenchLoadingStepDefinition[],
  stepId: WorkbenchLoadingStepId,
): number => stepDefinitions.findIndex((step) => step.id === stepId);

const lastLoadingStepId = (
  stepDefinitions: readonly WorkbenchLoadingStepDefinition[],
): WorkbenchLoadingStepId =>
  stepDefinitions[stepDefinitions.length - 1]?.id ?? "ready";

const loadingState = ({
  activeStep,
  title,
  message,
  detail = null,
  progress,
  visible = true,
  tone = "busy",
  stepDefinitions = DEFAULT_STEP_DEFINITIONS,
}: WorkbenchLoadingStateRequest): WorkbenchLoadingState => {
  const activeIndex = loadingStepIndex(stepDefinitions, activeStep);
  return {
    visible,
    title,
    message,
    detail,
    progress: clampProgress(progress),
    tone,
    steps: stepDefinitions.map((step, index) => {
      let status: WorkbenchLoadingStepStatus = "pending";
      if (index < activeIndex) {
        status = "complete";
      } else if (index === activeIndex) {
        status = tone === "attention" ? "attention" : "active";
      }
      return {
        id: step.id,
        label: step.label,
        status,
      };
    }),
  };
};

export const createWorkbenchLoadingController = (
  nowMs: () => number,
  minimumInitialVisibleMs: number,
): WorkbenchLoadingController => {
  let initialLoadingStartedAt = nowMs();
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let nextOperationId = 0;
  let activeOperationId: number | null = null;
  let state = $state<WorkbenchLoadingState>(
    loadingState({
      activeStep: "session",
      title: "Opening workspace",
      message: "Preparing the interface",
      detail: null,
      progress: 0.06,
    }),
  );

  const clearHideTimer = (): void => {
    if (hideTimer === null) {
      return;
    }
    clearTimeout(hideTimer);
    hideTimer = null;
  };

  const setState = (
    next: WorkbenchLoadingStateRequest,
    options: WorkbenchLoadingStateOptions = {},
  ): void => {
    clearHideTimer();
    const progress =
      options.preserveProgress === false
        ? next.progress
        : Math.max(state.progress, next.progress);
    state = loadingState({
      ...next,
      progress,
    });
  };

  const scheduleHide = (startedAt: number, minimumVisibleMs: number): void => {
    const visibleElapsedMs = nowMs() - startedAt;
    const delayMs = Math.max(120, minimumVisibleMs - visibleElapsedMs);
    hideTimer = setTimeout(() => {
      hideTimer = null;
      state = {
        ...state,
        visible: false,
      };
    }, delayMs);
  };

  const completeInitial = (): void => {
    clearHideTimer();
    state = loadingState({
      activeStep: "ready",
      title: "Workspace ready",
      message: "Live updates are running",
      detail: null,
      progress: 1,
      tone: "ready",
    });
    scheduleHide(initialLoadingStartedAt, minimumInitialVisibleMs);
  };

  const resetInitial = (): void => {
    clearHideTimer();
    activeOperationId = null;
    initialLoadingStartedAt = nowMs();
    state = loadingState({
      activeStep: "session",
      title: "Opening workspace",
      message: "Preparing the interface",
      detail: null,
      progress: 0.06,
    });
  };

  const startOperation = (
    initial: WorkbenchLoadingStateRequest,
  ): WorkbenchLoadingOperation => {
    nextOperationId += 1;
    const operationId = nextOperationId;
    activeOperationId = operationId;
    const startedAt = nowMs();
    let stepDefinitions = initial.stepDefinitions ?? DEFAULT_STEP_DEFINITIONS;

    const isActive = (): boolean => activeOperationId === operationId;
    const withSteps = (
      next: WorkbenchLoadingStateRequest,
    ): WorkbenchLoadingStateRequest => {
      stepDefinitions = next.stepDefinitions ?? stepDefinitions;
      return {
        ...next,
        stepDefinitions,
      };
    };

    setState(withSteps(initial), { preserveProgress: false });

    return {
      update: (
        next: WorkbenchLoadingStateRequest,
        options: WorkbenchLoadingStateOptions = {},
      ): void => {
        if (isActive()) {
          setState(withSteps(next), options);
        }
      },
      finish: (next: WorkbenchLoadingFinishRequest = {}): void => {
        if (!isActive()) {
          return;
        }
        activeOperationId = null;
        clearHideTimer();
        state = loadingState({
          stepDefinitions,
          activeStep: next.activeStep ?? lastLoadingStepId(stepDefinitions),
          title: next.title ?? "Done",
          message: next.message ?? "Operation complete",
          detail: next.detail ?? null,
          progress: next.progress ?? 1,
          tone: next.tone ?? "ready",
        });
        scheduleHide(startedAt, minimumInitialVisibleMs);
      },
      fail: (next: WorkbenchLoadingFinishRequest): void => {
        if (!isActive()) {
          return;
        }
        activeOperationId = null;
        clearHideTimer();
        state = loadingState({
          stepDefinitions,
          activeStep: next.activeStep ?? lastLoadingStepId(stepDefinitions),
          title: next.title ?? "Operation failed",
          message: next.message ?? "The requested operation could not finish",
          detail: next.detail ?? null,
          progress: next.progress ?? state.progress,
          tone: next.tone ?? "attention",
        });
        scheduleHide(startedAt, 1800);
      },
      cancel: (): void => {
        if (!isActive()) {
          return;
        }
        activeOperationId = null;
        clearHideTimer();
        state = {
          ...state,
          visible: false,
        };
      },
    };
  };

  return {
    get state(): WorkbenchLoadingState {
      return state;
    },
    clearHideTimer,
    setState,
    completeInitial,
    resetInitial,
    startOperation,
  };
};
