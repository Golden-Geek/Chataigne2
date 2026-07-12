// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}

	interface Window {
		__TAURI__?: {
			window: {
				getCurrentWindow: () => {
					close: () => Promise<void>;
					destroy: () => Promise<void>;
					minimize: () => Promise<void>;
					toggleMaximize: () => Promise<void>;
					isMaximized: () => Promise<boolean>;
					onResized: (handler: () => void) => Promise<() => void>;
				};
			};
		};
		__TAURI_INTERNALS__?: {
			invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
			metadata?: {
				currentWindow?: {
					label?: string;
				};
			};
		};

		__PLATFORM__: 'windows' | 'linux' | 'macos' | 'unknown';
	}
}

export {};
