import type { SerializedDockview } from 'dockview-core';
import type { NodeId } from '../types';
import { applicationStorageKey } from './application.svelte';

const layoutKey = (): string | null => applicationStorageKey('layout.v1');
const selectionKey = (): string | null => applicationStorageKey('selection.v1');
const projectPathKey = (): string | null => applicationStorageKey('project-path.v1');
const PERSISTENCE_VERSION = 1;

interface PersistedEnvelope<T> {
	version: number;
	savedAt: string;
	payload: T;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

const hasLocalStorage = (): boolean =>
	typeof window !== 'undefined' && typeof window.localStorage !== 'undefined';

const readStoredJson = (key: string | null): unknown | null => {
	if (key === null || !hasLocalStorage()) {
		return null;
	}

	try {
		const raw = window.localStorage.getItem(key);
		if (!raw) {
			return null;
		}
		return JSON.parse(raw);
	} catch (error) {
		console.warn(`[ui-persistence] Unable to read "${key}".`, error);
		return null;
	}
};

const writeStoredJson = (key: string | null, payload: unknown): void => {
	if (key === null || !hasLocalStorage()) {
		return;
	}

	try {
		const envelope: PersistedEnvelope<unknown> = {
			version: PERSISTENCE_VERSION,
			savedAt: new Date().toISOString(),
			payload
		};
		window.localStorage.setItem(key, JSON.stringify(envelope));
	} catch (error) {
		console.warn(`[ui-persistence] Unable to write "${key}".`, error);
	}
};

const removeStoredValue = (key: string | null): void => {
	if (key === null || !hasLocalStorage()) {
		return;
	}

	try {
		window.localStorage.removeItem(key);
	} catch (error) {
		console.warn(`[ui-persistence] Unable to remove "${key}".`, error);
	}
};

const readStoredPayload = (key: string | null): unknown | null => {
	const value = readStoredJson(key);
	if (!isRecord(value)) {
		return null;
	}
	if (value.version !== PERSISTENCE_VERSION) {
		return null;
	}
	return value.payload ?? null;
};

const asSerializedDockview = (value: unknown): SerializedDockview | null => {
	if (!isRecord(value)) {
		return null;
	}
	if (!isRecord(value.grid) || !isRecord(value.panels)) {
		return null;
	}
	return value as unknown as SerializedDockview;
};

const asNodeIdArray = (value: unknown): NodeId[] => {
	if (!Array.isArray(value)) {
		return [];
	}

	const uniqueNodeIds = new Set<NodeId>();
	for (const candidate of value) {
		if (typeof candidate !== 'number' || !Number.isFinite(candidate)) {
			continue;
		}
		uniqueNodeIds.add(candidate);
	}

	return Array.from(uniqueNodeIds);
};

const asStoredProjectPath = (value: unknown): string | null => {
	if (typeof value !== 'string') {
		return null;
	}

	const normalized = value.trim();
	return normalized.length > 0 ? normalized : null;
};

export const loadPersistedDockLayout = (): SerializedDockview | null => {
	const payload = readStoredPayload(layoutKey());
	return asSerializedDockview(payload);
};

export const savePersistedDockLayout = (layout: SerializedDockview): void => {
	writeStoredJson(layoutKey(), layout);
};

export const loadPersistedSelection = (): NodeId[] => {
	const payload = readStoredPayload(selectionKey());
	return asNodeIdArray(payload);
};

export const savePersistedSelection = (nodeIds: NodeId[]): void => {
	writeStoredJson(selectionKey(), nodeIds);
};

export const loadPersistedLastProjectPath = (): string | null => {
	const payload = readStoredPayload(projectPathKey());
	return asStoredProjectPath(payload);
};

export const savePersistedLastProjectPath = (path: string | null): void => {
	const normalized = asStoredProjectPath(path);
	if (normalized === null) {
		removeStoredValue(projectPathKey());
		return;
	}

	writeStoredJson(projectPathKey(), normalized);
};

export const clearPersistedUiState = (): void => {
	if (!hasLocalStorage()) {
		return;
	}

	try {
		for (const key of [layoutKey(), selectionKey(), projectPathKey()]) {
			if (key !== null) window.localStorage.removeItem(key);
		}
	} catch (error) {
		console.warn('[ui-persistence] Unable to clear persisted UI state.', error);
	}
};
