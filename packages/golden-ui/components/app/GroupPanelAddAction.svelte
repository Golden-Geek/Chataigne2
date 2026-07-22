<script lang="ts">
	import type { PanelDefinition } from '../../dockview/panel-types';
	import ContextMenu from '../common/ContextMenu.svelte';
	import type { ContextMenuAnchor, ContextMenuItem } from '../common/context-menu';
	import addTabIcon from '../../style/icons/panels/add_tab.svg';

	let {
		availablePanels = [],
		onAddPanel
	}: {
		availablePanels: ReadonlyArray<PanelDefinition>;
		onAddPanel: (panelType: string) => void;
	} = $props();

	let menuOpen = $state(false);
	let triggerElement: HTMLButtonElement | null = $state(null);

	const canAddPanels = $derived(availablePanels.length > 0);

	let menuAnchor = $derived.by((): ContextMenuAnchor | null => {
		if (!triggerElement) {
			return null;
		}
		return {
			kind: 'element',
			element: triggerElement,
			placement: 'bottom-start',
			offsetRem: 0.08
		};
	});

	const toMenuItem = (panel: PanelDefinition): ContextMenuItem => ({
		id: panel.panelType,
		label: panel.title,
		hint: panel.description,
		action: () => {
			onAddPanel(panel.panelType);
		}
	});

	let menuItems = $derived.by((): ContextMenuItem[] => {
		// Uncategorized panels stay at the top level; categorized panels collapse into a
		// submenu per category so the list stays tidy as module editors are added.
		const topLevel: ContextMenuItem[] = [];
		const byCategory = new Map<string, ContextMenuItem[]>();

		for (const panel of availablePanels) {
			const category = panel.category?.trim();
			if (category) {
				const bucket = byCategory.get(category) ?? [];
				bucket.push(toMenuItem(panel));
				byCategory.set(category, bucket);
			} else {
				topLevel.push(toMenuItem(panel));
			}
		}

		const categories = [...byCategory.keys()].sort((a, b) => a.localeCompare(b));
		const submenus: ContextMenuItem[] = categories.map((category) => ({
			id: `category:${category}`,
			label: category,
			submenu: byCategory.get(category) ?? []
		}));

		return [...topLevel, ...submenus];
	});

	export const closeMenu = (): void => {
		menuOpen = false;
	};
</script>

{#snippet panelMenuItem(item: ContextMenuItem)}
	<span class="gc-group-panel-add-menu-content">
		<span class="gc-group-panel-add-menu-copy">
			<span class="gc-group-panel-add-menu-title">{item.label}</span>
			{#if item.hint}
				<span class="gc-group-panel-add-menu-subtitle">{item.hint}</span>
			{/if}
		</span>
		{#if item.submenu}
			<span class="gc-group-panel-add-menu-chevron" aria-hidden="true">></span>
		{/if}
	</span>
{/snippet}

<div class="gc-group-panel-add">
	<button
		bind:this={triggerElement}
		type="button"
		class="gc-group-panel-add-trigger"
		aria-label="Add panel in this group"
		title="Add panel"
		disabled={!canAddPanels}
		onclick={() => {
			if (!canAddPanels) {
				return;
			}
			menuOpen = !menuOpen;
		}}>
		<img src={addTabIcon} alt="" />
	</button>

	<ContextMenu
		bind:open={menuOpen}
		items={menuItems}
		anchor={menuAnchor}
		insideElements={[triggerElement]}
		menuClassName="gc-group-panel-add-menu"
		itemClassName="gc-group-panel-add-menu-button"
		itemContent={panelMenuItem}
		minWidthRem={9}
		maxWidthCss="max-content"
		zIndex={20} />
</div>

<style>
	.gc-group-panel-add {
		position: relative;
		display: flex;
		align-items: center;
		block-size: 100%;
	}

	.gc-group-panel-add-trigger {
		inline-size: 1.4rem;
		block-size: 1.4rem;
		padding: 0.16rem;
		border-radius: 0.35rem;
		cursor: pointer;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		opacity: 0.9;
		transition: opacity 0.14s ease;
	}

	.gc-group-panel-add-trigger:hover:not(:disabled) {
		opacity: 1;
	}

	.gc-group-panel-add-trigger:disabled {
		opacity: 0.45;
		cursor: default;
	}

	.gc-group-panel-add-trigger img {
		inline-size: 100%;
		block-size: 100%;
		display: block;
	}

	:global(.gc-context-item.gc-group-panel-add-menu-button) {
		display: block;
		padding: 0.38rem 0.5rem;
	}

	.gc-group-panel-add-menu-content {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		inline-size: 100%;
		min-inline-size: max-content;
	}

	.gc-group-panel-add-menu-copy {
		display: block;
		min-inline-size: 0;
	}

	.gc-group-panel-add-menu-title {
		display: block;
		line-height: 1.18;
		white-space: nowrap;
		overflow-wrap: normal;
	}

	.gc-group-panel-add-menu-subtitle {
		display: block;
		max-inline-size: 13rem;
		margin-block-start: 0.16rem;
		opacity: 0.66;
		font-size: 0.72em;
		line-height: 1.24;
		white-space: normal;
		overflow-wrap: break-word;
	}

	.gc-group-panel-add-menu-chevron {
		flex: 0 0 auto;
		align-self: center;
		opacity: 0.68;
		font-size: 0.74em;
	}
</style>
