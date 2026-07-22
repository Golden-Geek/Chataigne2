import type { UiCreatableUserItem } from '../../types';
import { getIconURLForNodeType, getIconURLForCategory } from '../../store/node-types';
import type { ContextMenuItem } from './context-menu';

type CreateItemHandler = (item: UiCreatableUserItem) => void | Promise<void>;

type MenuEntry = MenuBranchEntry | MenuLeafEntry;

interface MenuBranchEntry {
	kind: 'branch';
	label: string;
	path: string[];
	color?: string;
	children: MenuEntry[];
}

interface MenuLeafEntry {
	kind: 'leaf';
	item: UiCreatableUserItem;
}

const normalizeMenuPath = (path: readonly string[] | undefined): string[] => {
	return (path ?? []).map((segment) => segment.trim()).filter((segment) => segment.length > 0);
};

const findOrCreateBranch = (
	entries: MenuEntry[],
	label: string,
	parentPath: readonly string[],
	color?: string
): MenuBranchEntry => {
	const existing = entries.find(
		(entry): entry is MenuBranchEntry => entry.kind === 'branch' && entry.label === label
	);
	if (existing) {
		existing.color ??= color;
		return existing;
	}

	const branch = {
		kind: 'branch',
		label,
		path: [...parentPath, label],
		color,
		children: []
	} satisfies MenuBranchEntry;
	entries.push(branch);
	return branch;
};

const createLeafItem = (
	item: UiCreatableUserItem,
	onCreateItem: CreateItemHandler
): ContextMenuItem => ({
	id: `create:${[...normalizeMenuPath(item.menu_path), item.node_type, item.item_kind].join('/')}`,
	label: item.label,
	icon: item.icon ?? getIconURLForNodeType(item.node_type, item.item_kind) ?? undefined,
	color: item.color,
	action: () => {
		void onCreateItem(item);
	}
});

const createBranchItem = (
	branch: MenuBranchEntry,
	onCreateItem: CreateItemHandler
): ContextMenuItem => ({
	id: `create-path:${branch.path.join('/')}`,
	label: branch.label,
	icon: getIconURLForCategory(branch.label) ?? undefined,
	color: branch.color,
	submenu: finalizeEntries(branch.children, onCreateItem)
});

const finalizeEntries = (
	entries: readonly MenuEntry[],
	onCreateItem: CreateItemHandler
): ContextMenuItem[] => {
	const branches = entries.filter((entry): entry is MenuBranchEntry => entry.kind === 'branch');
	const leaves = entries.filter((entry): entry is MenuLeafEntry => entry.kind === 'leaf');
	const regularBranches = branches.filter((branch) => branch.label !== 'Generic');
	const genericBranches = branches.filter((branch) => branch.label === 'Generic');

	const items = regularBranches.map((branch) => createBranchItem(branch, onCreateItem));
	if (regularBranches.length > 0 && genericBranches.length > 0) {
		items.push({ separator: true });
	}
	items.push(...genericBranches.map((branch) => createBranchItem(branch, onCreateItem)));
	if (branches.length > 0 && leaves.length > 0) {
		items.push({ separator: true });
	}
	for (const leaf of leaves) {
		const previous = items[items.length - 1];
		if (leaf.item.separator_before && previous && !previous.separator) {
			items.push({ separator: true });
		}
		items.push(createLeafItem(leaf.item, onCreateItem));
	}
	return items;
};

export const buildCreatableItemMenu = (
	items: readonly UiCreatableUserItem[],
	onCreateItem: CreateItemHandler
): ContextMenuItem[] => {
	const rootEntries: MenuEntry[] = [];

	for (const item of items) {
		const path = normalizeMenuPath(item.menu_path);
		const pathColors = item.menu_path_colors ?? [];
		let entries = rootEntries;
		const parentPath: string[] = [];

		for (const [index, segment] of path.entries()) {
			const branch = findOrCreateBranch(entries, segment, parentPath, pathColors[index]);
			entries = branch.children;
			parentPath.push(segment);
		}

		entries.push({ kind: 'leaf', item });
	}

	return finalizeEntries(rootEntries, onCreateItem);
};
