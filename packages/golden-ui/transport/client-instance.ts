const cachedUiClientInstanceIds = new Map<string | undefined, string>();

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

export const getUiClientInstanceId = (applicationId?: string): string => {
	const cached = cachedUiClientInstanceIds.get(applicationId);
	if (cached !== undefined) {
		return cached;
	}

	if (applicationId === undefined || !hasSessionStorage()) {
		const created = makeUiClientInstanceId();
		cachedUiClientInstanceIds.set(applicationId, created);
		return created;
	}

	try {
		const storageKey = `golden_ui.${encodeURIComponent(applicationId)}.client_instance.v2`;
		const stored = window.sessionStorage.getItem(storageKey)?.trim() ?? '';
		if (stored.length > 0) {
			cachedUiClientInstanceIds.set(applicationId, stored);
			return stored;
		}

		const created = makeUiClientInstanceId();
		window.sessionStorage.setItem(storageKey, created);
		cachedUiClientInstanceIds.set(applicationId, created);
		return created;
	} catch (error) {
		console.warn('[ui transport] Unable to access per-tab client instance id.', error);
		const created = makeUiClientInstanceId();
		cachedUiClientInstanceIds.set(applicationId, created);
		return created;
	}
};
