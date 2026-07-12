import type { Component } from 'svelte';
import type { UiNodeDto } from '../../../types';

export interface OutlinerRowSupplementEntry {
	component: Component<{ node: UiNodeDto }>;
}

export type OutlinerRowSupplementRegistry = Record<string, OutlinerRowSupplementEntry>;

const customOutlinerRowSupplementRegistry = new Map<string, OutlinerRowSupplementEntry>();

const normalizeOutlinerRowKey = (key: string): string => key.trim();

export const registerOutlinerRowSupplement = (
	key: string,
	entry: OutlinerRowSupplementEntry
): void => {
	const normalizedKey = normalizeOutlinerRowKey(key);
	if (!normalizedKey) {
		throw new Error(
			'Outliner row supplement registration requires a non-empty node type or item kind.'
		);
	}
	customOutlinerRowSupplementRegistry.set(normalizedKey, entry);
};

export const registerOutlinerRowSupplements = (entries: OutlinerRowSupplementRegistry): void => {
	for (const [key, entry] of Object.entries(entries)) {
		registerOutlinerRowSupplement(key, entry);
	}
};

export const unregisterOutlinerRowSupplement = (key: string): void => {
	customOutlinerRowSupplementRegistry.delete(normalizeOutlinerRowKey(key));
};

export const clearCustomOutlinerRowSupplements = (): void => {
	customOutlinerRowSupplementRegistry.clear();
};

export const resolveOutlinerRowSupplement = (
	node: UiNodeDto | null
): OutlinerRowSupplementEntry | null => {
	if (!node) {
		return null;
	}
	const normalizedNodeType = normalizeOutlinerRowKey(node.node_type);
	const normalizedItemKind = normalizeOutlinerRowKey(node.user_item_kind);
	return (
		customOutlinerRowSupplementRegistry.get(normalizedNodeType) ??
		(normalizedItemKind ? customOutlinerRowSupplementRegistry.get(normalizedItemKind) : null) ??
		null
	);
};
