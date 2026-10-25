<script lang="ts">
	import { tick, type Snippet } from 'svelte';
	import { LARGE_MENU_THRESHOLD, type ContextMenuItem } from '../context-menu';
	import VirtualRows from '../virtual-rows/VirtualRows.svelte';
	interface MenuRow {
		item: ContextMenuItem;
		index: number;
		key: string | number;
	}
	let {
		items,
		item: renderItem
	}: {
		items: readonly ContextMenuItem[];
		item: Snippet<[ContextMenuItem, number]>;
	} = $props();
	const anonymousKeys = new WeakMap<ContextMenuItem, number>();
	let nextKey = 0;
	let query = $state('');
	let scroller = $state<HTMLDivElement | null>(null);
	let virtualRows = $state<ReturnType<typeof VirtualRows<MenuRow>>>();
	const large = $derived(items.length > LARGE_MENU_THRESHOLD);
	const rows = $derived.by(() => {
		const occurrences = new Map<string, number>();
		return items.map((item, index): MenuRow => {
			let key: string | number;
			if (item.id !== undefined) {
				const occurrence = occurrences.get(item.id) ?? 0;
				occurrences.set(item.id, occurrence + 1);
				key = JSON.stringify([item.id, occurrence]);
			} else {
				if (!anonymousKeys.has(item)) anonymousKeys.set(item, nextKey++);
				key = anonymousKeys.get(item)!;
			}
			return { item, index, key };
		});
	});
	const filteredRows = $derived.by(() => {
		const search = query.trim().toLocaleLowerCase();
		return search
			? rows.filter(
					(row) => !row.item.separator && row.item.label?.toLocaleLowerCase().includes(search)
				)
			: rows;
	});
	$effect(() => {
		query;
		void tick().then(() => scroller?.scrollTo({ top: 0 }));
	});
	export function searchItems(text: string): void {
		query = text;
	}
	export async function revealItem(index: number): Promise<boolean> {
		const row = rows[index];
		if (!row) return false;
		if (!large) return true;
		const found = virtualRows?.reveal(row.key) ?? false;
		await tick();
		return found;
	}
</script>

{#if large}
	<div class="catalog-menu">
		<input
			type="search"
			aria-label="Filter menu items"
			placeholder="Filter items…"
			bind:value={query} />
		<div
			class="catalog-list"
			role="group"
			aria-label="Menu items"
			tabindex="-1"
			style:--catalog-row-count={Math.max(1, filteredRows.length)}
			bind:this={scroller}>
			<VirtualRows
				bind:this={virtualRows}
				rows={filteredRows}
				keyOf={(row) => row.key}
				{scroller}
				rowHeightRem={1.95}
				variableHeights={true}>
				{#snippet item(row)}
					{@render renderItem(row.item, row.index)}
				{/snippet}
			</VirtualRows>
			{#if filteredRows.length === 0}
				<div class="empty" role="status">No matching items.</div>
			{/if}
		</div>
	</div>
{:else}
	{#each rows as row (row.key)}
		{@render renderItem(row.item, row.index)}
	{/each}
{/if}

<style>
	.catalog-menu {
		inline-size: min(
			22rem,
			var(--gc-context-collection-width, 22rem),
			calc(var(--gc-context-available-width, 100vw) - 0.6rem)
		);
		min-inline-size: 0;
	}
	input {
		box-sizing: border-box;
		inline-size: 100%;
		margin-block-end: 0.3rem;
	}
	.catalog-list {
		block-size: min(
			calc(var(--catalog-row-count) * 1.95rem),
			32rem,
			calc(var(--gc-context-available-height, 100vh) - 3.5rem)
		);
		min-block-size: 1.95rem;
		overflow: auto;
		overscroll-behavior: contain;
		scrollbar-gutter: stable;
	}
	.empty {
		padding: 0.3rem;
		opacity: 0.7;
	}
</style>
