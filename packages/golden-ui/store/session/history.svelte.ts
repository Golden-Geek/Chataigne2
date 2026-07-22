import type { UiHistoryState } from '../../types';

export interface WorkbenchHistoryStore {
	readonly historyBusy: boolean;
	readonly canUndo: boolean;
	readonly canRedo: boolean;
	readonly hasActiveEditSession: boolean;
	readonly editSessionEpoch: number;
	readonly currentHistoryStateId: number;
	applyHistoryState(history: UiHistoryState | undefined): void;
	undo(runUndo: () => Promise<void>): Promise<void>;
	redo(runRedo: () => Promise<void>): Promise<void>;
	reset(): void;
}

export const createWorkbenchHistoryStore = (): WorkbenchHistoryStore => {
	let historyBusy = $state(false);
	let canUndo = $state(false);
	let canRedo = $state(false);
	let hasActiveEditSession = $state(false);
	let editSessionEpoch = $state(0);
	let currentHistoryStateId = $state(0);

	const applyHistoryState = (history: UiHistoryState | undefined): void => {
		if (!history) {
			return;
		}
		canUndo = history.can_undo;
		canRedo = history.can_redo;
		if (hasActiveEditSession !== history.active_edit_session) {
			editSessionEpoch += 1;
		}
		hasActiveEditSession = history.active_edit_session;
		currentHistoryStateId = history.current_history_state_id;
	};

	const runHistoryAction = async (canRun: boolean, action: () => Promise<void>): Promise<void> => {
		if (historyBusy || !canRun) {
			return;
		}
		historyBusy = true;
		try {
			await action();
		} finally {
			historyBusy = false;
		}
	};

	const undo = (runUndo: () => Promise<void>): Promise<void> => {
		return runHistoryAction(canUndo, runUndo);
	};

	const redo = (runRedo: () => Promise<void>): Promise<void> => {
		return runHistoryAction(canRedo, runRedo);
	};

	const reset = (): void => {
		historyBusy = false;
		canUndo = false;
		canRedo = false;
		hasActiveEditSession = false;
		editSessionEpoch = 0;
		currentHistoryStateId = 0;
	};

	return {
		get historyBusy(): boolean {
			return historyBusy;
		},
		get canUndo(): boolean {
			return canUndo;
		},
		get canRedo(): boolean {
			return canRedo;
		},
		get hasActiveEditSession(): boolean {
			return hasActiveEditSession;
		},
		get editSessionEpoch(): number {
			return editSessionEpoch;
		},
		get currentHistoryStateId(): number {
			return currentHistoryStateId;
		},
		applyHistoryState,
		undo,
		redo,
		reset
	};
};
