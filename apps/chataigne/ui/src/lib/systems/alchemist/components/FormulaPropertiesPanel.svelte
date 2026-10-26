<script lang="ts">
	import {
		ManagerListPanel,
		NodeInspector,
		canDragOutlinerNode,
		type UiNodeDto,
		type UiCreatableUserItem
	} from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import {
		PROPERTY_NODE_TYPE,
		PROPERTY_MANAGER_NODE_TYPE,
		PROPERTY_FOLDER_NODE_TYPE
	} from '../alchemistGraph';
	import { PROPERTY_DRAG_TYPE } from '../editor/formulaItems.svelte';
	let {
		processorNode,
		formula,
		properties,
		activePropertyContainer,
		formulaReadOnly,
		createPropertyItem,
		visible = $bindable(true),
		onInsetChange
	}: {
		processorNode: UiNodeDto | null;
		formula: UiNodeDto | null;
		properties: UiNodeDto | null;
		activePropertyContainer: UiNodeDto | null;
		formulaReadOnly: boolean;
		createPropertyItem: (parent: UiNodeDto, item: UiCreatableUserItem) => void;
		visible?: boolean;
		onInsetChange: (inset: number) => void;
	} = $props();
	const MIN_PANEL_WIDTH = 10,
		MAX_PANEL_WIDTH = 32.5;
	let width = $state(15),
		measured = $state(0);
	let root = $state<HTMLElement | null>(null);
	let graphState = $derived(appState.session?.graph.state ?? null);
	$effect(() => {
		if (!root) return;
		const observer = new ResizeObserver(() => {
			measured = root?.getBoundingClientRect().width ?? 0;
		});
		observer.observe(root);
		return () => observer.disconnect();
	});
	$effect(() => onInsetChange(visible ? measured : 0));
	const isPropertyTreeNode = (node: UiNodeDto | null | undefined): node is UiNodeDto =>
		Boolean(
			node &&
			(node.node_type === PROPERTY_NODE_TYPE ||
				node.node_type === PROPERTY_MANAGER_NODE_TYPE ||
				node.node_type === PROPERTY_FOLDER_NODE_TYPE)
		);

	const canRenderPropertyChildren = (node: UiNodeDto): boolean =>
		node.node_type === PROPERTY_MANAGER_NODE_TYPE || node.node_type === PROPERTY_FOLDER_NODE_TYPE;

	const canMovePropertyNode = (node: UiNodeDto): boolean =>
		isPropertyTreeNode(node) && canDragOutlinerNode(graphState, node);

	const setPropertyGraphDragData = (node: UiNodeDto, event: DragEvent): void => {
		if (!event.dataTransfer) return;
		event.dataTransfer.effectAllowed = 'copyMove';
		if (node.node_type === PROPERTY_NODE_TYPE || node.node_type === PROPERTY_MANAGER_NODE_TYPE) {
			event.dataTransfer.setData(PROPERTY_DRAG_TYPE, String(node.node_id));
		}
	};
</script>

<!-- Slide-in properties panel -->
<aside
	bind:this={root}
	class="properties-panel"
	class:visible
	style:width="{width}em"
	aria-label="Formula properties">
	<div class="properties-heading">
		<button
			type="button"
			class="properties-toggle-btn"
			aria-pressed={visible}
			title="Hide properties"
			onclick={() => (visible = false)}>
			<span class="properties-toggle-chevron">‹</span>
			<span class="properties-toggle-label">{processorNode ? 'Processor' : 'Properties'}</span>
		</button>
	</div>
	<div class="properties-body">
		{#if processorNode}
			<div class="processor-node-inspector" aria-label="Processor node inspector">
				<NodeInspector nodes={[processorNode]} level={0} />
			</div>
		{/if}
		{#if formula}
			<ManagerListPanel
				managerNode={properties}
				addTargetNode={formulaReadOnly ? null : activePropertyContainer}
				addItems={formulaReadOnly ? [] : activePropertyContainer?.creatable_user_items}
				searchPlaceholder="Search properties..."
				missingMessage="Formula properties are not available."
				emptyMessage={formulaReadOnly
					? 'Built-in formula properties are read-only.'
					: 'Drag a property onto the graph to create a getter.'}
				rootDropMessage={formulaReadOnly
					? 'Built-in formula properties are read-only.'
					: 'Drop here to move into Properties.'}
				addButtonTitle="Add property item"
				isTreeNode={isPropertyTreeNode}
				canRenderNodeChildren={canRenderPropertyChildren}
				nodeDraggable={formulaReadOnly ? () => false : canMovePropertyNode}
				onNodeDragStartData={formulaReadOnly ? undefined : setPropertyGraphDragData}
				onCreateItem={formulaReadOnly
					? undefined
					: (parent, item) => createPropertyItem(parent, item)} />
		{/if}
	</div>
	<!-- Resize handle on right edge -->
	<div
		class="panel-resize-handle"
		role="separator"
		aria-label="Resize properties panel"
		aria-orientation="vertical"
		onpointerdown={(e) => {
			e.preventDefault();
			const startX = e.clientX;
			const startWidth = width;
			const em = parseFloat(getComputedStyle(root!).fontSize);
			const el = e.currentTarget as HTMLElement;
			el.setPointerCapture(e.pointerId);
			el.onpointermove = (ev) => {
				width = Math.max(
					MIN_PANEL_WIDTH,
					Math.min(MAX_PANEL_WIDTH, startWidth + (ev.clientX - startX) / em)
				);
			};
			el.onpointerup = el.onpointercancel = (ev) => {
				el.releasePointerCapture(ev.pointerId);
				el.onpointermove = null;
				el.onpointerup = null;
				el.onpointercancel = null;
			};
		}}>
	</div>
</aside>

<!-- Collapsed-state tab — visible only when panel is hidden -->
<button
	type="button"
	class="properties-show-tab"
	class:panel-visible={visible}
	aria-hidden={visible}
	title="Show properties"
	onclick={() => (visible = true)}>
	<span class="properties-toggle-chevron">›</span>
	<span class="properties-toggle-label">Properties</span>
</button>

<style>
	/* Properties panel — glass overlay sliding in from the left */
	.properties-panel {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		z-index: 30;
		display: grid;
		grid-template-rows: auto minmax(0, 1fr) auto;
		min-inline-size: 0;
		min-block-size: 0;
		border-inline-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		background: color-mix(in srgb, var(--gc-color-background-soft, #1a1a1a) 72%, transparent);
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
		box-shadow: 0.5rem 0 2rem color-mix(in srgb, black 32%, transparent);
		transform: translateX(-100%);
		transition: transform 0.22s cubic-bezier(0.2, 0, 0.13, 1);
		pointer-events: none;
	}

	.properties-panel.visible {
		transform: translateX(0);
		pointer-events: auto;
	}

	/* Toggle button inside the panel heading */
	.properties-toggle-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.18rem 0.3rem 0.18rem 0.2rem;
		border: none;
		border-radius: 0.3rem;
		background: transparent;
		color: var(--gc-color-text);
		font: inherit;
		cursor: pointer;
		transition: background 0.12s;
	}

	.properties-toggle-btn:hover {
		background: color-mix(in srgb, var(--gc-color-accent, #66a6ff) 14%, transparent);
	}

	.properties-toggle-chevron {
		font-size: 0.9rem;
		line-height: 1;
		color: color-mix(in srgb, var(--gc-color-text) 65%, transparent);
	}

	.properties-toggle-label {
		font-size: 0.74rem;
		font-weight: 600;
	}

	/* Collapsed-state tab — shown when panel is hidden */
	.properties-show-tab {
		position: absolute;
		top: 0;
		left: 0;
		z-index: 35;
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.35rem 0.55rem 0.35rem 0.4rem;
		border: none;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		border-inline-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		border-end-end-radius: 0.4rem;
		background: color-mix(in srgb, var(--gc-color-background-soft, #1a1a1a) 72%, transparent);
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
		color: var(--gc-color-text);
		font: inherit;
		cursor: pointer;
		transition:
			opacity 0.18s,
			background 0.12s;
	}

	.properties-show-tab.panel-visible {
		opacity: 0;
		pointer-events: none;
	}

	.properties-show-tab:not(.panel-visible):hover {
		background: color-mix(
			in srgb,
			var(--gc-color-accent, #66a6ff) 18%,
			var(--gc-color-background-soft, #1a1a1a)
		);
	}

	/* Resize handle on the right edge of the panel */
	.panel-resize-handle {
		position: absolute;
		inset-block: 0;
		inset-inline-end: 0;
		inline-size: 0.35rem;
		cursor: col-resize;
		z-index: 5;
		touch-action: none;
	}

	.panel-resize-handle:hover,
	.panel-resize-handle:active {
		background: color-mix(in srgb, var(--gc-color-accent, #66a6ff) 55%, transparent);
	}

	.properties-heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.28rem 0.5rem 0.28rem 0.35rem;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
	}

	.properties-body {
		display: grid;
		grid-template-rows: auto minmax(0, 1fr);
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.processor-node-inspector {
		max-block-size: 42vh;
		padding: 0.45rem;
		overflow: auto;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		background: color-mix(in srgb, var(--gc-color-background) 72%, transparent);
	}
</style>
