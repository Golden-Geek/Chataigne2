<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import ContextMenu from '../../../common/ContextMenu.svelte';
	import type { NodeId, UiNodeDto } from '../../../../types';
	let {
		children,
		surfaceId,
		choose
	}: {
		children: readonly UiNodeDto[];
		surfaceId: NodeId;
		choose(id: NodeId): boolean;
	} = $props();
	let open = $state(false);
	let destroyed = false;
	onDestroy(() => {
		destroyed = true;
	});
	let anchor = $state<HTMLButtonElement | null>(null);
	let menu = $state<ReturnType<typeof ContextMenu>>();
	const items = $derived(
		open
			? children.map((child) => ({
					id: child.uuid,
					label: child.meta.label,
					action: () => {
						chooseChild(child.node_id);
					}
				}))
			: []
	);
	const restoreFocus = () => {
		const active = document.activeElement;
		if (!(active instanceof Element) || !active.closest(`.dashboard-child-menu-${surfaceId}`))
			return;
		void tick().then(() => {
			if (!destroyed && anchor?.isConnected && document.activeElement === document.body)
				anchor.focus({ preventScroll: true });
		});
	};
	export function openMenu() {
		open = true;
		void tick().then(() => {
			if (!destroyed && open && anchor?.isConnected)
				document
					.querySelector<HTMLInputElement>(`.dashboard-child-menu-${surfaceId} input`)
					?.focus({ preventScroll: true });
		});
	}
	export function chooseChild(id: NodeId): boolean {
		if (!children.some((child) => child.node_id === id)) return false;
		const applied = choose(id);
		if (applied) {
			restoreFocus();
			open = false;
		}
		return applied;
	}
	export function search(query: string) {
		menu?.searchItems(0, query);
	}
	export async function reveal(id: NodeId) {
		openMenu();
		await tick();
		if (destroyed) return false;
		const index = children.findIndex((child) => child.node_id === id);
		return index < 0 ? false : ((await menu?.revealItem(0, index)) ?? false);
	}
</script>

<div class="dashboard-child-browser-anchor">
	<button
		bind:this={anchor}
		type="button"
		class="dashboard-child-browser"
		data-dashboard-child-browser={surfaceId}
		title="Preview of selected children. Browse all widgets in this container."
		onpointerdown={(event) => event.stopPropagation()}
		onclick={openMenu}>Preview · {children.length.toLocaleString()} widgets · Browse</button>
</div>
<ContextMenu
	bind:this={menu}
	bind:open
	{items}
	windowed
	anchor={{ kind: 'element', element: anchor, placement: 'bottom-end' }}
	preferPointerAnchor={false}
	className={`dashboard-child-menu dashboard-child-menu-${surfaceId}`}
	onClose={restoreFocus} />

<style>
	.dashboard-child-browser-anchor {
		position: sticky;
		inset-block-start: 0;
		inset-inline-start: 0;
		block-size: 0;
		inline-size: 100%;
		z-index: 4;
		pointer-events: none;
	}
	.dashboard-child-browser {
		pointer-events: auto;
		position: absolute;
		inset-block-start: 0;
		inset-inline-end: 0;
		z-index: 3;
		max-inline-size: 100%;
		padding: 0.2em 0.4em;
		font: inherit;
		font-size: 0.75rem;
		color: var(--gc-color-text, #ddd);
		background: var(--gc-color-background, #222);
		border: solid 0.06rem currentColor;
		border-radius: 0.25rem;
		cursor: pointer;
	}
</style>
