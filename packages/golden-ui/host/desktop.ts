export interface DesktopCommandArgs {
	[key: string]: unknown;
}

export interface DesktopFileDialogOptions {
	allowedExtensions?: string[];
	suggestedPath?: string | null;
	filterLabel?: string;
	title?: string;
	logTag?: string;
}

const DESKTOP_WINDOW_CLOSE_REQUESTED_EVENT = 'gc-window-close-requested';

const normalizeDialogPath = (value: unknown): string | null => {
	if (typeof value !== 'string') {
		return null;
	}

	const normalized = value.trim();
	return normalized.length > 0 ? normalized : null;
};

export const hasDesktopHost = (): boolean =>
	typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__?.invoke);

export const subscribeDesktopWindowCloseRequested = (handler: () => void): (() => void) => {
	if (typeof window === 'undefined' || !hasDesktopHost()) {
		return () => {};
	}

	const listener = () => {
		handler();
	};
	window.addEventListener(DESKTOP_WINDOW_CLOSE_REQUESTED_EVENT, listener);
	return () => {
		window.removeEventListener(DESKTOP_WINDOW_CLOSE_REQUESTED_EVENT, listener);
	};
};

export const invokeDesktopCommand = async <T = unknown>(
	command: string,
	args?: DesktopCommandArgs,
	logTag = 'desktop-host'
): Promise<T | undefined> => {
	const invoke = window.__TAURI_INTERNALS__?.invoke;
	if (!invoke) {
		return undefined;
	}

	try {
		return (await invoke(command, args)) as T;
	} catch (error) {
		console.error(`[${logTag}] ${command} failed.`, error);
		return undefined;
	}
};

export const openDesktopFileDialog = async (
	options: DesktopFileDialogOptions = {}
): Promise<string | null> => {
	const selected = await invokeDesktopCommand<string | null>(
		'open_file_dialog',
		{
			allowed_extensions: options.allowedExtensions,
			filter_label: options.filterLabel,
			title: options.title
		},
		options.logTag ?? 'desktop-host'
	);

	return normalizeDialogPath(selected);
};

export const saveDesktopFileDialog = async (
	options: DesktopFileDialogOptions = {}
): Promise<string | null> => {
	const invoke = window.__TAURI_INTERNALS__?.invoke;
	if (!invoke) {
		return null;
	}

	try {
		const selected = (await invoke('save_file_dialog', {
			suggested_path: options.suggestedPath ?? undefined,
			allowed_extensions: options.allowedExtensions,
			filter_label: options.filterLabel,
			title: options.title
		})) as string | null;

		return normalizeDialogPath(selected);
	} catch (error) {
		const logTag = options.logTag ?? 'desktop-host';
		console.error(`[${logTag}] save_file_dialog failed.`, error);
		const message =
			error instanceof Error && error.message.trim().length > 0 ? error.message : 'unknown error';
		throw new Error(`Failed to open the desktop save dialog: ${message}`);
	}
};

/**
 * Writes `contents` to `fileName` inside `subdirSegments` (joined onto the
 * OS shared application-data directory), creating the directory if needed.
 * Returns the absolute path written, or null if it failed or there is no
 * desktop host.
 */
export const writeDesktopAppDataFile = async (
	subdirSegments: string[],
	fileName: string,
	contents: string,
	logTag = 'desktop-host'
): Promise<string | null> => {
	const written = await invokeDesktopCommand<string>(
		'write_app_data_file',
		{
			subdirSegments,
			fileName,
			contents
		},
		logTag
	);

	return normalizeDialogPath(written ?? null);
};

/**
 * Writes `contents` to `fileName` inside an explicit directory, creating the
 * directory if needed. Returns the absolute path written, or null if it failed
 * or there is no desktop host.
 */
export const writeDesktopFileInDirectory = async (
	directory: string,
	fileName: string,
	contents: string,
	logTag = 'desktop-host'
): Promise<string | null> => {
	const written = await invokeDesktopCommand<string>(
		'write_file_in_directory',
		{
			directory,
			fileName,
			contents
		},
		logTag
	);

	return normalizeDialogPath(written ?? null);
};

export const requestDesktopWindowClose = async (): Promise<unknown> =>
	invokeDesktopCommand('window_close', undefined, 'window-controls');

export const forceCloseDesktopWindow = async (): Promise<unknown> =>
	invokeDesktopCommand('window_destroy', undefined, 'window-controls');
