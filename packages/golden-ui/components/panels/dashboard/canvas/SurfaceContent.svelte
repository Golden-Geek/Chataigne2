<script lang="ts">
	import { getContext } from 'svelte';
	import { dashboardViewContextKey, type DashboardViewContext } from './view-context.svelte';
	import { createFreeSurfaceViewport } from './free-surface-viewport.svelte';
	import FreeSurfaceOverview from './FreeSurfaceOverview.svelte';
	import type { createDashboardSurface } from './surface.svelte';
	import type { createDashboardMarquee } from './marquee.svelte';
	import type { SurfaceDropPreview } from './contracts';
	import type { UiNodeDto } from '../../../../types';
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
	let nonFreeSurfaceFlowRenderItems = $derived(surface.nonFreeSurfaceFlowRenderItems);
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
	let freeLayoutElement = $state<HTMLElement | null>(null);
	const freeViewport = createFreeSurfaceViewport({
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
		{#each childWidgets as child (child.node_id)}
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
	<div class="dashboard-tab-body" class:compact>
		{#if displayedSurfaceDropPreview}
			{@render dashboardSurfaceDropPreviewSlot('', 'dashboard-drop-preview-panel')}
		{:else if activeTabNodeId !== null}
			{@const activeChild =
				childWidgets.find((candidate) => candidate.node_id === activeTabNodeId) ?? null}
			{#if activeChild}
				<div class="dashboard-slot">
					<div class="dashboard-slot-fill">
						<DashboardCanvasSelf node={activeChild} {editMode} />
					</div>
				</div>
			{/if}
		{/if}
	</div>
{:else if layoutKind === 'accordion'}
	<div class="dashboard-accordion" class:compact>
		{#each childWidgets as child (child.node_id)}
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
				class:dashboard-hidden-drag-source={isHiddenSourceSurfaceChild(child)}>
				<button
					type="button"
					class:selected={openAccordionNodeIds.includes(child.node_id)}
					onpointerdown={withStoppedPropagation}
					onclick={() => {
						toggleAccordionNode(child.node_id);
					}}>{child.meta.label}</button>
				{#if openAccordionNodeIds.includes(child.node_id)}
					<div class="dashboard-slot accordion-slot">
						<div class="dashboard-slot-fill">
							<DashboardCanvasSelf node={child} {editMode} />
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
					<DashboardCanvasSelf node={child} {editMode} />
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
	<div class="dashboard-layout">
		{#each nonFreeSurfaceFlowRenderItems as item (item.key)}
			<div
				class="dashboard-slot"
				class:dashboard-drop-preview={item.kind === 'preview'}
				class:dashboard-drop-preview-flow={item.kind === 'preview'}
				class:is-committing={item.kind === 'preview' &&
					pendingSurfaceDrop !== null &&
					surfaceDropPreview === null}
				class:dashboard-hidden-drag-source={item.kind === 'child' &&
					isHiddenSourceSurfaceChild(item.child)}
				style={item.kind === 'preview' ? inlineSurfaceDropPreviewStyle() : slotStyle(item.child)}
				aria-hidden={item.kind === 'preview'}>
				<div class="dashboard-slot-fill">
					{#if item.kind === 'preview'}
						<div class="dashboard-drop-preview-fill">
							{@render dashboardSurfaceDropPreviewContent(item.preview)}
						</div>
					{:else}
						<DashboardCanvasSelf node={item.child} {editMode} />
					{/if}
				</div>
			</div>
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
