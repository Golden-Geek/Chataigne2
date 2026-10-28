declare global {
	interface Window {
		__TAURI_INTERNALS__?: {
			invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
		};
	}
}

export function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
	if (!window.__TAURI_INTERNALS__)
		return Promise.reject(new Error('Open this fixture in the native probe.'));
	return window.__TAURI_INTERNALS__.invoke<T>(command, args);
}
