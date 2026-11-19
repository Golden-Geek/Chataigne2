import { hasDesktopHost } from './desktop';

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
