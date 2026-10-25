<script lang="ts">
	import { getContext, onDestroy, tick } from 'svelte';
	import { createSurfaceScroll } from './surface-scroll.svelte';
	import { dashboardViewContextKey, type DashboardViewContext } from './view-context.svelte';
	import { createFreeSurfaceViewport } from './free-surface-viewport.svelte';
	import FreeSurfaceOverview from './FreeSurfaceOverview.svelte';
	import SurfaceChildBrowser from './SurfaceChildBrowser.svelte';
	import { createSurfaceRetention } from './surface-retention.svelte';
	import { childPresentationBudgets, dashboardSelectedDetailLimit } from './presentation-budget';
	import type { createDashboardSurface } from './surface.svelte';
	import type { createDashboardMarquee } from './marquee.svelte';
	import type { SurfaceDropPreview, NonFreeSurfaceFlowRenderItem } from './contracts';
	import type { NodeId, UiNodeDto } from '../../../../types';
	import DashboardCanvasSelf from '../DashboardCanvas.svelte';
	import { withStoppedPropagation } from './dom';
	let {
		surface,
		marquee,
		editMode,
		compact = false,
		dropIndicatorLabel = 'Drop to add a child widget',
		renderMarqueeInSurface = false
	} = $props<{
		surface: ReturnType<typeof createDashboardSurface>;
		marquee: Pick<ReturnType<typeof createDashboardMarquee>, 'marqueeSelection' | 'marqueeStyle'>;
		editMode: boolean;
		compact?: boolean;
		dropIndicatorLabel?: string;
		renderMarqueeInSurface?: boolean;
	}>();
	let layoutKind = $derived(surface.layoutKind);
	let childWidgets: UiNodeDto[] = $derived(surface.childWidgets);
	let renderedChildWidgets: UiNodeDto[] = $derived(surface.renderedChildWidgets);
	let activeTabNodeId = $derived(surface.activeTabNodeId);
	let openAccordionNodeIds = $derived(surface.openAccordionNodeIds);
	let surfaceDragDepth = $derived(surface.surfaceDragDepth);
	let surfaceDropPreview = $derived(surface.surfaceDropPreview);
	let pendingSurfaceDrop = $derived(surface.pendingSurfaceDrop);
	let displayedSurfaceDropPreview = $derived(surface.displayedSurfaceDropPreview);
	let nonFreeSurfaceFlowRenderItems: NonFreeSurfaceFlowRenderItem[] = $derived(
		surface.nonFreeSurfaceFlowRenderItems
	);
	const slotStyle = (...args: Parameters<typeof surface.slotStyle>) => surface.slotStyle(...args);
	const inlineSurfaceDropPreviewStyle = (
		...args: Parameters<typeof surface.inlineSurfaceDropPreviewStyle>
	) => surface.inlineSurfaceDropPreviewStyle(...args);
	const isHiddenSourceSurfaceChild = (
		...args: Parameters<typeof surface.isHiddenSourceSurfaceChild>
	) => surface.isHiddenSourceSurfaceChild(...args);
	const shouldRenderSurfaceDropPreviewBeforeChild = (
		...args: Parameters<typeof surface.shouldRenderSurfaceDropPreviewBeforeChild>
	) => surface.shouldRenderSurfaceDropPreviewBeforeChild(...args);
	const shouldRenderSurfaceDropPreviewAtEnd = (
		...args: Parameters<typeof surface.shouldRenderSurfaceDropPreviewAtEnd>
	) => surface.shouldRenderSurfaceDropPreviewAtEnd(...args);
	const toggleAccordionNode = (...args: Parameters<typeof surface.toggleAccordionNode>) =>
		surface.toggleAccordionNode(...args);
	const activateDashboardTab = (...args: Parameters<typeof surface.activateDashboardTab>) =>
		surface.activateDashboardTab(...args);
	let marqueeSelection = $derived(marquee.marqueeSelection);
	let marqueeStyle = $derived(marquee.marqueeStyle);
	const view = getContext<DashboardViewContext | undefined>(dashboardViewContextKey) ?? null;
	let marker = $state<HTMLSpanElement | null>(null);
	let browser = $state<ReturnType<typeof SurfaceChildBrowser>>();
	let chosen = $state<NodeId | null>(null);
	const scrolling = createSurfaceScroll({
		get element() {
			return marker?.parentElement ?? null;
		},
		get node() {
			return surface.node;
		},
		get graph() {
			return surface.graph;
		},
		view
	});
	let ownerIdentity: string | null = null;
	$effect(() => {
		if (surface.node.uuid !== ownerIdentity) {
			ownerIdentity = surface.node.uuid;
			chosen = null;
		}
	});
	const retention = createSurfaceRetention({
		get element() {
			return marker?.parentElement ?? null;
		},
		get identity() {
			return surface.node.uuid;
		},
		get viewport() {
			return view?.viewport ?? null;
		},
		getNode(id) {
			const graph = surface.graph;
			return graph?.parentById.get(id) === surface.node.node_id
				? graph.nodesById.get(id)
				: undefined;
		}
	});
	$effect(() => {
		if (browser) return view?.registerBrowser(surface.node.node_id, browser);
	});
	const collection = $derived(
		layoutKind !== 'free' &&
			(childWidgets.length > surface.budget ||
				((layoutKind === 'tabs' || layoutKind === 'accordion') && childWidgets.length > 128))
	);
	const retained = $derived.by(() => {
		void surface.budget;
		void layoutKind;
		const ids = new Set(retention.read());
		return ids;
	});
	$effect.pre(() => {
		void surface.graph;
		void retained;
	});
	const frameTargets = $derived(new Set(view?.retainedFor(surface.node.node_id) ?? []));
	let destroyed = false;
	onDestroy(() => {
		destroyed = true;
	});
	const selectedChild = $derived.by(() => {
		const selected = view?.selected ?? [],
			graph = surface.graph;
		for (let index = selected.length - 1; index >= 0; index--)
			if (graph?.parentById.get(selected[index]) === surface.node.node_id) return selected[index];
		return undefined;
	});
	$effect(() => {
		const element = marker?.parentElement;
		const requested = frameTargets;
		if (!element || layoutKind === 'free') return;
		const preferred = selectedChild ?? [...requested].at(-1);
		if (preferred === undefined) return;
		void tick().then(() => {
			if (destroyed || marker?.parentElement !== element || frameTargets !== requested) return;
			const target = element.querySelector<HTMLElement>(
				`.dashboard-widget-shell[data-node-id="${preferred}"]`
			);
			if (target) scrolling.reveal(target);
		});
	});
	const kept = $derived.by(() => {
		const ids = new Set(retained);
		if (layoutKind === 'accordion' && displayedSurfaceDropPreview)
			for (const child of childWidgets)
				if (shouldRenderSurfaceDropPreviewBeforeChild(child)) ids.add(child.node_id);
		for (const id of frameTargets) ids.add(id);
		for (const id of view?.selected.slice(-dashboardSelectedDetailLimit) ?? []) ids.add(id);
		if (chosen !== null) ids.add(chosen);
		if (collection && layoutKind === 'accordion' && surface.budget > 0)
			for (const id of openAccordionNodeIds.slice(
				-Math.min(dashboardSelectedDetailLimit, surface.budget)
			))
				ids.add(id);
		return ids;
	});
	const accordionChildren = $derived(
		collection ? childWidgets.filter((child) => kept.has(child.node_id)) : childWidgets
	);
	const flowItems = $derived(
		collection
			? nonFreeSurfaceFlowRenderItems.filter(
					(item) => item.kind === 'preview' || kept.has(item.child.node_id)
				)
			: nonFreeSurfaceFlowRenderItems
	);
	const renderedFlow = $derived(surface.boundedFlowSegments(flowItems, collection));
	const activeTab = $derived(
		childWidgets.find((child) => child.node_id === activeTabNodeId) ?? null
	);
	const shownTab = $derived(
		activeTab && (surface.budget > 0 || kept.has(activeTab.node_id)) ? activeTab : null
	);
	const shownTabs = $derived(
		childWidgets.filter((child) => child === shownTab || frameTargets.has(child.node_id))
	);
	const chooseChild = (id: NodeId) => {
		if (!surface.chooseDashboardChild(id)) return false;
		chosen = view ? null : id;
		return true;
	};
	let freeLayoutElement = $state<HTMLElement | null>(null);
	const freeViewport = createFreeSurfaceViewport({
		get budget() {
			return surface.budget;
		},
		retained: () => retained,
		get graph() {
			return surface.graph;
		},
		get node() {
			return surface.node;
		},
		get enabled() {
			return layoutKind === 'free';
		},
		get element() {
			return freeLayoutElement;
		},
		view
	});
	let mountedFreeWidgets = $derived(freeViewport.mounted);
	const mountedChildren = $derived(
		layoutKind === 'free'
			? mountedFreeWidgets
			: layoutKind === 'tabs'
				? shownTabs
				: layoutKind === 'accordion'
					? accordionChildren.filter((child) => openAccordionNodeIds.includes(child.node_id))
					: flowItems.flatMap((item) => (item.kind === 'child' ? [item.child] : []))
	);
	const childBudgets = $derived(childPresentationBudgets(mountedChildren, surface.budget));
	const childBudget = (child: UiNodeDto) => childBudgets.get(child.node_id) ?? 1;
	const surfaceDropKindLabel = (preview: SurfaceDropPreview): string => {
		if (preview.targetKind === 'node') {
			return 'Node Widget';
		}
		switch (preview.genericWidgetKind) {
			case 'button':
				return 'Button Widget';
			case 'slider':
				return 'Slider Widget';
			case 'textInput':
				return 'Text Input Widget';
			case 'checkbox':
				return 'Checkbox Widget';
			default:
				return 'Text Widget';
		}
	};
</script>

<span hidden bind:this={marker} aria-hidden="true" data-dashboard-budget={surface.budget}></span>
{#if collection}
	<SurfaceChildBrowser
		bind:this={browser}
		children={childWidgets}
		surfaceId={surface.node.node_id}
		choose={chooseChild} />
{/if}

{#snippet dashboardSurfaceDropPreviewContent(preview: SurfaceDropPreview)}
	<div class="dashboard-drop-preview-meta">
		<span class="dashboard-drop-preview-kind">{surfaceDropKindLabel(preview)}</span>
		<strong>{preview.label}</strong>
	</div>
	<div class="dashboard-drop-preview-shell">
		{#if preview.targetKind === 'node'}
			<div class="dashboard-drop-preview-node-toolbar">
				<span></span>
				<span></span>
				<span></span>
			</div>
			<div class="dashboard-drop-preview-node-lines">
				<span class="wide"></span>
				<span></span>
				<span class="short"></span>
			</div>
		{:else if preview.genericWidgetKind === 'button'}
			<div class="dashboard-drop-preview-button">{preview.previewText ?? preview.label}</div>
		{:else if preview.genericWidgetKind === 'slider'}
			<div class="dashboard-drop-preview-slider">
				<span class="dashboard-drop-preview-slider-label">{preview.label}</span>
				<span class="dashboard-drop-preview-slider-track"><span></span></span>
			</div>
		{:else if preview.genericWidgetKind === 'checkbox'}
			<div class="dashboard-drop-preview-checkbox">
				<span class="dashboard-drop-preview-checkbox-box"></span>
				<span>{preview.previewText ?? preview.label}</span>
			</div>
		{:else if preview.genericWidgetKind === 'textInput'}
			<div class="dashboard-drop-preview-input" class:multiline={preview.multiline}>
				<span>{preview.previewPlaceholder ?? preview.label}</span>
			</div>
		{:else}
			<div class="dashboard-drop-preview-text">{preview.previewText ?? preview.label}</div>
		{/if}
	</div>
{/snippet}

{#snippet dashboardSurfaceDropPreviewSlot(style: string = '', additionalClass: string = '')}
	{#if displayedSurfaceDropPreview}
		<div
			class={`dashboard-slot dashboard-drop-preview ${additionalClass}`.trim()}
			class:is-committing={pendingSurfaceDrop !== null && surfaceDropPreview === null}
			{style}
			aria-hidden="true">
			<div class="dashboard-slot-fill">
				<div class="dashboard-drop-preview-fill">
					{@render dashboardSurfaceDropPreviewContent(displayedSurfaceDropPreview)}
				</div>
			</div>
		</div>
	{/if}
{/snippet}
{#if layoutKind === 'tabs'}
	<div class="dashboard-tab-strip" class:compact>
		{#each collection ? childWidgets.filter( (child) => retained.has(child.node_id) ) : childWidgets as child (child.node_id)}
			{#if shouldRenderSurfaceDropPreviewBeforeChild(child)}
				<button
					type="button"
					class="selected dashboard-drop-preview-tab"
					tabindex="-1"
					aria-hidden="true">
					{displayedSurfaceDropPreview?.label}
				</button>
			{/if}
			<button
				type="button"
				data-dashboard-child-id={child.node_id}
				class:dashboard-hidden-drag-source={isHiddenSourceSurfaceChild(child)}
				class:selected={!displayedSurfaceDropPreview && activeTabNodeId === child.node_id}
				onpointerdown={withStoppedPropagation}
				onclick={() => {
					activateDashboardTab(child.node_id);
				}}>{child.meta.label}</button>
		{/each}
		{#if shouldRenderSurfaceDropPreviewAtEnd()}
			<button
				type="button"
				class="selected dashboard-drop-preview-tab"
				tabindex="-1"
				aria-hidden="true">
				{displayedSurfaceDropPreview?.label}
			</button>
		{/if}
	</div>
	<div class="dashboard-tab-body" class:compact style:position="relative">
		{#if displayedSurfaceDropPreview}
			{@render dashboardSurfaceDropPreviewSlot('', 'dashboard-drop-preview-panel')}
		{/if}
		{#each shownTabs as activeChild (activeChild.node_id)}
			<div
				class="dashboard-slot"
				class:dashboard-frame-measurement={activeChild !== shownTab ||
					displayedSurfaceDropPreview !== null}
				inert={activeChild !== shownTab || displayedSurfaceDropPreview !== null}
				aria-hidden={activeChild !== shownTab || displayedSurfaceDropPreview !== null
					? true
					: undefined}>
				<div class="dashboard-slot-fill">
					<DashboardCanvasSelf
						node={activeChild}
						{editMode}
						presentationBudget={childBudget(activeChild)} />
				</div>
			</div>
		{/each}
	</div>
{:else if layoutKind === 'accordion'}
	<div class="dashboard-accordion" class:compact>
		{#each accordionChildren as child (child.node_id)}
			{#if shouldRenderSurfaceDropPreviewBeforeChild(child)}
				<section class="dashboard-accordion-item dashboard-drop-preview-accordion">
					<button
						type="button"
						class="selected dashboard-drop-preview-tab"
						tabindex="-1"
						aria-hidden="true">
						{displayedSurfaceDropPreview?.label}
					</button>
					{@render dashboardSurfaceDropPreviewSlot('', 'dashboard-drop-preview-panel')}
				</section>
			{/if}
			<section
				class="dashboard-accordion-item"
				data-dashboard-child-id={child.node_id}
				class:dashboard-hidden-drag-source={isHiddenSourceSurfaceChild(child)}>
				<button
					type="button"
					class:selected={openAccordionNodeIds.includes(child.node_id)}
					onpointerdown={withStoppedPropagation}
					onclick={() => {
						toggleAccordionNode(child.node_id);
					}}>{child.meta.label}</button>
				{#if openAccordionNodeIds.includes(child.node_id) || frameTargets.has(child.node_id)}
					<div
						class="dashboard-slot accordion-slot"
						class:dashboard-frame-measurement={!openAccordionNodeIds.includes(child.node_id)}>
						<div class="dashboard-slot-fill">
							<DashboardCanvasSelf
								node={child}
								{editMode}
								presentationBudget={childBudget(child)} />
						</div>
					</div>
				{/if}
			</section>
		{/each}
		{#if shouldRenderSurfaceDropPreviewAtEnd()}
			<section class="dashboard-accordion-item dashboard-drop-preview-accordion">
				<button
					type="button"
					class="selected dashboard-drop-preview-tab"
					tabindex="-1"
					aria-hidden="true">
					{displayedSurfaceDropPreview?.label}
				</button>
				{@render dashboardSurfaceDropPreviewSlot('', 'dashboard-drop-preview-panel')}
			</section>
		{/if}
	</div>
{:else if layoutKind === 'free'}
	<div
		class="dashboard-layout"
		bind:this={freeLayoutElement}
		data-dashboard-free-total={childWidgets.length}
		data-dashboard-free-mounted={mountedFreeWidgets.length}
		data-dashboard-free-presentation={freeViewport.overview ? 'overview' : 'detail'}>
		{#if freeViewport.overview}
			<FreeSurfaceOverview
				overview={freeViewport.overview}
				activate={(x, y) => view?.activateAt(surface.node.node_id, x, y)} />
		{/if}
		{#each mountedFreeWidgets as child (child.node_id)}
			{#if shouldRenderSurfaceDropPreviewBeforeChild(child)}
				{@render dashboardSurfaceDropPreviewSlot(
					inlineSurfaceDropPreviewStyle(),
					layoutKind === 'free' ? '' : 'dashboard-drop-preview-flow'
				)}
			{/if}
			<div
				class="dashboard-slot"
				class:dashboard-hidden-drag-source={isHiddenSourceSurfaceChild(child)}
				style={slotStyle(child)}>
				<div class="dashboard-slot-fill">
					<DashboardCanvasSelf node={child} {editMode} presentationBudget={childBudget(child)} />
				</div>
			</div>
		{/each}
		{#if shouldRenderSurfaceDropPreviewAtEnd()}
			{@render dashboardSurfaceDropPreviewSlot(
				inlineSurfaceDropPreviewStyle(),
				layoutKind === 'free' ? '' : 'dashboard-drop-preview-flow'
			)}
		{/if}
	</div>
{:else}
	<div class="dashboard-layout" data-dashboard-collection-preview={collection ? '' : undefined}>
		{#each renderedFlow as segment (segment.key)}
			{#if segment.kind === 'spacer'}
				<div class="dashboard-flow-spacer" style={segment.style} aria-hidden="true"></div>
			{:else}
				{@const item = segment.item}
				<div
					class="dashboard-slot"
					class:dashboard-drop-preview={item.kind === 'preview'}
					class:dashboard-drop-preview-flow={item.kind === 'preview'}
					class:is-committing={item.kind === 'preview' &&
						pendingSurfaceDrop !== null &&
						surfaceDropPreview === null}
					class:dashboard-hidden-drag-source={item.kind === 'child' &&
						isHiddenSourceSurfaceChild(item.child)}
					style={`${item.kind === 'preview' ? inlineSurfaceDropPreviewStyle() : slotStyle(item.child)} ${collection ? surface.boundedGridStyle(item) : ''}`}
					aria-hidden={item.kind === 'preview'}>
					<div class="dashboard-slot-fill">
						{#if item.kind === 'preview'}
							<div class="dashboard-drop-preview-fill">
								{@render dashboardSurfaceDropPreviewContent(item.preview)}
							</div>
						{:else}
							<DashboardCanvasSelf
								node={item.child}
								{editMode}
								presentationBudget={childBudget(item.child)} />
						{/if}
					</div>
				</div>
			{/if}
		{/each}
	</div>
{/if}

{#if renderedChildWidgets.length === 0 && !displayedSurfaceDropPreview}
	<div class="dashboard-empty-inline dashboard-surface-empty">
		Drop widgets here or use the add menu.
	</div>
{/if}

{#if surfaceDragDepth > 0 && !displayedSurfaceDropPreview}
	<div class="dashboard-drop-indicator">{dropIndicatorLabel}</div>
{/if}

{#if renderMarqueeInSurface && marqueeSelection}
	<div class="dashboard-marquee-selection" style={marqueeStyle} aria-hidden="true"></div>
{/if}

<style>
	.dashboard-frame-measurement {
		position: absolute;
		inset: 0;
		visibility: hidden;
		pointer-events: none;
	}
</style>
