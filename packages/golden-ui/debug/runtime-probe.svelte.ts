const isBrowser = typeof window !== 'undefined';

export type RuntimeProbeEntrySource = 'error' | 'unhandledrejection';

export interface RuntimeProbeEntry {
	id: number;
	source: RuntimeProbeEntrySource;
	message: string;
	stack: string | null;
	detail: string | null;
	timestamp: string;
}

interface RuntimeProbeBridge {
	snapshot: () => RuntimeProbeEntry[];
	clear: () => void;
	setOverlayEnabled: (enabled: boolean) => void;
}

declare global {
	interface Window {
		__GC_RUNTIME_PROBE__?: RuntimeProbeBridge;
	}
}

const maxRuntimeProbeEntries = 32;

export const runtimeProbe = $state({
	installed: false,
	overlayEnabled: false,
	entries: []
} as {
	installed: boolean;
	overlayEnabled: boolean;
	entries: RuntimeProbeEntry[];
});

let nextRuntimeProbeEntryId = 1;

const normalizeErrorLike = (
	value: unknown
): Pick<RuntimeProbeEntry, 'message' | 'stack' | 'detail'> => {
	if (value instanceof Error) {
		return {
			message: value.message || value.name,
			stack: value.stack ?? null,
			detail: value.name !== 'Error' ? value.name : null
		};
	}
	if (typeof value === 'string') {
		return {
			message: value,
			stack: null,
			detail: null
		};
	}
	if (value && typeof value === 'object') {
		try {
			return {
				message: JSON.stringify(value),
				stack: null,
				detail: Object.prototype.toString.call(value)
			};
		} catch {
			return {
				message: String(value),
				stack: null,
				detail: Object.prototype.toString.call(value)
			};
		}
	}
	return {
		message: String(value),
		stack: null,
		detail: null
	};
};

const readOverlayEnabled = (): boolean => {
	if (!isBrowser) {
		return false;
	}
	const url = new URL(window.location.href);
	const explicit = url.searchParams.get('gc_debug_runtime');
	if (explicit === '1') {
		window.localStorage.setItem('gc:debug-runtime', '1');
		return true;
	}
	if (explicit === '0') {
		window.localStorage.removeItem('gc:debug-runtime');
		return false;
	}
	return window.localStorage.getItem('gc:debug-runtime') === '1';
};

const pushRuntimeProbeEntry = (
	source: RuntimeProbeEntrySource,
	value: unknown,
	stackOverride: string | null = null
): void => {
	const normalized = normalizeErrorLike(value);
	const nextEntry: RuntimeProbeEntry = {
		id: nextRuntimeProbeEntryId++,
		source,
		message: normalized.message,
		stack: stackOverride ?? normalized.stack,
		detail: normalized.detail,
		timestamp: new Date().toISOString()
	};
	runtimeProbe.entries = [nextEntry, ...runtimeProbe.entries].slice(0, maxRuntimeProbeEntries);
};

export const clearRuntimeProbe = (): void => {
	runtimeProbe.entries = [];
};

export const setRuntimeProbeOverlayEnabled = (enabled: boolean): void => {
	runtimeProbe.overlayEnabled = enabled;
	if (!isBrowser) {
		return;
	}
	if (enabled) {
		window.localStorage.setItem('gc:debug-runtime', '1');
		return;
	}
	window.localStorage.removeItem('gc:debug-runtime');
};

export const ensureRuntimeProbeInstalled = (): void => {
	if (!isBrowser) {
		return;
	}
	runtimeProbe.overlayEnabled = readOverlayEnabled();
	if (window.__GC_RUNTIME_PROBE__) {
		runtimeProbe.installed = true;
		return;
	}

	const syncOverlayEnabled = (): void => {
		runtimeProbe.overlayEnabled = readOverlayEnabled();
	};

	window.addEventListener('popstate', syncOverlayEnabled);
	window.addEventListener('hashchange', syncOverlayEnabled);
	window.addEventListener('error', (event) => {
		pushRuntimeProbeEntry(
			'error',
			event.error ?? event.message ?? 'Unknown window error',
			event.error instanceof Error ? (event.error.stack ?? null) : null
		);
	});
	window.addEventListener('unhandledrejection', (event) => {
		pushRuntimeProbeEntry('unhandledrejection', event.reason ?? 'Unhandled promise rejection');
	});

	window.__GC_RUNTIME_PROBE__ = {
		snapshot: () => runtimeProbe.entries.map((entry) => ({ ...entry })),
		clear: clearRuntimeProbe,
		setOverlayEnabled: setRuntimeProbeOverlayEnabled
	};

	runtimeProbe.installed = true;
};
