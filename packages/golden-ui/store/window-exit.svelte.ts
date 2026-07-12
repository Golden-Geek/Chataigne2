import {
	forceCloseDesktopWindow,
	hasDesktopHost,
	requestDesktopWindowClose,
	subscribeDesktopWindowCloseRequested
} from '../host/desktop';
import {
	attemptSaveProjectFile,
	hasPendingProjectChanges,
	projectFileDisplayName,
	projectFileState
} from './project-files.svelte';
import { appState } from './workbench.svelte';

export const windowExitState = $state({
	open: false,
	busy: false,
	errorMessage: null as string | null
});

const closeWindowImmediately = async (): Promise<boolean> => {
	if (hasDesktopHost()) {
		const result = await forceCloseDesktopWindow();
		if (result === undefined) {
			console.error('[window-exit] force close failed.');
			return false;
		}
		return true;
	}

	if (typeof window !== 'undefined') {
		window.close();
		return true;
	}

	return false;
};

const handleWindowCloseRequested = async (): Promise<void> => {
	if (windowExitState.busy) {
		return;
	}

	const currentHistoryStateId = appState.session?.currentHistoryStateId;
	if (hasPendingProjectChanges(currentHistoryStateId)) {
		windowExitState.errorMessage = null;
		windowExitState.open = true;
		return;
	}

	windowExitState.open = false;
	windowExitState.errorMessage = null;
	void closeWindowImmediately();
};

export const requestWindowExit = async (): Promise<void> => {
	if (windowExitState.busy) {
		return;
	}

	if (hasDesktopHost()) {
		const result = await requestDesktopWindowClose();
		if (result !== undefined) {
			return;
		}
	}

	await handleWindowCloseRequested();
};

export const cancelWindowExit = (): void => {
	if (windowExitState.busy) {
		return;
	}

	windowExitState.open = false;
	windowExitState.errorMessage = null;
};

export const confirmWindowExitDiscard = async (): Promise<boolean> => {
	if (windowExitState.busy) {
		return false;
	}

	windowExitState.open = false;
	windowExitState.errorMessage = null;
	const closed = await closeWindowImmediately();
	if (!closed) {
		windowExitState.open = true;
	}
	return closed;
};

export const confirmWindowExitSave = async (): Promise<boolean> => {
	if (windowExitState.busy) {
		return false;
	}

	windowExitState.busy = true;
	windowExitState.errorMessage = null;
	try {
		const saveResult = await attemptSaveProjectFile();
		if (!saveResult.ok) {
			if (!saveResult.cancelled) {
				windowExitState.errorMessage = saveResult.message ?? 'Failed to save the project.';
			}
			return false;
		}

		windowExitState.open = false;
		const closed = await closeWindowImmediately();
		if (!closed) {
			windowExitState.open = true;
		}
		return closed;
	} finally {
		windowExitState.busy = false;
	}
};

export const getWindowExitProjectName = (): string =>
	projectFileDisplayName(projectFileState.currentPath);

export const mountWindowExitHandling = (): (() => void) => {
	windowExitState.open = false;
	windowExitState.busy = false;
	windowExitState.errorMessage = null;

	return subscribeDesktopWindowCloseRequested(() => {
		void handleWindowCloseRequested();
	});
};
