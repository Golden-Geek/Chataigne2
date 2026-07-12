import { appState } from './workbench.svelte';
import type {
	WorkbenchLoadingOperation,
	WorkbenchLoadingStepDefinition,
	WorkbenchSession
} from './workbench.svelte';
import type { UiProjectFileLoadResult, UiProjectLoadRecovery } from '../types';
import { hasDesktopHost, openDesktopFileDialog, saveDesktopFileDialog } from '../host/desktop';
import { requestConfirmation } from './confirmation-dialog.svelte';
import { captureProjectUiState, restoreProjectUiState } from './project-ui-state';
import { loadPersistedLastProjectPath, savePersistedLastProjectPath } from './ui-persistence';
import {
	normalizeProjectFilePath,
	projectFileDialogExtensions,
	projectFileDialogTitle,
	projectFileFilterLabel
} from './project-file-format.svelte';
import { recordPerformanceSample } from './performance-profiler.svelte';

export const BROWSER_PROJECT_DIRECTORY_LABEL = '~/Documents/Chataigne';
export const UNTITLED_PROJECT_LABEL = 'Untitled';
const PROJECT_FILE_LOG_TAG = 'project-file';

const PROJECT_LOAD_STEP_DEFINITIONS: readonly WorkbenchLoadingStepDefinition[] = [
	{ id: 'file', label: 'Read project' },
	{ id: 'runtime', label: 'Load runtime' },
	{ id: 'snapshot', label: 'Refresh graph' },
	{ id: 'workspace', label: 'Rebuild workspace' },
	{ id: 'ready', label: 'Ready' }
] as const;

const NEW_PROJECT_STEP_DEFINITIONS: readonly WorkbenchLoadingStepDefinition[] = [
	{ id: 'runtime', label: 'Reset runtime' },
	{ id: 'snapshot', label: 'Refresh graph' },
	{ id: 'workspace', label: 'Rebuild workspace' },
	{ id: 'ready', label: 'Ready' }
] as const;

const nowMs = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());

export interface ProjectFileActionResult {
	ok: boolean;
	cancelled: boolean;
	message: string | null;
}

const normalizeProjectPath = (value: string | null | undefined): string | null => {
	if (typeof value !== 'string') {
		return null;
	}

	const normalized = value.trim();
	return normalized.length > 0 ? normalized : null;
};

export const projectFileDisplayName = (path: string | null | undefined): string => {
	const normalized = normalizeProjectPath(path);
	if (!normalized) {
		return UNTITLED_PROJECT_LABEL;
	}

	const segments = normalized.split(/[\\/]/);
	const lastSegment = segments[segments.length - 1]?.trim();
	return lastSegment && lastSegment.length > 0 ? lastSegment : normalized;
};

export const hasPendingProjectChanges = (historyStateId: number | null | undefined): boolean =>
	(historyStateId ?? 0) !== projectFileState.savedHistoryStateId;

const persistedLastProjectPath = loadPersistedLastProjectPath();

export const projectFileState = $state({
	currentPath: null as string | null,
	lastOpenedPath: normalizeProjectPath(persistedLastProjectPath),
	busy: false,
	savedHistoryStateId: 0,
	lastRecovery: null as UiProjectLoadRecovery | null
});

const setCurrentProjectPath = (path: string | null): void => {
	projectFileState.currentPath = normalizeProjectPath(path);
};

const rememberLastOpenedPath = (path: string | null): void => {
	const normalized = normalizeProjectPath(path);
	projectFileState.lastOpenedPath = normalized;
	savePersistedLastProjectPath(normalized);
};

export const syncProjectFilePathFromSnapshot = (path: string | null | undefined): void => {
	const normalized = normalizeProjectPath(path);
	setCurrentProjectPath(normalized);
	if (normalized) {
		rememberLastOpenedPath(normalized);
	}
};

const chooseOpenPath = async (): Promise<string | null> => {
	return openDesktopFileDialog({
		allowedExtensions: projectFileDialogExtensions(),
		filterLabel: projectFileFilterLabel(),
		title: projectFileDialogTitle('open'),
		logTag: 'project-files'
	});
};

const chooseSavePath = async (suggestedPath: string | null): Promise<string | null> => {
	return saveDesktopFileDialog({
		suggestedPath: normalizeProjectFilePath(suggestedPath ?? UNTITLED_PROJECT_LABEL),
		allowedExtensions: projectFileDialogExtensions(),
		filterLabel: projectFileFilterLabel(),
		title: projectFileDialogTitle('save'),
		logTag: 'project-files'
	});
};

const markProjectStateClean = (historyStateId = 0): void => {
	projectFileState.savedHistoryStateId = historyStateId;
};

const projectFileActionSucceeded = (): ProjectFileActionResult => ({
	ok: true,
	cancelled: false,
	message: null
});

const projectFileActionCancelled = (): ProjectFileActionResult => ({
	ok: false,
	cancelled: true,
	message: null
});

const toProjectFileErrorMessage = (fallback: string, error: unknown): string => {
	if (error instanceof Error) {
		const message = error.message.trim();
		if (message.length > 0) {
			return `${fallback}: ${message}`;
		}
	}

	return fallback;
};

const appendProjectFileLog = (
	session: WorkbenchSession | null,
	level: 'info' | 'warning' | 'error',
	message: string
): void => {
	session?.appendUiLogRecord(level, PROJECT_FILE_LOG_TAG, message);
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

const projectLoadRecoveryFromUnknown = (value: unknown): UiProjectLoadRecovery | null => {
	if (!isRecord(value)) {
		return null;
	}

	const rawProblems = Array.isArray(value.problems) ? value.problems : [];
	const problems = rawProblems
		.map((problem): UiProjectLoadRecovery['problems'][number] | null => {
			if (!isRecord(problem)) {
				return null;
			}
			const message = typeof problem.message === 'string' ? problem.message.trim() : '';
			if (message.length === 0) {
				return null;
			}
			const stage =
				typeof problem.stage === 'string' && problem.stage.trim().length > 0
					? problem.stage.trim()
					: 'project_load';
			return { stage, message };
		})
		.filter((problem): problem is UiProjectLoadRecovery['problems'][number] => problem !== null);

	return problems.length > 0 ? { problems } : null;
};

const projectLoadRecoveryFromError = (error: unknown): UiProjectLoadRecovery | null => {
	const body = isRecord(error) ? error.body : undefined;
	if (!isRecord(body) || body.recoverable !== true) {
		return null;
	}

	const recovery = projectLoadRecoveryFromUnknown(body.recovery);
	if (recovery) {
		return recovery;
	}

	const message =
		typeof body.error === 'string' && body.error.trim().length > 0
			? body.error.trim()
			: error instanceof Error
				? error.message
				: 'Unknown project load problem';
	return { problems: [{ stage: 'project_load', message }] };
};

const formatProjectRecoveryProblems = (recovery: UiProjectLoadRecovery): string =>
	recovery.problems
		.map((problem) => problem.message)
		.filter((message) => message.length > 0)
		.join('; ');

const confirmProjectRecovery = async (
	displayName: string,
	recovery: UiProjectLoadRecovery
): Promise<boolean> => {
	const problem = formatProjectRecoveryProblems(recovery) || 'Unknown project load problem';
	const action = await requestConfirmation({
		title: 'Project Load Problem',
		message: `There was a problem while opening "${displayName}". Would you like to still try to load the file?`,
		errorMessage: problem,
		actions: [
			{ id: 'keep-closed', label: 'Cancel', tone: 'neutral' },
			{ id: 'recover', label: 'Try Anyway', tone: 'primary', defaultFocus: true }
		]
	});
	return action === 'recover';
};

const appendRecoveryDiagnostics = (
	session: WorkbenchSession,
	recovery: UiProjectLoadRecovery | undefined,
	sourceLabel: string
): void => {
	if (!recovery || recovery.problems.length === 0) {
		projectFileState.lastRecovery = null;
		return;
	}

	projectFileState.lastRecovery = recovery;
	appendProjectFileLog(
		session,
		'warning',
		`Loaded project with recovery (${sourceLabel}): ${formatProjectRecoveryProblems(recovery)}`
	);
};

const loadWithRecoveryPrompt = async (
	session: WorkbenchSession,
	displayName: string,
	loading: WorkbenchLoadingOperation,
	load: (recover: boolean) => Promise<UiProjectFileLoadResult>
): Promise<UiProjectFileLoadResult | null> => {
	try {
		return await load(false);
	} catch (error) {
		const recovery = projectLoadRecoveryFromError(error);
		if (!recovery) {
			throw error;
		}

		loading.update({
			activeStep: 'runtime',
			title: 'Project load problem',
			message: 'Waiting for confirmation',
			detail: formatProjectRecoveryProblems(recovery),
			progress: 0.34
		});

		const confirmed = await confirmProjectRecovery(displayName, recovery);
		if (!confirmed) {
			appendProjectFileLog(
				session,
				'warning',
				`Project load cancelled after recoverable problem: ${formatProjectRecoveryProblems(recovery)}`
			);
			return null;
		}

		loading.update({
			activeStep: 'runtime',
			title: 'Opening project',
			message: 'Trying partial project load',
			detail: displayName,
			progress: 0.42
		});
		return load(true);
	}
};

const projectFileActionFailed = (
	session: WorkbenchSession | null,
	message: string,
	context: Record<string, unknown> = {},
	error?: unknown
): ProjectFileActionResult => {
	if (error === undefined) {
		console.warn('[project-files]', message, context);
	} else {
		console.error('[project-files]', message, context, error);
	}
	appendProjectFileLog(session, 'error', message);
	return {
		ok: false,
		cancelled: false,
		message
	};
};

type ProjectFileOperationStart =
	| { ok: true; session: WorkbenchSession }
	| { ok: false; message: string };

const startProjectFileOperation = (operationName: string): ProjectFileOperationStart => {
	const session = appState.session;
	if (!session) {
		const message = `${operationName} is unavailable because the project session is not ready.`;
		console.warn('[project-files]', message);
		return { ok: false, message };
	}

	if (projectFileState.busy) {
		const message = `${operationName} is already in progress.`;
		console.warn('[project-files]', message);
		appendProjectFileLog(session, 'warning', message);
		return { ok: false, message };
	}

	projectFileState.busy = true;
	return { ok: true, session };
};

const finishProjectFileOperation = (): void => {
	projectFileState.busy = false;
};

const startProjectLoading = (
	session: WorkbenchSession,
	title: string,
	message: string,
	detail: string | null,
	progress = 0.12
): WorkbenchLoadingOperation =>
	session.startLoadingOperation({
		stepDefinitions: PROJECT_LOAD_STEP_DEFINITIONS,
		activeStep: 'file',
		title,
		message,
		detail,
		progress
	});

const saveProjectAtPath = async (
	session: WorkbenchSession,
	path: string
): Promise<ProjectFileActionResult> => {
	const normalizedPath = normalizeProjectFilePath(path);
	const startedAt = nowMs();
	try {
		await session.client.projectSave(
			normalizedPath,
			captureProjectUiState(session.selectedNodesIds)
		);
		setCurrentProjectPath(normalizedPath);
		rememberLastOpenedPath(normalizedPath);
		markProjectStateClean(session.currentHistoryStateId);
		recordPerformanceSample('info', 'project.save', `Saved project: ${normalizedPath}`, {
			path: normalizedPath,
			totalMs: nowMs() - startedAt
		});
		appendProjectFileLog(session, 'info', `Saved project: ${normalizedPath}`);
		return projectFileActionSucceeded();
	} catch (error) {
		return projectFileActionFailed(
			session,
			toProjectFileErrorMessage(`Failed to save project to "${normalizedPath}"`, error),
			{ path: normalizedPath },
			error
		);
	}
};

const loadProjectAtPath = async (session: WorkbenchSession, path: string): Promise<boolean> => {
	const startedAt = nowMs();
	const displayName = projectFileDisplayName(path);
	const loading = startProjectLoading(session, 'Opening project', `Loading ${displayName}`, path);
	try {
		loading.update({
			activeStep: 'runtime',
			title: 'Opening project',
			message: 'Loading project into the runtime',
			detail: displayName,
			progress: 0.34
		});
		const loadStartedAt = nowMs();
		const loadResult = await loadWithRecoveryPrompt(session, displayName, loading, (recover) =>
			session.client.projectLoad(path, { recover })
		);
		if (!loadResult) {
			loading.fail({
				activeStep: 'runtime',
				title: 'Project load cancelled',
				message: 'The project was not loaded',
				detail: displayName,
				progress: 0.34
			});
			return false;
		}
		const loadedPath = loadResult.path;
		restoreProjectUiState(loadResult.ui_state);
		appendRecoveryDiagnostics(session, loadResult.recovery, loadedPath);
		const loadMs = nowMs() - loadStartedAt;
		loading.update({
			activeStep: 'snapshot',
			title: 'Refreshing workspace',
			message: 'Requesting the updated graph',
			detail: displayName,
			progress: 0.62
		});
		const refreshStartedAt = nowMs();
		const refreshed = await session.refreshSnapshot();
		const refreshMs = nowMs() - refreshStartedAt;
		if (!refreshed) {
			loading.fail({
				activeStep: 'snapshot',
				title: 'Project load failed',
				message: 'Could not refresh the workspace graph',
				detail: displayName,
				progress: 0.62
			});
			return false;
		}
		loading.update({
			activeStep: 'workspace',
			title: 'Rebuilding workspace',
			message: 'Restoring panels and selections',
			detail: displayName,
			progress: 0.86
		});
		setCurrentProjectPath(loadedPath);
		rememberLastOpenedPath(loadedPath);
		markProjectStateClean(0);
		recordPerformanceSample('info', 'project.open', `Opened project: ${loadedPath}`, {
			path: loadedPath,
			loadMs,
			refreshMs,
			totalMs: nowMs() - startedAt
		});
		appendProjectFileLog(session, 'info', `Opened project: ${loadedPath}`);
		loading.finish({
			title: loadResult.recovery ? 'Project loaded with recovery' : 'Project loaded',
			message: displayName,
			detail: loadedPath
		});
		return true;
	} catch (error) {
		const message = toProjectFileErrorMessage(`Failed to open project "${path}"`, error);
		console.error('[project-files]', message, { path }, error);
		appendProjectFileLog(session, 'error', message);
		loading.fail({
			activeStep: 'runtime',
			title: 'Project load failed',
			message,
			detail: displayName,
			progress: 0.34
		});
		return false;
	}
};

export const createNewProjectFile = async (): Promise<boolean> => {
	const operation = startProjectFileOperation('Create new project');
	if (!operation.ok) {
		return false;
	}

	let loading: WorkbenchLoadingOperation | null = null;
	try {
		const { session } = operation;
		loading = session.startLoadingOperation({
			stepDefinitions: NEW_PROJECT_STEP_DEFINITIONS,
			activeStep: 'runtime',
			title: 'Creating new project',
			message: 'Resetting the runtime workspace',
			detail: null,
			progress: 0.28
		});
		await session.client.projectNew();
		loading.update({
			activeStep: 'snapshot',
			title: 'Refreshing workspace',
			message: 'Requesting the empty project graph',
			detail: null,
			progress: 0.62
		});
		const refreshed = await session.refreshSnapshot();
		if (!refreshed) {
			loading.fail({
				activeStep: 'snapshot',
				title: 'New project failed',
				message: 'Could not refresh the workspace graph',
				detail: null,
				progress: 0.62
			});
			return false;
		}
		loading.update({
			activeStep: 'workspace',
			title: 'Rebuilding workspace',
			message: 'Preparing the new project interface',
			detail: null,
			progress: 0.86
		});
		setCurrentProjectPath(null);
		projectFileState.lastRecovery = null;
		markProjectStateClean(0);
		loading.finish({
			title: 'New project ready',
			message: UNTITLED_PROJECT_LABEL,
			detail: null
		});
		return true;
	} catch (error) {
		const message = toProjectFileErrorMessage('Failed to create a new project', error);
		console.error('[project-files]', message, error);
		appendProjectFileLog(operation.session, 'error', message);
		loading?.fail({
			activeStep: 'runtime',
			title: 'New project failed',
			message,
			detail: null,
			progress: 0.28
		});
		return false;
	} finally {
		finishProjectFileOperation();
	}
};

const attemptSaveProjectFileAs = async (): Promise<ProjectFileActionResult> => {
	if (!hasDesktopHost()) {
		const message = 'Save As is only available in the desktop app right now.';
		console.info('[project-files]', message);
		appendProjectFileLog(appState.session, 'warning', message);
		return {
			ok: false,
			cancelled: false,
			message
		};
	}

	const operation = startProjectFileOperation('Save project');
	if (!operation.ok) {
		return {
			ok: false,
			cancelled: false,
			message: operation.message
		};
	}

	try {
		let path: string | null = null;
		try {
			path = await chooseSavePath(projectFileState.currentPath);
		} catch (error) {
			return projectFileActionFailed(
				operation.session,
				toProjectFileErrorMessage('Failed to open Save As', error),
				{},
				error
			);
		}

		if (!path) {
			return projectFileActionCancelled();
		}

		return saveProjectAtPath(operation.session, path);
	} finally {
		finishProjectFileOperation();
	}
};

export const attemptSaveProjectFile = async (): Promise<ProjectFileActionResult> => {
	const currentPath = projectFileState.currentPath;
	if (!currentPath) {
		return attemptSaveProjectFileAs();
	}

	const operation = startProjectFileOperation('Save project');
	if (!operation.ok) {
		return {
			ok: false,
			cancelled: false,
			message: operation.message
		};
	}

	try {
		return saveProjectAtPath(operation.session, currentPath);
	} finally {
		finishProjectFileOperation();
	}
};

export const saveProjectFile = async (): Promise<boolean> => (await attemptSaveProjectFile()).ok;

export const saveProjectFileAs = async (): Promise<boolean> =>
	(await attemptSaveProjectFileAs()).ok;

export const openProjectFile = async (): Promise<boolean> => {
	if (!hasDesktopHost()) {
		console.info('[project-files] browser open uses the File menu upload flow.');
		return false;
	}

	const operation = startProjectFileOperation('Open project');
	if (!operation.ok) {
		return false;
	}

	try {
		const path = await chooseOpenPath();
		if (!path) {
			return false;
		}
		return loadProjectAtPath(operation.session, path);
	} finally {
		finishProjectFileOperation();
	}
};

export const reopenLastProjectFile = async (): Promise<boolean> => {
	const preferredPath = projectFileState.currentPath ?? projectFileState.lastOpenedPath;
	if (!preferredPath) {
		return false;
	}

	const operation = startProjectFileOperation('Open project');
	if (!operation.ok) {
		return false;
	}

	try {
		return loadProjectAtPath(operation.session, preferredPath);
	} finally {
		finishProjectFileOperation();
	}
};

export const uploadProjectFileAndLoad = async (file: File): Promise<boolean> => {
	const operation = startProjectFileOperation('Open project');
	if (!operation.ok) {
		return false;
	}

	const fileSize =
		file.size > 1024 * 1024
			? `${(file.size / (1024 * 1024)).toFixed(1)} MB`
			: `${Math.max(1, Math.round(file.size / 1024))} KB`;
	const loading = startProjectLoading(
		operation.session,
		'Opening uploaded project',
		`Reading ${file.name}`,
		fileSize
	);
	try {
		const { session } = operation;
		const contents = await file.text();
		loading.update({
			activeStep: 'runtime',
			title: 'Opening uploaded project',
			message: 'Sending project to the runtime',
			detail: file.name,
			progress: 0.38
		});
		const loadResult = await loadWithRecoveryPrompt(session, file.name, loading, (recover) =>
			session.client.projectUploadLoad(file.name, contents, { recover })
		);
		if (!loadResult) {
			loading.fail({
				activeStep: 'runtime',
				title: 'Project load cancelled',
				message: 'The uploaded project was not loaded',
				detail: file.name,
				progress: 0.38
			});
			return false;
		}
		const path = loadResult.path;
		restoreProjectUiState(loadResult.ui_state);
		appendRecoveryDiagnostics(session, loadResult.recovery, path);
		loading.update({
			activeStep: 'snapshot',
			title: 'Refreshing workspace',
			message: 'Requesting the updated graph',
			detail: projectFileDisplayName(path),
			progress: 0.64
		});
		const refreshed = await session.refreshSnapshot();
		if (!refreshed) {
			loading.fail({
				activeStep: 'snapshot',
				title: 'Project load failed',
				message: 'Could not refresh the workspace graph',
				detail: file.name,
				progress: 0.64
			});
			return false;
		}
		loading.update({
			activeStep: 'workspace',
			title: 'Rebuilding workspace',
			message: 'Restoring panels and selections',
			detail: projectFileDisplayName(path),
			progress: 0.86
		});
		setCurrentProjectPath(path);
		rememberLastOpenedPath(path);
		markProjectStateClean(0);
		loading.finish({
			title: loadResult.recovery ? 'Project loaded with recovery' : 'Project loaded',
			message: projectFileDisplayName(path),
			detail: path
		});
		return true;
	} catch (error) {
		const message = toProjectFileErrorMessage(
			`Failed to load uploaded project "${file.name}"`,
			error
		);
		console.error('[project-files]', message, { fileName: file.name }, error);
		appendProjectFileLog(operation.session, 'error', message);
		loading.fail({
			activeStep: 'runtime',
			title: 'Project load failed',
			message,
			detail: file.name,
			progress: 0.38
		});
		return false;
	} finally {
		finishProjectFileOperation();
	}
};
