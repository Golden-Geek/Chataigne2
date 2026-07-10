const UI_CLIENT_INSTANCE_STORAGE_KEY = 'golden_ui.client_instance.v2';

let cachedUiClientInstanceId: string | null = null;

const hasSessionStorage = (): boolean =>
	typeof window !== 'undefined' && typeof window.sessionStorage !== 'undefined';

const makeFallbackClientInstanceId = (): string => {
	const timePart = Date.now().toString(36);
	const randomPart = Math.random().toString(36).slice(2, 10).padEnd(8, '0');
	return `ui-client-${timePart}-${randomPart}`;
};

const makeUiClientInstanceId = (): string => {
	if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
		return crypto.randomUUID();
	}
	return makeFallbackClientInstanceId();
};

export const getUiClientInstanceId = (): string => {
	if (cachedUiClientInstanceId !== null) {
		return cachedUiClientInstanceId;
	}

	if (!hasSessionStorage()) {
		cachedUiClientInstanceId = makeUiClientInstanceId();
		return cachedUiClientInstanceId;
	}

	try {
		const stored = window.sessionStorage.getItem(UI_CLIENT_INSTANCE_STORAGE_KEY)?.trim() ?? '';
		if (stored.length > 0) {
			cachedUiClientInstanceId = stored;
			return cachedUiClientInstanceId;
		}

		const created = makeUiClientInstanceId();
		window.sessionStorage.setItem(UI_CLIENT_INSTANCE_STORAGE_KEY, created);
		cachedUiClientInstanceId = created;
		return cachedUiClientInstanceId;
	} catch (error) {
		console.warn('[ui transport] Unable to access per-tab client instance id.', error);
		cachedUiClientInstanceId = makeUiClientInstanceId();
		return cachedUiClientInstanceId;
	}
};
