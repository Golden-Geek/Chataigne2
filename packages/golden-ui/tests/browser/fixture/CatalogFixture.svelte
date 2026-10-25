<script lang="ts">
	import { tick } from 'svelte';
	import ContextMenu from '../../../components/common/ContextMenu.svelte';
	import { buildCreatableItemMenu } from '../../../components/common/creatable-item-menu';
	import type { ContextMenuItem } from '../../../components/common/context-menu';
	import type { UiCreatableUserItem } from '../../../types';
	import { measureCollectionFlow } from './timing';
	let menu = $state<ReturnType<typeof ContextMenu>>();
	let items = $state.raw<ContextMenuItem[]>([]);
	let open = $state(false);
	let maxWidthCss = $state('max-content');
	let count = 0;
	const frames = async () => {
		await tick();
		await new Promise(requestAnimationFrame);
		await tick();
		await new Promise(requestAnimationFrame);
	};
	export async function setup(size: number) {
		count = size;
		maxWidthCss = 'max-content';
		const descriptors: UiCreatableUserItem[] = Array.from({ length: size }, (_, index) => ({
			node_type: 'FixtureItem',
			item_kind: String(index),
			label: `Catalog ${String(index).padStart(6, '0')}`,
			menu_path: [],
			initial_params: [],
			select_when_created: true
		}));
		const leaves = buildCreatableItemMenu(descriptors, () => {});
		items = [{ id: 'nested', label: 'Nested catalog', submenu: leaves }, ...leaves];
		open = true;
		await tick();
		menu!.searchItems(0, '');
		await frames();
	}
	export async function reveal(index: number, focus = false, depth = 0) {
		const result = await menu!.revealItem(depth, index, focus);
		await frames();
		return result;
	}
	export async function search(text: string, depth = 0) {
		menu!.searchItems(depth, text);
		await frames();
	}
	export async function nested() {
		await reveal(0, true);
		menu!.openSubmenu(0, 0);
		await frames();
	}
	export async function reverse() {
		items = [...items].reverse();
		await frames();
	}
	export async function setWidth(width: string) {
		maxWidthCss = width;
		await frames();
	}
	export async function measureFilter() {
		await search('');
		return measureCollectionFlow(
			'catalog-filter',
			count,
			(index) =>
				menu!.searchItems(0, index % 2 ? '' : `Catalog ${String(count - 1).padStart(6, '0')}`),
			(index) => {
				const list = document.querySelector<HTMLElement>('.catalog-list');
				const virtual = list?.querySelector<HTMLElement>('.virtual-rows');
				const total = Number(virtual?.dataset.rowCount);
				const label =
					index % 2 ? 'Nested catalog' : `Catalog ${String(count - 1).padStart(6, '0')}`;
				const button = [...(list?.querySelectorAll('button') ?? [])].find((node) =>
					node.textContent?.includes(label)
				);
				if (!list || !button || (index % 2 ? total !== count + 1 : total !== 1)) return false;
				const bounds = list.getBoundingClientRect(),
					rectangle = button.getBoundingClientRect();
				return (
					rectangle.width > 0 && rectangle.bottom > bounds.top && rectangle.top < bounds.bottom
				);
			}
		);
	}
</script>

<ContextMenu
	bind:this={menu}
	bind:open
	{items}
	{maxWidthCss}
	anchor={{ kind: 'point', x: 120, y: 80 }} />
