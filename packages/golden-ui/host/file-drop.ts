import { hasDesktopHost } from './desktop';

const EXPLORER_FILE = 'application/x-golden-explorer-file';
let explorerDrag: { path: string; token: string; current: () => boolean } | null = null;

export function beginExplorerFileDrag(
	event: DragEvent,
	path: string,
	current: () => boolean
): void {
	if (!event.dataTransfer) return;
	const token = crypto.randomUUID();
	explorerDrag = { path, token, current };
	event.dataTransfer.effectAllowed = 'copy';
	event.dataTransfer.setData(EXPLORER_FILE, token);
}

export function endExplorerFileDrag(): void {
	explorerDrag = null;
}

export function explorerFileDragPath(event: DragEvent): string | null {
	return explorerDrag?.current() && event.dataTransfer?.types.includes(EXPLORER_FILE)
		? explorerDrag.path
		: null;
}

export function isWorkbenchFileDrag(event: DragEvent): boolean {
	return (
		explorerFileDragPath(event) !== null ||
		(hasDesktopFileDrop() && (event.dataTransfer?.types.includes('Files') ?? false))
	);
}

export function readWorkbenchFileDrop(event: DragEvent, signal: AbortSignal): Promise<string[]> {
	const transfer = event.dataTransfer;
	if (explorerDrag?.current() && transfer?.getData(EXPLORER_FILE) === explorerDrag.token)
		return Promise.resolve([explorerDrag.path]);
	if (transfer?.files.length && hasDesktopFileDrop())
		return readDroppedFilePaths(transfer.files, signal);
	return Promise.resolve([]);
}

interface WebViewFileBridge extends EventTarget {
	postMessageWithAdditionalObjects(message: unknown, files: File[]): void;
}

function fileBridge(): WebViewFileBridge | undefined {
	if (!hasDesktopHost()) return undefined;
	const host = window as Window & { chrome?: { webview?: WebViewFileBridge } };
	const bridge = host.chrome?.webview;
	return typeof bridge?.postMessageWithAdditionalObjects === 'function' ? bridge : undefined;
}

export function hasDesktopFileDrop(): boolean {
	return fileBridge() !== undefined;
}

/** Resolve only the native files made available by the drop event. */
export async function readDroppedFilePaths(
	files: FileList,
	signal: AbortSignal
): Promise<string[]> {
	const bridge = fileBridge();
	if (!bridge) throw new Error('Desktop file drops are unavailable.');
	signal.throwIfAborted();
	const id = crypto.randomUUID();
	const dropped = Array.from(files);
	return new Promise((resolve, reject) => {
		const kind = 'golden:file-drop';
		const cleanup = () => {
			clearTimeout(timeout);
			bridge.removeEventListener('message', receive);
			signal.removeEventListener('abort', cancel);
		};
		const fail = (error: unknown) => {
			cleanup();
			reject(error);
		};
		const cancel = () => fail(signal.reason);
		const receive: EventListener = (event) => {
			const value = (event as MessageEvent).data;
			if (value?.kind !== kind || value.id !== id) return;
			cleanup();
			if (
				Array.isArray(value.paths) &&
				value.paths.length === dropped.length &&
				value.paths.every((path: unknown) => typeof path === 'string' && path.length > 0)
			) {
				resolve(value.paths);
			} else {
				reject(new Error('The drop does not contain native files.'));
			}
		};
		const timeout = setTimeout(() => fail(new Error('Could not open the dropped files.')), 2500);
		bridge.addEventListener('message', receive);
		signal.addEventListener('abort', cancel, { once: true });
		try {
			bridge.postMessageWithAdditionalObjects({ kind, id }, dropped);
		} catch (error) {
			fail(error);
		}
	});
}
