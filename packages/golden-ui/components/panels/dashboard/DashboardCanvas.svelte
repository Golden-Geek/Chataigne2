<script lang="ts">
	import DashboardSurfaceContent from './canvas/SurfaceContent.svelte';
	import { createDashboardBindingDrop } from './canvas/binding-drop.svelte';
	import { createDashboardMarquee } from './canvas/marquee.svelte';
	import { createDashboardSurface } from './canvas/surface.svelte';
	import { createDashboardOrderedLayout } from './canvas/ordered-layout.svelte';
	import { createDashboardFreeLayout } from './canvas/free-layout.svelte';
	import { createDashboardInteractionGeometry } from './canvas/interaction-geometry.svelte';
	import { createDashboardInteractionChrome } from './canvas/interaction-chrome.svelte';
	import './canvas/styles/surface-layout.css';
	import './canvas/styles/widget-frame.css';
	import './canvas/styles/content-preview.css';
	import { createDashboardContextMenu } from './canvas/context-menu.svelte';
	import { createDashboardCamera } from './canvas/camera.svelte';
	import { createDashboardWidgetContent } from './canvas/widget-content.svelte';
	import type {
		DashboardPersistedPageView,
		DashboardPersistedPageViewChangeHandler,
		FreeLayoutResizeZone,
		FreeLayoutRect
	} from './canvas/contracts';
	import { freeLayoutResizeZones } from './canvas/geometry';
	import { selectionModeFromEvent, focusClosestPageViewport } from './canvas/dom';
	import { onDestroy, getContext, setContext, untrack } from 'svelte';
	import {
		createDashboardViewContext,
		dashboardViewContextKey,
		type DashboardViewContext
	} from './canvas/view-context.svelte';
	import { appState, type SelectionMode } from '../../../store/workbench.svelte';
	import type { UiNodeDto } from '../../../types';
	import { formatCssValue } from '../../../css-value';
	import ContextMenu from '../../common/ContextMenu.svelte';
	import Slider from '../../common/Slider.svelte';
	import {
		dashboardPageDetailBudget,
		normalizePresentationBudget
	} from './canvas/presentation-budget';

	import { getDashboardWidgetOrderingState } from './dashboard-actions';

	import {
		type DashboardLabelPlacement,
		formatParamValue,
		getDashboardLayoutKind,
		getDashboardPlacement,
		getDashboardSnapGrid,
		getLiveNode
	} from './dashboard-model';

	let {
		node,
		editMode = false,
		pageNavigationEnabled = true,
		persistedPageView = null,
		presentationBudget = dashboardPageDetailBudget,
		onPersistedPageViewChange = null
	} = $props<{
		node: UiNodeDto;
		editMode?: boolean;
		pageNavigationEnabled?: boolean;
		persistedPageView?: DashboardPersistedPageView | null;
		presentationBudget?: number;
		onPersistedPageViewChange?: DashboardPersistedPageViewChangeHandler | null;
	}>();

	let session = $derived(appState.session);
	const inheritedView =
		getContext<DashboardViewContext | undefined>(dashboardViewContextKey) ?? null;
	// Recursive instances retain their page/widget role while the authored node changes.
	const ownsView = untrack(() => node.node_type === 'dashboard_page');
	const dashboardView = ownsView
		? createDashboardViewContext({
				get viewport() {
					return pageViewportElement;
				},
				get frame() {
					return pageFrameElement;
				},
				get frameSize() {
					return camera.logicalSize;
				},
				get graph() {
					return graph;
				},
				get page() {
					return liveNode;
				},
				get camera() {
					return camera.viewSignal;
				},
				get ready() {
					return camera.viewportReady;
				},
				get selected() {
					return session?.selectedNodesIds ?? [];
				},
				select(id) {
					session?.selectNodes([id], 'REPLACE');
				}
			})
		: inheritedView;
	if (ownsView && dashboardView) setContext(dashboardViewContextKey, dashboardView);
	let graph = $derived(session?.graph.state ?? null);
	let liveNode = $derived(getLiveNode(graph, node));
	let isSelected = $derived(session?.isNodeSelected(liveNode.node_id) ?? false);
	let isPage = $derived(liveNode.node_type === 'dashboard_page');
	let isContainerWidget = $derived(liveNode.node_type === 'dashboard_widget_container');
	let isNodeWidget = $derived(liveNode.node_type === 'dashboard_node_widget');
	let isGenericWidget = $derived(liveNode.node_type === 'dashboard_generic_widget');

	let placement = $derived(getDashboardPlacement(graph, liveNode));
	let widgetLabelText = $derived(liveNode.meta.label.trim());
	let parentNode = $derived.by(() => {
		const parentId = graph?.parentById.get(liveNode.node_id);
		if (parentId === undefined || !graph) {
			return null;
		}
		return graph.nodesById.get(parentId) ?? null;
	});
	let parentLayoutKind = $derived(getDashboardLayoutKind(graph, parentNode, 'free'));
	const surfaceState = createDashboardSurface({
		get budget() {
			return Math.max(0, normalizePresentationBudget(presentationBudget) - (isPage ? 0 : 1));
		},
		view: dashboardView,
		get graph() {
			return graph;
		},
		get node() {
			return liveNode;
		},
		get editMode() {
			return editMode;
		},
		get chrome() {
			return interactionChrome;
		},
		get geometry() {
			return interactionGeometry;
		}
	});
	let layoutKind = $derived(surfaceState.layoutKind);
	let childWidgets = $derived(surfaceState.childWidgets);
	let surfaceDragDepth = $derived(surfaceState.surfaceDragDepth);
	let surfaceSnapGrid = $derived(surfaceState.surfaceSnapGrid);
	let snapGridVisible = $derived(surfaceState.snapGridVisible);
	let surfaceStyle = $derived(surfaceState.surfaceStyle);
	const handleSurfaceDragEnter = surfaceState.handleSurfaceDragEnter;
	const handleSurfaceDragOver = surfaceState.handleSurfaceDragOver;
	const handleSurfaceDragLeave = surfaceState.handleSurfaceDragLeave;
	const handleSurfaceDrop = surfaceState.handleSurfaceDrop;
	let widgetOrderingState = $derived(getDashboardWidgetOrderingState(graph, liveNode.node_id));
	const contextMenu = createDashboardContextMenu({
		get graph() {
			return graph;
		},
		get session() {
			return session;
		},
		get node() {
			return liveNode;
		},
		get parent() {
			return parentNode;
		},
		get editMode() {
			return editMode;
		},
		get layoutKind() {
			return layoutKind;
		},
		get snapGrid() {
			return surfaceSnapGrid;
		},
		get ordering() {
			return widgetOrderingState;
		},
		selectWidgetNode: (mode) => selectWidgetNode(mode)
	});
	let dashboardContextMenu = $derived(contextMenu.dashboardContextMenu);
	let dashboardContextMenuItems = $derived(contextMenu.dashboardContextMenuItems);
	let dashboardContextMenuAnchor = $derived(contextMenu.dashboardContextMenuAnchor);
	const closeDashboardContextMenu = contextMenu.closeDashboardContextMenu;
	const handleSurfaceContextMenu = contextMenu.handleSurfaceContextMenu;
	const handleWidgetContextMenu = contextMenu.handleWidgetContextMenu;

	const bindingDrop = createDashboardBindingDrop({
		get graph() {
			return graph;
		},
		get node() {
			return liveNode;
		},
		get editMode() {
			return editMode;
		}
	});
	let bindingDragDepth = $derived(bindingDrop.bindingDragDepth);
	const canBindNodeWidget = bindingDrop.canBindNodeWidget;
	const canBindGenericWidget = bindingDrop.canBindGenericWidget;
	const handleBindingDragEnter = bindingDrop.handleBindingDragEnter;
	const handleBindingDragOver = bindingDrop.handleBindingDragOver;
	const handleBindingDragLeave = bindingDrop.handleBindingDragLeave;
	const handleNodeWidgetBindingDrop = bindingDrop.handleNodeWidgetBindingDrop;
	const handleGenericWidgetBindingDrop = bindingDrop.handleGenericWidgetBindingDrop;
	let isPageViewportFocused = $state(false);
	let pageViewportElement = $state<HTMLDivElement | null>(null);
	let pageFrameElement = $state<HTMLDivElement | null>(null);
	let widgetShellElement = $state<HTMLElement | null>(null);
	const camera = createDashboardCamera({
		get graph() {
			return graph;
		},
		get node() {
			return liveNode;
		},
		get editMode() {
			return editMode;
		},
		get navigationEnabled() {
			return pageNavigationEnabled;
		},
		get persistedView() {
			return persistedPageView;
		},
		get onPersistedViewChange() {
			return onPersistedPageViewChange;
		},
		get viewport() {
			return pageViewportElement;
		},
		get frame() {
			return pageFrameElement;
		},
		get focused() {
			return isPageViewportFocused;
		},
		get selectedNodeIds() {
			return session?.selectedNodesIds ?? [];
		},
		frameRects: (ids) => dashboardView?.frameRects(ids) ?? Promise.resolve(null)
	});
	let pageSize = $derived(camera.pageSize);
	let pageSceneStyle = $derived(camera.pageSceneStyle);
	let pageFrameStyle = $derived(camera.pageFrameStyle);
	let pageBleedFrameStyle = $derived(camera.pageBleedFrameStyle);
	const handlePageViewportPointerDown = camera.handlePageViewportPointerDown;
	const handlePageViewportWheel = camera.handlePageViewportWheel;

	const interactionChrome = createDashboardInteractionChrome({
		get node() {
			return liveNode;
		},
		get parent() {
			return parentNode;
		},
		get content() {
			return widgetContent;
		}
	});
	const clearSurfaceInsertionIndicator = interactionChrome.clearSurfaceInsertionIndicator;
	const setFreeLayoutHoverSurface = interactionChrome.setFreeLayoutHoverSurface;
	const setFreeLayoutAncestorShells = interactionChrome.setFreeLayoutAncestorShells;
	const publishWidgetMoveSurfacePreview = interactionChrome.publishWidgetMoveSurfacePreview;
	const interactionGeometry = createDashboardInteractionGeometry({
		get graph() {
			return graph;
		},
		get node() {
			return liveNode;
		},
		get parent() {
			return parentNode;
		},
		get placement() {
			return placement;
		},
		get session() {
			return session;
		},
		get editMode() {
			return editMode;
		},
		get isSelected() {
			return isSelected;
		},
		get freeInteraction() {
			return freeLayoutInteraction;
		},
		get orderedInteraction() {
			return widgetMoveInteraction;
		}
	});
	const resolveTransformNodes = interactionGeometry.resolveTransformNodes;
	const freeLayoutState = createDashboardFreeLayout({
		get graph() {
			return graph;
		},
		get node() {
			return liveNode;
		},
		get parent() {
			return parentNode;
		},
		get placement() {
			return placement;
		},
		get editMode() {
			return editMode;
		},
		get isSelected() {
			return isSelected;
		},
		get parentLayoutKind() {
			return parentLayoutKind;
		},
		get snapGrid() {
			return parentSnapGrid;
		},
		get widgetElement() {
			return widgetShellElement;
		},
		chrome: interactionChrome,
		geometry: interactionGeometry,
		clearChrome: () => clearFreeLayoutInteractionChrome(),
		selectWidgetNode: (mode) => selectWidgetNode(mode)
	});
	let freeLayoutInteraction = $derived(freeLayoutState.freeLayoutInteraction);
	let supportsFreePlacement = $derived(freeLayoutState.supportsFreePlacement);
	let widgetShellStyle = $derived(freeLayoutState.widgetShellStyle);
	let anchorGuides = $derived(freeLayoutState.anchorGuides);
	const beginFreeLayoutMove = freeLayoutState.beginFreeLayoutMove;
	const beginFreeLayoutResize = freeLayoutState.beginFreeLayoutResize;
	const resetFreeDropTarget = freeLayoutState.resetFreeDropTarget;
	const orderedLayoutState = createDashboardOrderedLayout({
		get graph() {
			return graph;
		},
		get node() {
			return liveNode;
		},
		get parent() {
			return parentNode;
		},
		get placement() {
			return placement;
		},
		get editMode() {
			return editMode;
		},
		get parentLayoutKind() {
			return parentLayoutKind;
		},
		get ordering() {
			return widgetOrderingState;
		},
		chrome: interactionChrome,
		geometry: interactionGeometry,
		clearChrome: () => clearFreeLayoutInteractionChrome()
	});
	let widgetMoveInteraction = $derived(orderedLayoutState.widgetMoveInteraction);
	let constrainedResizeInteraction = $derived(orderedLayoutState.constrainedResizeInteraction);
	let supportsOrdering = $derived(orderedLayoutState.supportsOrdering);
	const beginWidgetMoveInteraction = orderedLayoutState.beginWidgetMoveInteraction;
	const beginConstrainedResize = orderedLayoutState.beginConstrainedResize;
	const resetOrderedDropTarget = orderedLayoutState.resetOrderedDropTarget;
	const marqueeState = createDashboardMarquee({
		get node() {
			return liveNode;
		},
		get session() {
			return session;
		},
		get editMode() {
			return editMode;
		},
		get children() {
			return childWidgets;
		},
		get viewport() {
			return pageViewportElement;
		},
		get transforming() {
			return freeLayoutInteraction !== null;
		},
		queryRect: (surfaceId, rect) => dashboardView?.query(surfaceId, rect) ?? null
	});
	let marqueeSelection = $derived(marqueeState.marqueeSelection);
	let marqueeStyle = $derived(marqueeState.marqueeStyle);
	const handleSurfacePointerDown = marqueeState.handleSurfacePointerDown;
	const handlePageScenePointerDown = marqueeState.handlePageScenePointerDown;
	export function selectWidgetsInRect(rect: FreeLayoutRect, mode: SelectionMode = 'REPLACE') {
		const root = pageFrameElement ?? widgetShellElement;
		const element = root?.querySelector<HTMLElement>(
			`[data-dashboard-surface-node-id="${liveNode.node_id}"]`
		);
		if (element) marqueeState.selectInRect(element, rect, mode);
		return session?.selectedNodesIds ?? [];
	}
	export const activateSurfaceTab = (surface: number, child: number) =>
		dashboardView?.activateTab(surface, child) ?? false;
	export const toggleSurfaceAccordion = (surface: number, child: number) =>
		dashboardView?.toggleAccordion(surface, child) ?? false;
	export const activateWidgetAt = (surface: number, clientX: number, clientY: number) =>
		dashboardView?.activateAt(surface, clientX, clientY) ?? null;
	export const browseSurface = (surface: number) => dashboardView?.browse(surface) ?? false;
	export const chooseSurfaceChild = (surface: number, child: number) =>
		dashboardView?.chooseChild(surface, child) ?? false;
	export const searchSurfaceChildren = (surface: number, query: string) =>
		dashboardView?.searchChildren(surface, query);
	export const revealSurfaceChild = async (surface: number, child: number) =>
		(await dashboardView?.revealChild(surface, child)) ?? false;

	const clearFreeLayoutInteractionChrome = (): void => {
		setFreeLayoutHoverSurface(null);
		clearSurfaceInsertionIndicator();
		publishWidgetMoveSurfacePreview(null);
		setFreeLayoutAncestorShells(null);
		resetFreeDropTarget();
		resetOrderedDropTarget();
	};

	const selectWidgetNode = (selectionMode: SelectionMode = 'REPLACE'): void => {
		if (!editMode) {
			return;
		}
		session?.selectNode(liveNode.node_id, selectionMode);
	};

	const selectWidgetNodeFromKeyboard = (event: KeyboardEvent): void => {
		if (event.key !== 'Enter' && event.key !== ' ') {
			return;
		}
		event.preventDefault();
		selectWidgetNode(selectionModeFromEvent(event));
	};

	const widgetLabelPlacementClass = (labelPlacement: DashboardLabelPlacement | null): string => {
		if (labelPlacement === null) {
			return 'label-hidden';
		}
		return `label-${labelPlacement}`;
	};

	const handleWidgetPointerDown = (event: PointerEvent): void => {
		focusClosestPageViewport(event.currentTarget);
		if (!editMode || event.button !== 0) {
			return;
		}
		event.stopPropagation();
		const selectionMode = selectionModeFromEvent(event);
		if (supportsFreePlacement) {
			if (selectionMode !== 'REPLACE') {
				selectWidgetNode(selectionMode);
				return;
			}
			if (!isSelected) {
				selectWidgetNode('REPLACE');
			}
			beginFreeLayoutMove(event, resolveTransformNodes());
			return;
		}
		if (selectionMode !== 'REPLACE') {
			event.stopPropagation();
			selectWidgetNode(selectionMode);
			return;
		}
		if (!isSelected) {
			selectWidgetNode('REPLACE');
		}
		if (supportsOrdering) {
			beginWidgetMoveInteraction(event);
			return;
		}
		event.stopPropagation();
		selectWidgetNode('REPLACE');
	};

	onDestroy(() => {
		clearFreeLayoutInteractionChrome();
	});

	let supportsWidgetDrag = $derived(editMode && (supportsFreePlacement || supportsOrdering));
	let widgetInteractionActive = $derived(
		freeLayoutInteraction !== null ||
			widgetMoveInteraction !== null ||
			constrainedResizeInteraction !== null
	);
	let visibleResizeZones = $derived.by(() => {
		if (!editMode) {
			return [] as FreeLayoutResizeZone[];
		}
		if (supportsFreePlacement) {
			return freeLayoutResizeZones;
		}
		if (parentLayoutKind === 'horizontal') {
			return freeLayoutResizeZones.filter(
				(zone) => (zone.edges.left || zone.edges.right) && !zone.edges.top && !zone.edges.bottom
			);
		}
		if (parentLayoutKind === 'vertical') {
			return freeLayoutResizeZones.filter(
				(zone) => (zone.edges.top || zone.edges.bottom) && !zone.edges.left && !zone.edges.right
			);
		}
		return [] as FreeLayoutResizeZone[];
	});
	let parentSnapGrid = $derived(getDashboardSnapGrid(graph, parentNode));

	const widgetContent = createDashboardWidgetContent({
		get graph() {
			return graph;
		},
		get node() {
			return liveNode;
		},
		get isNodeWidget() {
			return isNodeWidget;
		},
		get editMode() {
			return editMode;
		},
		get placement() {
			return placement;
		}
	});
	let widgetKind = $derived(widgetContent.widgetKind);
	let boundNode = $derived(widgetContent.boundNode);
	let boundParamNode = $derived(widgetContent.boundParamNode);
	let boundParam = $derived(widgetContent.boundParam);
	let valueRange = $derived(widgetContent.valueRange);
	let sliderStep = $derived(widgetContent.sliderStep);
	let defaultChecked = $derived(widgetContent.defaultChecked);
	let maxChildLevel = $derived(widgetContent.maxChildLevel);
	let showEnableButton = $derived(widgetContent.showEnableButton);
	let NodeWidgetDisplayComponent = $derived(widgetContent.NodeWidgetDisplayComponent);
	let widgetWrapperLabelPlacement = $derived(widgetContent.widgetWrapperLabelPlacement);
	let hasExternalWidgetLabel = $derived(widgetContent.hasExternalWidgetLabel);
	let hasInsideWidgetLabel = $derived(widgetContent.hasInsideWidgetLabel);
	let genericSliderValue = $derived(widgetContent.genericSliderValue);
	let genericSliderDisabled = $derived(widgetContent.genericSliderDisabled);
	const applyGenericParamValue = widgetContent.applyGenericParamValue;
	const triggerGenericAction = widgetContent.triggerGenericAction;
	const applySliderValue = widgetContent.applySliderValue;
	const applyStringValue = widgetContent.applyStringValue;
	let textConfig = $derived(widgetContent.textConfig);
	let placeholderConfig = $derived(widgetContent.placeholderConfig);
	let multiline = $derived(widgetContent.multiline);
	let genericDisplayValue = $derived(widgetContent.genericDisplayValue);
</script>

<ContextMenu
	open={dashboardContextMenu.open}
	items={dashboardContextMenuItems}
	anchor={dashboardContextMenuAnchor}
	minWidthRem={12}
	menuClassName="gc-dashboard-context-menu"
	onOpenChange={(nextOpen) => {
		if (!nextOpen) {
			closeDashboardContextMenu();
		}
	}} />

{#snippet dashboardWidgetChrome()}
	{#if editMode && visibleResizeZones.length > 0}
		{#each visibleResizeZones as resizeZone}
			<div
				class="dashboard-resize-zone {resizeZone.name}"
				aria-hidden="true"
				onpointerdown={(event) => {
					if (!isSelected) {
						selectWidgetNode('REPLACE');
					}
					if (supportsFreePlacement) {
						beginFreeLayoutResize(event, resizeZone.edges, resolveTransformNodes());
						return;
					}
					beginConstrainedResize(event, resizeZone.edges);
				}}>
			</div>
		{/each}
	{/if}
	{#if anchorGuides.length > 0}
		<div class="dashboard-anchor-guides" aria-hidden="true">
			{#each anchorGuides as guide}
				<div class="dashboard-anchor-guide {guide.axis} {guide.direction}" style={guide.style}>
				</div>
			{/each}
		</div>
	{/if}
{/snippet}

{#snippet dashboardSurfaceHitZones()}
	{#if editMode && layoutKind !== 'free'}
		<div class="dashboard-surface-hit-zones" aria-hidden="true">
			<div class="dashboard-surface-hit-zone top"></div>
			<div class="dashboard-surface-hit-zone right"></div>
			<div class="dashboard-surface-hit-zone bottom"></div>
			<div class="dashboard-surface-hit-zone left"></div>
		</div>
	{/if}
{/snippet}

{#if isPage}
	<div
		class="gc-dashboard-canvas dashboard-page-viewport"
		bind:this={pageViewportElement}
		tabindex="-1"
		role="region"
		aria-label="Dashboard page viewer"
		onfocusin={() => {
			isPageViewportFocused = true;
		}}
		onfocusout={(event) => {
			const nextTarget = event.relatedTarget;
			if (nextTarget instanceof Node && (event.currentTarget as HTMLElement).contains(nextTarget)) {
				return;
			}
			isPageViewportFocused = false;
		}}
		onwheel={handlePageViewportWheel}
		onpointerdown={handlePageViewportPointerDown}>
		<div class="dashboard-page-scene" style={pageSceneStyle}>
			<div
				class="dashboard-page-scene-hitbox"
				role="presentation"
				aria-hidden="true"
				onpointerdown={handlePageScenePointerDown}>
			</div>
			<div class="dashboard-page-frame" bind:this={pageFrameElement} style={pageFrameStyle}>
				<div
					class="dashboard-surface dashboard-page {layoutKind}"
					class:surface-target-active={surfaceDragDepth > 0}
					class:edit-bleed-visible={editMode}
					class:snap-grid-visible={snapGridVisible}
					data-local-context-menu
					data-dashboard-drop-message="Drop inside page"
					data-dashboard-surface-node-id={String(liveNode.node_id)}
					role="region"
					aria-label="Dashboard page surface"
					style={surfaceStyle}
					oncontextmenu={handleSurfaceContextMenu}
					onpointerdown={handleSurfacePointerDown}
					ondragenter={handleSurfaceDragEnter}
					ondragover={handleSurfaceDragOver}
					ondragleave={handleSurfaceDragLeave}
					ondrop={(event) => {
						void handleSurfaceDrop(event);
					}}>
					{@render dashboardSurfaceHitZones()}
					<DashboardSurfaceContent
						surface={surfaceState}
						marquee={marqueeState}
						{editMode}
						dropIndicatorLabel="Drop to create a widget" />
				</div>
			</div>
			{#if editMode && pageSize.enabled}
				<div
					class="dashboard-page-visibility-overlay"
					style={pageBleedFrameStyle}
					aria-hidden="true">
				</div>
			{/if}
			{#if marqueeSelection}
				<div class="dashboard-marquee-selection" style={marqueeStyle} aria-hidden="true"></div>
			{/if}
		</div>
	</div>
{:else if isContainerWidget}
	<section
		class="gc-dashboard-canvas dashboard-widget-shell dashboard-container"
		class:selected={isSelected}
		class:free-layout-active={widgetInteractionActive}
		class:editable-free={supportsWidgetDrag}
		class:run-mode={!editMode}
		data-local-context-menu
		data-node-id={liveNode.node_id}
		data-dashboard-surface-id={String(parentNode?.node_id ?? '')}
		bind:this={widgetShellElement}
		style={widgetShellStyle}
		role="button"
		tabindex={editMode ? 0 : -1}
		oncontextmenu={handleWidgetContextMenu}
		onpointerdown={handleWidgetPointerDown}
		onkeydown={selectWidgetNodeFromKeyboard}>
		{@render dashboardWidgetChrome()}
		<div class="dashboard-widget-frame {widgetLabelPlacementClass(widgetWrapperLabelPlacement)}">
			{#if hasExternalWidgetLabel}
				<div class="dashboard-widget-label">{widgetLabelText}</div>
			{/if}
			<div class="dashboard-widget-body dashboard-container-body">
				<div
					class="dashboard-surface dashboard-container-surface {layoutKind}"
					class:surface-target-active={surfaceDragDepth > 0}
					class:snap-grid-visible={snapGridVisible}
					class:with-inline-label={hasInsideWidgetLabel}
					data-local-context-menu
					data-dashboard-drop-message="Drop inside container"
					data-dashboard-surface-node-id={String(liveNode.node_id)}
					role="group"
					aria-label="Dashboard container surface"
					style={surfaceStyle}
					oncontextmenu={handleSurfaceContextMenu}
					onpointerdown={handleSurfacePointerDown}
					ondragenter={handleSurfaceDragEnter}
					ondragover={handleSurfaceDragOver}
					ondragleave={handleSurfaceDragLeave}
					ondrop={(event) => {
						void handleSurfaceDrop(event);
					}}>
					{#if hasInsideWidgetLabel}
						<span class="dashboard-container-inline-label">{widgetLabelText}</span>
					{/if}
					{@render dashboardSurfaceHitZones()}
					<DashboardSurfaceContent
						surface={surfaceState}
						marquee={marqueeState}
						{editMode}
						compact={true}
						renderMarqueeInSurface={true} />
				</div>
			</div>
		</div>
	</section>
{:else if isNodeWidget}
	<section
		class="gc-dashboard-canvas dashboard-widget-shell dashboard-node-widget"
		class:selected={isSelected}
		class:binding-active={bindingDragDepth > 0}
		class:free-layout-active={widgetInteractionActive}
		class:editable-free={supportsWidgetDrag}
		class:run-mode={!editMode}
		data-local-context-menu
		data-node-id={liveNode.node_id}
		data-dashboard-surface-id={String(parentNode?.node_id ?? '')}
		bind:this={widgetShellElement}
		style={widgetShellStyle}
		role="button"
		tabindex={editMode ? 0 : -1}
		oncontextmenu={handleWidgetContextMenu}
		ondragenter={(event) => {
			handleBindingDragEnter(event, canBindNodeWidget);
		}}
		ondragover={(event) => {
			handleBindingDragOver(event, canBindNodeWidget);
		}}
		ondragleave={handleBindingDragLeave}
		ondrop={(event) => {
			void handleNodeWidgetBindingDrop(event);
		}}
		onpointerdown={handleWidgetPointerDown}
		onkeydown={selectWidgetNodeFromKeyboard}>
		{@render dashboardWidgetChrome()}
		<div class="dashboard-widget-frame {widgetLabelPlacementClass(widgetWrapperLabelPlacement)}">
			{#if hasExternalWidgetLabel}
				<div class="dashboard-widget-label">{widgetLabelText}</div>
			{/if}
			<div class="dashboard-widget-body">
				<div class="dashboard-widget-content inspector-body">
					{#if boundNode}
						<div class="dashboard-live-content" class:inert-mode={editMode} inert={editMode}>
							{#if NodeWidgetDisplayComponent}
								<NodeWidgetDisplayComponent
									widgetNode={liveNode}
									targetNode={boundNode}
									insideLabel={hasInsideWidgetLabel ? widgetLabelText : null}
									{maxChildLevel}
									{showEnableButton}
									{editMode} />
							{:else}
								<div class="dashboard-empty-inline">No widget type is available for this node.</div>
							{/if}
						</div>
					{:else}
						<div class="dashboard-empty-inline">This widget is not bound yet.</div>
					{/if}
				</div>
			</div>
		</div>
	</section>
{:else if isGenericWidget}
	<section
		class="gc-dashboard-canvas dashboard-widget-shell dashboard-generic-widget"
		class:selected={isSelected}
		class:binding-active={bindingDragDepth > 0}
		class:free-layout-active={widgetInteractionActive}
		class:editable-free={supportsWidgetDrag}
		class:run-mode={!editMode}
		data-local-context-menu
		data-node-id={liveNode.node_id}
		data-dashboard-surface-id={String(parentNode?.node_id ?? '')}
		bind:this={widgetShellElement}
		style={widgetShellStyle}
		role="button"
		tabindex={editMode ? 0 : -1}
		oncontextmenu={handleWidgetContextMenu}
		ondragenter={(event) => {
			handleBindingDragEnter(event, canBindGenericWidget);
		}}
		ondragover={(event) => {
			handleBindingDragOver(event, canBindGenericWidget);
		}}
		ondragleave={handleBindingDragLeave}
		ondrop={(event) => {
			void handleGenericWidgetBindingDrop(event);
		}}
		onpointerdown={handleWidgetPointerDown}
		onkeydown={selectWidgetNodeFromKeyboard}>
		{@render dashboardWidgetChrome()}
		<div class="dashboard-widget-frame {widgetLabelPlacementClass(widgetWrapperLabelPlacement)}">
			{#if hasExternalWidgetLabel}
				<div class="dashboard-widget-label">{widgetLabelText}</div>
			{/if}
			<div class="dashboard-widget-body">
				<div class="dashboard-widget-content generic-body">
					<div class="dashboard-live-content" class:inert-mode={editMode} inert={editMode}>
						{#if widgetKind === 'button'}
							<button
								type="button"
								class="generic-button"
								class:with-inside-label={hasInsideWidgetLabel}
								onclick={() => {
									void triggerGenericAction();
								}}
								disabled={editMode}>
								{#if hasInsideWidgetLabel}
									<span class="generic-inline-label">{widgetLabelText}</span>
								{/if}
								<span class="generic-button-text"
									>{textConfig || boundParamNode?.meta.label || 'Trigger'}</span>
							</button>
						{:else if widgetKind === 'slider'}
							<div class="generic-slider-wrap">
								<div class="generic-slider-bar">
									<Slider
										value={genericSliderValue}
										min={valueRange[0]}
										max={valueRange[1]}
										step={sliderStep}
										disabled={genericSliderDisabled}
										readOnly={Boolean(boundParam?.read_only)}
										label={hasInsideWidgetLabel ? widgetLabelText : ''}
										onValueChange={(nextValue: number) => {
											void applySliderValue(String(nextValue));
										}} />
								</div>
								<div class="generic-readout">
									{boundParam ? formatParamValue(boundParam.value) : 'Unbound'}
								</div>
							</div>
						{:else if widgetKind === 'checkbox'}
							<label class="generic-checkbox" class:with-inside-label={hasInsideWidgetLabel}>
								{#if hasInsideWidgetLabel}
									<span class="generic-inline-label">{widgetLabelText}</span>
								{/if}
								<input
									type="checkbox"
									checked={boundParam?.value.kind === 'bool'
										? boundParam.value.value
										: defaultChecked}
									disabled={editMode || boundParam?.value.kind !== 'bool'}
									onchange={(event) => {
										void applyGenericParamValue({
											kind: 'bool',
											value: (event.target as HTMLInputElement).checked
										});
									}} />
								<span class="generic-checkbox-text"
									>{textConfig || boundParamNode?.meta.label || 'Enabled'}</span>
							</label>
						{:else if widgetKind === 'textInput'}
							{#if multiline}
								<div
									class="generic-input-shell multiline"
									class:with-inside-label={hasInsideWidgetLabel}>
									{#if hasInsideWidgetLabel}
										<span class="generic-inline-label">{widgetLabelText}</span>
									{/if}
									<textarea
										rows="4"
										placeholder={placeholderConfig}
										disabled={editMode ||
											!boundParam ||
											(boundParam.value.kind !== 'str' &&
												boundParam.value.kind !== 'file' &&
												boundParam.value.kind !== 'enum' &&
												boundParam.value.kind !== 'css_value')}
										onchange={(event) => {
											void applyStringValue((event.target as HTMLTextAreaElement).value);
										}}
										>{boundParam
											? boundParam.value.kind === 'css_value'
												? formatCssValue(boundParam.value)
												: boundParam.value.kind === 'str' ||
													  boundParam.value.kind === 'file' ||
													  boundParam.value.kind === 'enum'
													? boundParam.value.value
													: textConfig
											: textConfig}</textarea>
								</div>
							{:else}
								<div class="generic-input-shell" class:with-inside-label={hasInsideWidgetLabel}>
									{#if hasInsideWidgetLabel}
										<span class="generic-inline-label">{widgetLabelText}</span>
									{/if}
									<input
										type="text"
										value={boundParam
											? boundParam.value.kind === 'css_value'
												? formatCssValue(boundParam.value)
												: boundParam.value.kind === 'str' ||
													  boundParam.value.kind === 'file' ||
													  boundParam.value.kind === 'enum'
													? boundParam.value.value
													: textConfig
											: textConfig}
										placeholder={placeholderConfig}
										disabled={editMode ||
											!boundParam ||
											(boundParam.value.kind !== 'str' &&
												boundParam.value.kind !== 'file' &&
												boundParam.value.kind !== 'enum' &&
												boundParam.value.kind !== 'css_value')}
										onchange={(event) => {
											void applyStringValue((event.target as HTMLInputElement).value);
										}} />
								</div>
							{/if}
						{:else}
							<div class="generic-text-display" class:with-inside-label={hasInsideWidgetLabel}>
								{#if hasInsideWidgetLabel}
									<span class="generic-inline-label">{widgetLabelText}</span>
								{/if}
								<span class="generic-text-display-value"
									>{genericDisplayValue || 'Drop a parameter or set static text.'}</span>
							</div>
						{/if}
					</div>
				</div>
			</div>
		</div>
	</section>
{/if}
