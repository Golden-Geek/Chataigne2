import type { GraphState } from '../../store/graph.svelte';
import type { WorkbenchSession } from '../../store/workbench.svelte';
import type { UiNodeDto, UiNodeMetaDto } from '../../types';
import { normalizeContextMenuItems, type ContextMenuItem } from './context-menu';

export interface NodeContextMenuContributorContext {
	node: UiNodeDto;
	parentNode: UiNodeDto | null;
	graphState: GraphState | null;
	session: WorkbenchSession | null;
	closeMenu: () => void;
	patchMeta: (node: number, patch: Partial<UiNodeMetaDto>) => Promise<boolean>;
}

export interface NodeContextMenuContributorEntry {
	match?: (context: NodeContextMenuContributorContext) => boolean;
	items: (
		context: NodeContextMenuContributorContext
	) => readonly ContextMenuItem[] | null | undefined;
}

export type NodeContextMenuContributorRegistry = Record<string, NodeContextMenuContributorEntry>;

const customNodeContextMenuContributorRegistry = new Map<string, NodeContextMenuContributorEntry>();

const normalizeContributorKey = (key: string): string => key.trim();

export const registerNodeContextMenuContributor = (
	key: string,
	entry: NodeContextMenuContributorEntry
): void => {
	const normalizedKey = normalizeContributorKey(key);
	if (!normalizedKey) {
		throw new Error('Node context menu registration requires a non-empty key.');
	}
	customNodeContextMenuContributorRegistry.set(normalizedKey, entry);
};

export const registerNodeContextMenuContributors = (
	entries: NodeContextMenuContributorRegistry
): void => {
	for (const [key, entry] of Object.entries(entries)) {
		registerNodeContextMenuContributor(key, entry);
	}
};

export const unregisterNodeContextMenuContributor = (key: string): void => {
	customNodeContextMenuContributorRegistry.delete(normalizeContributorKey(key));
};

export const clearCustomNodeContextMenuContributors = (): void => {
	customNodeContextMenuContributorRegistry.clear();
};

export const resolveNodeContextMenuItems = (
	context: NodeContextMenuContributorContext
): ContextMenuItem[] => {
	const items: ContextMenuItem[] = [];
	for (const entry of customNodeContextMenuContributorRegistry.values()) {
		if (entry.match && !entry.match(context)) {
			continue;
		}
		items.push(...normalizeContextMenuItems(entry.items(context)));
	}
	return normalizeContextMenuItems(items);
};
