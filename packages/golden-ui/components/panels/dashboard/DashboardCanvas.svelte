<script lang="ts">
	import { onDestroy } from 'svelte';
	import { registerCommandHandler } from '../../../store/commands.svelte';
	import { appState, type SelectionMode } from '../../../store/workbench.svelte';
	import {
		createUiEditSession,
		sendPatchMetaIntent,
		sendRemoveNodeIntent,
		sendSetParamIntent
	} from '../../../store/ui-intents';
	import type { NodeId, ParamEventBehaviour, ParamValue, UiNodeDto } from '../../../types';
	import {
		cssValueToPx,
		formatCssValue,
		parseCssValue,
		pxToCssValue,
		type CssUnitConversionContext,
		type CssValueData
	} from '../../../css-value';
	import DashboardCanvasSelf from './DashboardCanvas.svelte';
	import { resolveDashboardNodeWidgetType } from './dashboard-node-widget-registry';
	import ContextMenu from '../../common/ContextMenu.svelte';
	import Slider from '../../common/Slider.svelte';
	import type { ContextMenuAnchor, ContextMenuItem } from '../../common/context-menu';

	import {
		bindDashboardGenericWidgetTarget,
		bindDashboardNodeWidgetTarget,
		createDashboardContainerWidget,
		createDashboardNodeWidget,
		duplicateDashboardWidget,
		getDashboardContainerCreationDefaults,
		getDashboardGenericWidgetCreationDefaults,
		getDashboardNodeWidgetCreationDefaults,
		getDashboardWidgetOrderingState,
		moveDashboardWidgetByDelta,
		moveDashboardWidgetToBack,
		moveDashboardWidgetToFront,
		moveDashboardWidgetToSurface,
		type DashboardWidgetCreationPlacement,
		wrapDashboardWidgetInContainer
	} from './dashboard-actions';

	type DashboardGenericPreviewKind = 'button' | 'slider' | 'checkbox' | 'textInput' | 'text';
	import { readDashboardDragPayload, type DashboardDragPayload } from './dashboard-drag';
	import {
		type DashboardAnchor,
		type DashboardGridDirection,
		type DashboardLabelPlacement,
		formatParamValue,
		getDashboardGridSettings,
		getDashboardLayoutKind,
		getDashboardPageSize,
		getDashboardPlacement,
		getDashboardSnapGrid,
		getDirectItemChildren,
		getDirectParam,
		getDirectParamNode,
		getLiveNode,
		getGap,
		resolveReferenceTarget,
		type DashboardPlacement,
		type DashboardLayoutKind
	} from './dashboard-model';

	type DashboardPersistedPageView = {
		panX: number;
		panY: number;
		zoom: number;
	};

	type DashboardPersistedPageViewChangeHandler = (
		pageNodeId: NodeId,
		nextPageView: DashboardPersistedPageView
	) => void;

	type SurfaceDropPreview = {
		label: string;
		targetKind: DashboardDragPayload['kind'];
		placement: DashboardWidgetCreationPlacement;
		targetIndex: number | null;
		genericWidgetKind: DashboardGenericPreviewKind | null;
		previewText: string | null;
		previewPlaceholder: string | null;
		multiline: boolean;
	};
	type PendingSurfaceDrop = {
		preview: SurfaceDropPreview;
		knownChildIds: NodeId[];
		knownDirectChildIds: NodeId[];
	};
	type PendingWidgetMoveCommit = {
		preview: SurfaceDropPreview | null;
		hiddenChildNodeId: NodeId | null;
		expectedChildIds: NodeId[];
	};
	type NonFreeSurfaceFlowRenderItem =
		| {
				key: string;
				kind: 'child';
				child: UiNodeDto;
		  }
		| {
				key: string;
				kind: 'preview';
				preview: SurfaceDropPreview;
		  };
	type WidgetMovePreviewEventDetail = {
		surfaceNodeId: NodeId | null;
		preview: SurfaceDropPreview | null;
		sourceSurfaceNodeId: NodeId | null;
		sourceWidgetNodeId: NodeId | null;
	};
	type WidgetMoveCommitEventDetail = {
		targetSurfaceNodeId: NodeId | null;
		preview: SurfaceDropPreview | null;
		sourceSurfaceNodeId: NodeId | null;
		sourceWidgetNodeId: NodeId | null;
	};
	type DashboardContextMenuScope = 'surface' | 'widget';
	type DashboardContextMenuState = {
		open: boolean;
		scope: DashboardContextMenuScope;
		x: number;
		y: number;
		surfacePlacement: DashboardWidgetCreationPlacement | null;
	};

	let {
		node,
		editMode = false,
		pageNavigationEnabled = true,
		persistedPageView = null,
		onPersistedPageViewChange = null
	} = $props<{
		node: UiNodeDto;
		editMode?: boolean;
		pageNavigationEnabled?: boolean;
		persistedPageView?: DashboardPersistedPageView | null;
		onPersistedPageViewChange?: DashboardPersistedPageViewChangeHandler | null;
	}>();

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveNode = $derived(getLiveNode(graph, node));
	let isSelected = $derived(session?.isNodeSelected(liveNode.node_id) ?? false);
	let isPage = $derived(liveNode.node_type === 'dashboard_page');
	let isContainerWidget = $derived(liveNode.node_type === 'dashboard_widget_container');
	let isNodeWidget = $derived(liveNode.node_type === 'dashboard_node_widget');
	let isGenericWidget = $derived(liveNode.node_type === 'dashboard_generic_widget');
	let isWidget = $derived(isContainerWidget || isNodeWidget || isGenericWidget);
	let isLayoutSurface = $derived(isPage || isContainerWidget);

	let placement = $derived(getDashboardPlacement(graph, liveNode));
	let widgetLabelPlacement = $derived(placement.labelPlacement);
	let widgetLabelText = $derived(liveNode.meta.label.trim());
	let parentNode = $derived.by(() => {
		const parentId = graph?.parentById.get(liveNode.node_id);
		if (parentId === undefined || !graph) {
			return null;
		}
		return graph.nodesById.get(parentId) ?? null;
	});
	let parentLayoutKind = $derived(getDashboardLayoutKind(graph, parentNode, 'free'));
	let layoutKind = $derived(getDashboardLayoutKind(graph, liveNode, isPage ? 'free' : 'vertical'));
	let gap = $derived(getGap(graph, liveNode));
	let gridSettings = $derived(getDashboardGridSettings(graph, liveNode));
	let childWidgets = $derived(getDirectItemChildren(graph, liveNode));
	let renderedChildWidgets = $derived.by(() =>
		effectiveHiddenChildNodeId === null || layoutKind === 'free'
			? childWidgets
			: childWidgets.filter((child) => child.node_id !== effectiveHiddenChildNodeId)
	);
	let widgetOrderingState = $derived(getDashboardWidgetOrderingState(graph, liveNode.node_id));
	let canInsertContainer = $derived.by(() => {
		if (!editMode || !isLayoutSurface) {
			return false;
		}
		return liveNode.creatable_user_items.some(
			(candidate) => candidate.node_type === 'dashboard_widget_container'
		);
	});
	let canWrapWidget = $derived.by(() => {
		if (
			!editMode ||
			!isWidget ||
			!parentNode ||
			!liveNode.meta.user_permissions.can_remove_and_duplicate
		) {
			return false;
		}
		return parentNode.creatable_user_items.some(
			(candidate) => candidate.node_type === 'dashboard_widget_container'
		);
	});
	let canDuplicateWidget = $derived.by(() => {
		if (
			!editMode ||
			!isWidget ||
			!parentNode ||
			!liveNode.meta.user_permissions.can_remove_and_duplicate
		) {
			return false;
		}
		return parentNode.creatable_user_items.some(
			(candidate) => candidate.node_type === liveNode.node_type
		);
	});
	let canDeleteWidget = $derived(
		editMode && isWidget && liveNode.meta.user_permissions.can_remove_and_duplicate
	);
	let canBringBackward = $derived(
		editMode && isWidget && (widgetOrderingState?.currentIndex ?? 0) > 0
	);
	let canBringToBack = $derived(canBringBackward);
	let canBringForward = $derived.by(() => {
		if (!editMode || !isWidget || !widgetOrderingState) {
			return false;
		}
		return widgetOrderingState.currentIndex < widgetOrderingState.itemCount - 1;
	});
	let canBringToFront = $derived(canBringForward);

	let activeTabNodeId = $state<NodeId | null>(null);
	let openAccordionNodeIds = $state<NodeId[]>([]);
	let surfaceDragDepth = $state(0);
	let bindingDragDepth = $state(0);
	let isPageViewportFocused = $state(false);
	let pageViewportElement = $state<HTMLDivElement | null>(null);
	let pageFrameElement = $state<HTMLDivElement | null>(null);
	let widgetShellElement = $state<HTMLElement | null>(null);
	let pageViewportSize = $state({ width: 0, height: 0 });
	let pageViewportResizeFrame = $state<number | null>(null);
	let surfaceDropPreview = $state<SurfaceDropPreview | null>(null);
	let widgetMoveSurfacePreview = $state<SurfaceDropPreview | null>(null);
	let widgetMoveHiddenChildNodeId = $state<NodeId | null>(null);
	let pendingSurfaceDrop = $state<PendingSurfaceDrop | null>(null);
	let pendingWidgetMoveCommit = $state<PendingWidgetMoveCommit | null>(null);
	let dashboardContextMenu = $state<DashboardContextMenuState>({
		open: false,
		scope: 'surface',
		x: 0,
		y: 0,
		surfacePlacement: null
	});
	let effectiveHiddenChildNodeId = $derived(
		widgetMoveHiddenChildNodeId ?? pendingWidgetMoveCommit?.hiddenChildNodeId ?? null
	);
	let displayedSurfaceDropPreview = $derived(
		surfaceDropPreview ??
			widgetMoveSurfacePreview ??
			pendingSurfaceDrop?.preview ??
			pendingWidgetMoveCommit?.preview ??
			null
	);
	let rendersInlineSurfaceDropPreview = $derived.by(() => {
		return displayedSurfaceDropPreview !== null;
	});
	let pendingPersistedPageView = $state<DashboardPersistedPageView | null>(null);
	let appliedPersistedPageView = $state<DashboardPersistedPageView | null>(null);
	type ObservedAnchorPlacement = {
		nodeId: NodeId;
		anchor: DashboardAnchor;
		position: { x: number; y: number };
		size: { width: CssValueData; height: CssValueData };
	};
	type AnchorGuideAxis = 'horizontal' | 'vertical';
	type AnchorGuideDirection = 'left' | 'right' | 'up' | 'down';
	type AnchorGuide = {
		axis: AnchorGuideAxis;
		direction: AnchorGuideDirection;
		style: string;
	};

	type FreeLayoutInteractionMode = 'move' | 'resize';
	type FreeLayoutResizeEdges = {
		left: boolean;
		right: boolean;
		top: boolean;
		bottom: boolean;
	};
	type FreeLayoutPreview = {
		position: { x: number; y: number };
		size: { width: CssValueData; height: CssValueData };
	};
	type FreeLayoutTarget = {
		nodeId: NodeId;
		anchor: DashboardAnchor;
		positionParamNodeId: NodeId | null;
		positionParamBehaviour: ParamEventBehaviour | null;
		widthParamNodeId: NodeId | null;
		widthParamBehaviour: ParamEventBehaviour | null;
		heightParamNodeId: NodeId | null;
		heightParamBehaviour: ParamEventBehaviour | null;
		startPreview: FreeLayoutPreview;
		startRect: FreeLayoutRect;
	};
	type FreeLayoutTargetPreview = {
		target: FreeLayoutTarget;
		preview: FreeLayoutPreview;
	};
	type FreeLayoutPreviewSet = {
		primaryNodeId: NodeId;
		previews: FreeLayoutTargetPreview[];
	};
	type FreeLayoutDropTarget = {
		surfaceNodeId: NodeId;
		element: HTMLElement;
		placement: DashboardWidgetCreationPlacement;
		targetIndex: number | null;
		indicator: SurfaceInsertionIndicator | null;
	};
	type FreeLayoutInteraction = {
		pointerId: number;
		mode: FreeLayoutInteractionMode;
		resizeEdges: FreeLayoutResizeEdges | null;
		primaryNodeId: NodeId;
		sourceSurfaceNodeId: NodeId;
		selectionRect: FreeLayoutRect;
		targets: FreeLayoutTarget[];
		startClient: [number, number];
		rootRemPx: number;
		surfaceWidthPx: number;
		surfaceHeightPx: number;
		surfaceScaleX: number;
		surfaceScaleY: number;
		viewportWidthPx: number;
		viewportHeightPx: number;
	};
	type SurfaceInsertionIndicator = {
		left: number;
		top: number;
		width: number;
		height: number;
	};
	type SurfaceInsertionTarget = {
		targetIndex: number;
		indicator: SurfaceInsertionIndicator | null;
	};
	type WidgetMoveDropTarget = {
		surfaceNodeId: NodeId;
		element: HTMLElement;
		layoutKind: DashboardLayoutKind;
		targetIndex: number | null;
		placement: DashboardWidgetCreationPlacement;
		indicator: SurfaceInsertionIndicator | null;
	};
	type WidgetMoveInteraction = {
		pointerId: number;
		sourceSurfaceNodeId: NodeId;
		startClient: [number, number];
	};
	type ConstrainedResizeInteraction = {
		pointerId: number;
		axis: 'x' | 'y';
		sign: -1 | 1;
		startClient: [number, number];
		paramNodeId: NodeId;
		paramBehaviour: ParamEventBehaviour;
		startSize: CssValueData;
		startSizePx: number;
		rootRemPx: number;
		surfaceWidthPx: number;
		surfaceHeightPx: number;
		surfaceScaleX: number;
		surfaceScaleY: number;
		viewportWidthPx: number;
		viewportHeightPx: number;
		lastSent: CssValueData | null;
	};
	type SurfaceMetrics = {
		width: number;
		height: number;
		scaleX: number;
		scaleY: number;
	};
	type FreeLayoutRect = {
		left: number;
		top: number;
		width: number;
		height: number;
	};
	type DashboardPageViewState = {
		pointerId: number | null;
		lastClient: [number, number] | null;
		panX: number;
		panY: number;
		zoom: number;
	};
	type FreeLayoutResizeZone = {
		name: string;
		edges: FreeLayoutResizeEdges;
	};
	type MarqueeSelectionState = {
		pointerId: number;
		surfaceNodeId: NodeId;
		surfaceElement: HTMLElement;
		startClient: [number, number];
		currentClient: [number, number];
		baseSelection: NodeId[];
		selectionMode: SelectionMode;
	};
	type SurfacePressState = {
		pointerId: number;
		surfaceNodeId: NodeId;
		surfaceElement: HTMLElement;
		startClient: [number, number];
		baseSelection: NodeId[];
		selectionMode: SelectionMode;
	};

	let freeLayoutInteraction = $state<FreeLayoutInteraction | null>(null);
	let freeLayoutPreview = $state<FreeLayoutPreviewSet | null>(null);
	let freeLayoutDropTarget = $state<FreeLayoutDropTarget | null>(null);
	let widgetMoveInteraction = $state<WidgetMoveInteraction | null>(null);
	let widgetMoveDropTarget = $state<WidgetMoveDropTarget | null>(null);
	let constrainedResizeInteraction = $state<ConstrainedResizeInteraction | null>(null);
	let marqueeSelection = $state<MarqueeSelectionState | null>(null);
	let surfacePress = $state<SurfacePressState | null>(null);
	let pageView = $state<DashboardPageViewState>({
		pointerId: null,
		lastClient: null,
		panX: 0,
		panY: 0,
		zoom: 1
	});
	let freeLayoutHoverSurfaceElement: HTMLElement | null = null;
	let surfaceInsertionIndicatorElement: HTMLElement | null = null;
	let freeLayoutAncestorShells: HTMLElement[] = [];

	const minFreeWidgetWidthRem = 0.5;
	const minFreeWidgetHeightRem = 0.5;
	const marqueeDragThresholdPx = 4;
	const freeLayoutResizeZones: FreeLayoutResizeZone[] = [
		{ name: 'north-west', edges: { left: true, right: false, top: true, bottom: false } },
		{ name: 'north', edges: { left: false, right: false, top: true, bottom: false } },
		{ name: 'north-east', edges: { left: false, right: true, top: true, bottom: false } },
		{ name: 'east', edges: { left: false, right: true, top: false, bottom: false } },
		{ name: 'south-east', edges: { left: false, right: true, top: false, bottom: true } },
		{ name: 'south', edges: { left: false, right: false, top: false, bottom: true } },
		{ name: 'south-west', edges: { left: true, right: false, top: false, bottom: true } },
		{ name: 'west', edges: { left: true, right: false, top: false, bottom: false } }
	];
	const minPageZoom = 0.25;
	const maxPageZoom = 4;
	let freeLayoutCommitFrame: number | null = null;
	let freeLayoutCommitInFlight = false;
	let pendingFreeLayoutCommit: FreeLayoutPreviewSet | null = null;
	let lastCommittedFreeLayoutPreview: FreeLayoutPreviewSet | null = null;
	let freeLayoutEditSession: ReturnType<typeof createUiEditSession> | null = null;
	let observedAnchorPlacementCache: ObservedAnchorPlacement | null = null;
	let pageViewAnimationFrame = $state<number | null>(null);
	let pageViewAnimationTarget = $state<DashboardPersistedPageView | null>(null);
	let previousPageFitScale = $state<number | null>(null);
	let previousPageFitScaleEditMode = $state<boolean | null>(null);

	const getGraph = () => appState.session?.graph.state ?? null;

	const getRootRemPixels = (): number => {
		if (typeof window === 'undefined') {
			return 16;
		}
		const fontSize = Number.parseFloat(window.getComputedStyle(document.documentElement).fontSize);
		return Number.isFinite(fontSize) && fontSize > 0 ? fontSize : 16;
	};

	const getViewportWidthPx = (): number =>
		typeof window === 'undefined' ? 1280 : Math.max(window.innerWidth, 1);

	const getViewportHeightPx = (): number =>
		typeof window === 'undefined' ? 720 : Math.max(window.innerHeight, 1);

	const getClosestSurfaceMetrics = (
		target: EventTarget | null,
		rootRemPx: number
	): SurfaceMetrics => {
		if (!(target instanceof Element)) {
			return { width: getViewportWidthPx(), height: 20 * rootRemPx, scaleX: 1, scaleY: 1 };
		}
		const surface = target.closest('.dashboard-surface');
		if (!surface) {
			return { width: getViewportWidthPx(), height: 20 * rootRemPx, scaleX: 1, scaleY: 1 };
		}
		const rect = surface.getBoundingClientRect();
		const layoutWidth =
			surface instanceof HTMLElement ? Math.max(surface.clientWidth, 1) : Math.max(rect.width, 1);
		const layoutHeight =
			surface instanceof HTMLElement ? Math.max(surface.clientHeight, 1) : Math.max(rect.height, 1);
		return {
			width: Math.max(layoutWidth, rootRemPx),
			height: Math.max(layoutHeight, rootRemPx),
			scaleX: Math.max(rect.width, 1) / layoutWidth,
			scaleY: Math.max(rect.height, 1) / layoutHeight
		};
	};

	const getElementSurfaceMetrics = (element: HTMLElement, rootRemPx: number): SurfaceMetrics => {
		const rect = element.getBoundingClientRect();
		const layoutWidth = Math.max(element.clientWidth, 1);
		const layoutHeight = Math.max(element.clientHeight, 1);
		return {
			width: Math.max(layoutWidth, rootRemPx),
			height: Math.max(layoutHeight, rootRemPx),
			scaleX: Math.max(rect.width, 1) / layoutWidth,
			scaleY: Math.max(rect.height, 1) / layoutHeight
		};
	};

	const getDirectDashboardLayoutElement = (surface: EventTarget | null): HTMLElement | null => {
		if (!(surface instanceof HTMLElement)) {
			return null;
		}
		for (const child of Array.from(surface.children)) {
			if (child instanceof HTMLElement && child.classList.contains('dashboard-layout')) {
				return child;
			}
		}
		return null;
	};

	const selectionModeFromEvent = (
		event: Pick<MouseEvent, 'ctrlKey' | 'metaKey' | 'shiftKey'>
	): SelectionMode => {
		if (event.ctrlKey || event.metaKey) {
			return 'TOGGLE';
		}
		if (event.shiftKey) {
			return 'ADD';
		}
		return 'REPLACE';
	};

	const isInteractiveSurfaceTarget = (target: EventTarget | null): boolean => {
		if (!(target instanceof Element)) {
			return false;
		}
		return Boolean(
			target.closest(
				'button, input, textarea, select, option, label, a, [contenteditable="true"], [role="button"]'
			)
		);
	};

	const focusClosestPageViewport = (target: EventTarget | null): void => {
		if (!(target instanceof Element)) {
			return;
		}
		const viewport = target.closest('.dashboard-page-viewport');
		if (viewport instanceof HTMLElement) {
			viewport.focus();
		}
	};

	const clearSurfaceInsertionIndicator = (): void => {
		if (!surfaceInsertionIndicatorElement) {
			return;
		}
		surfaceInsertionIndicatorElement.classList.remove('dashboard-flow-drop-target');
		surfaceInsertionIndicatorElement.style.removeProperty('--dashboard-flow-drop-indicator-left');
		surfaceInsertionIndicatorElement.style.removeProperty('--dashboard-flow-drop-indicator-top');
		surfaceInsertionIndicatorElement.style.removeProperty('--dashboard-flow-drop-indicator-width');
		surfaceInsertionIndicatorElement.style.removeProperty('--dashboard-flow-drop-indicator-height');
		surfaceInsertionIndicatorElement = null;
	};

	const setSurfaceInsertionIndicator = (
		surfaceElement: HTMLElement | null,
		indicator: SurfaceInsertionIndicator | null
	): void => {
		if (surfaceInsertionIndicatorElement && surfaceInsertionIndicatorElement !== surfaceElement) {
			clearSurfaceInsertionIndicator();
		}
		if (!surfaceElement || !indicator) {
			clearSurfaceInsertionIndicator();
			return;
		}
		surfaceInsertionIndicatorElement = surfaceElement;
		surfaceElement.classList.add('dashboard-flow-drop-target');
		surfaceElement.style.setProperty('--dashboard-flow-drop-indicator-left', `${indicator.left}px`);
		surfaceElement.style.setProperty('--dashboard-flow-drop-indicator-top', `${indicator.top}px`);
		surfaceElement.style.setProperty(
			'--dashboard-flow-drop-indicator-width',
			`${indicator.width}px`
		);
		surfaceElement.style.setProperty(
			'--dashboard-flow-drop-indicator-height',
			`${indicator.height}px`
		);
	};

	const clearSurfaceDropState = (): void => {
		surfaceDragDepth = 0;
		surfaceDropPreview = null;
		clearSurfaceInsertionIndicator();
	};

	const pointFallsWithinElement = (
		element: HTMLElement,
		clientX: number,
		clientY: number
	): boolean => {
		const rect = element.getBoundingClientRect();
		return (
			clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom
		);
	};

	const resolveOwnedDashboardSurfaceElement = (
		candidate: HTMLElement,
		clientX: number,
		clientY: number
	): HTMLElement | null => {
		if (candidate.classList.contains('dashboard-surface')) {
			return candidate;
		}

		const containerShell = candidate.closest(
			'.dashboard-widget-shell.dashboard-container[data-node-id]'
		);
		if (containerShell instanceof HTMLElement) {
			const surfaceNodeId = containerShell.dataset.nodeId;
			if (surfaceNodeId) {
				const ownedSurface = containerShell.querySelector<HTMLElement>(
					`.dashboard-container-surface[data-dashboard-surface-node-id="${surfaceNodeId}"]`
				);
				if (ownedSurface && pointFallsWithinElement(containerShell, clientX, clientY)) {
					return ownedSurface;
				}
			}
		}

		const pageFrame = candidate.closest('.dashboard-page-frame');
		if (pageFrame instanceof HTMLElement) {
			const pageSurface = pageFrame.querySelector<HTMLElement>(
				'.dashboard-page[data-dashboard-surface-node-id]'
			);
			if (pageSurface && pointFallsWithinElement(pageSurface, clientX, clientY)) {
				return pageSurface;
			}
		}

		return candidate.closest('.dashboard-surface');
	};

	const resolveDashboardSurfaceAtPoint = (
		clientX: number,
		clientY: number,
		ignoredWidgetNodeIds: ReadonlySet<NodeId>
	): { surfaceNodeId: NodeId; element: HTMLElement } | null => {
		if (typeof document === 'undefined') {
			return null;
		}

		type SurfaceCandidate = {
			surfaceNodeId: NodeId;
			element: HTMLElement;
			nestingDepth: number;
			area: number;
		};

		const surfaceNestingDepth = (surface: HTMLElement): number => {
			let depth = 0;
			let cursor = surface.parentElement;
			while (cursor) {
				if (cursor.classList.contains('dashboard-surface')) {
					depth += 1;
				}
				cursor = cursor.parentElement;
			}
			return depth;
		};

		const visited = new Set<HTMLElement>();
		const candidates: SurfaceCandidate[] = [];
		for (const candidate of document.elementsFromPoint(clientX, clientY)) {
			if (!(candidate instanceof HTMLElement)) {
				continue;
			}
			const surface = resolveOwnedDashboardSurfaceElement(candidate, clientX, clientY);
			if (!(surface instanceof HTMLElement) || visited.has(surface)) {
				continue;
			}
			visited.add(surface);
			const ownerShell = surface.closest('.dashboard-widget-shell');
			if (ownerShell instanceof HTMLElement) {
				const ownerNodeId = Number(ownerShell.dataset.nodeId ?? '');
				if (Number.isFinite(ownerNodeId) && ignoredWidgetNodeIds.has(ownerNodeId as NodeId)) {
					continue;
				}
			}
			const surfaceNodeId = Number(surface.dataset.dashboardSurfaceNodeId ?? '');
			if (!Number.isFinite(surfaceNodeId)) {
				continue;
			}
			const rect = surface.getBoundingClientRect();
			candidates.push({
				surfaceNodeId: surfaceNodeId as NodeId,
				element: surface,
				nestingDepth: surfaceNestingDepth(surface),
				area: Math.max(rect.width * rect.height, 0)
			});
		}
		if (candidates.length === 0) {
			return null;
		}
		candidates.sort((left, right) => {
			if (left.nestingDepth !== right.nestingDepth) {
				return right.nestingDepth - left.nestingDepth;
			}
			if (left.area !== right.area) {
				return left.area - right.area;
			}
			return 0;
		});
		return {
			surfaceNodeId: candidates[0].surfaceNodeId,
			element: candidates[0].element
		};
	};

	const currentSurfaceOwnsDragEvent = (event: DragEvent): boolean => {
		const currentTarget = event.currentTarget;
		if (!(currentTarget instanceof HTMLElement)) {
			return false;
		}
		const resolvedSurface = resolveDashboardSurfaceAtPoint(
			event.clientX,
			event.clientY,
			new Set<NodeId>()
		);
		if (resolvedSurface) {
			return resolvedSurface.element === currentTarget;
		}
		if (!(event.target instanceof HTMLElement)) {
			return true;
		}
		return (
			resolveOwnedDashboardSurfaceElement(event.target, event.clientX, event.clientY) ===
			currentTarget
		);
	};

	type SurfaceOrderAnchor = {
		nodeId: NodeId;
		rect: DOMRect;
	};

	const resolveSurfaceContentElement = (
		surfaceElement: HTMLElement,
		targetLayoutKind: DashboardLayoutKind
	): HTMLElement => {
		if (targetLayoutKind === 'tabs') {
			return surfaceElement.querySelector<HTMLElement>('.dashboard-tab-strip') ?? surfaceElement;
		}
		if (targetLayoutKind === 'accordion') {
			return surfaceElement.querySelector<HTMLElement>('.dashboard-accordion') ?? surfaceElement;
		}
		return getDirectDashboardLayoutElement(surfaceElement) ?? surfaceElement;
	};

	const resolveSurfaceOrderAnchors = (
		surfaceNode: UiNodeDto,
		surfaceElement: HTMLElement,
		targetLayoutKind: DashboardLayoutKind,
		ignoredNodeIds: ReadonlySet<NodeId>
	): SurfaceOrderAnchor[] => {
		if (!graph) {
			return [];
		}
		const orderedChildren = getDirectItemChildren(graph, surfaceNode).filter(
			(child) => !ignoredNodeIds.has(child.node_id)
		);
		if (orderedChildren.length === 0) {
			return [];
		}

		if (targetLayoutKind === 'tabs') {
			const buttons = Array.from(
				surfaceElement.querySelectorAll<HTMLElement>(
					'.dashboard-tab-strip > button:not(.dashboard-drop-preview-tab)'
				)
			);
			return orderedChildren
				.map((child, index) => {
					const button = buttons[index];
					return button
						? {
								nodeId: child.node_id,
								rect: button.getBoundingClientRect()
							}
						: null;
				})
				.filter((anchor): anchor is SurfaceOrderAnchor => anchor !== null);
		}

		if (targetLayoutKind === 'accordion') {
			const sections = Array.from(
				surfaceElement.querySelectorAll<HTMLElement>(
					'.dashboard-accordion > .dashboard-accordion-item'
				)
			);
			return orderedChildren
				.map((child, index) => {
					const section = sections[index];
					if (!section) {
						return null;
					}
					const button = section.querySelector<HTMLElement>('button');
					const anchorElement = button ?? section;
					return {
						nodeId: child.node_id,
						rect: anchorElement.getBoundingClientRect()
					};
				})
				.filter((anchor): anchor is SurfaceOrderAnchor => anchor !== null);
		}

		return orderedChildren
			.map((child) => {
				const shell = surfaceElement.querySelector<HTMLElement>(
					`.dashboard-widget-shell[data-node-id="${String(child.node_id)}"][data-dashboard-surface-id="${String(surfaceNode.node_id)}"]`
				);
				return shell
					? {
							nodeId: child.node_id,
							rect: shell.getBoundingClientRect()
						}
					: null;
			})
			.filter((anchor): anchor is SurfaceOrderAnchor => anchor !== null);
	};

	const buildVerticalInsertionIndicator = (
		surfaceRect: DOMRect,
		trackRect: DOMRect,
		surfaceMetrics: SurfaceMetrics,
		xPx: number,
		targetRect?: DOMRect
	): SurfaceInsertionIndicator => {
		const rootRemPx = getRootRemPixels();
		const scaleX = Math.max(surfaceMetrics.scaleX, 1e-6);
		const scaleY = Math.max(surfaceMetrics.scaleY, 1e-6);
		const trackTop = (trackRect.top - surfaceRect.top) / scaleY;
		const trackBottom = (trackRect.bottom - surfaceRect.top) / scaleY;
		const targetTop = targetRect ? (targetRect.top - surfaceRect.top) / scaleY : null;
		const targetBottom = targetRect ? (targetRect.bottom - surfaceRect.top) / scaleY : null;
		const top = Math.max(trackTop + 0.3 * rootRemPx, 0);
		const indicatorTop = targetTop !== null ? Math.max(targetTop + 0.3 * rootRemPx, top) : top;
		const bottom =
			targetBottom !== null
				? Math.min(targetBottom - 0.3 * rootRemPx, trackBottom - 0.3 * rootRemPx)
				: trackBottom - 0.3 * rootRemPx;
		const height = Math.max(bottom - indicatorTop, 1.5 * rootRemPx);
		return {
			left: Math.max(0, xPx / scaleX - 0.1 * rootRemPx),
			top: indicatorTop,
			width: 0.2 * rootRemPx,
			height
		};
	};

	const buildHorizontalInsertionIndicator = (
		surfaceRect: DOMRect,
		trackRect: DOMRect,
		surfaceMetrics: SurfaceMetrics,
		yPx: number,
		targetRect?: DOMRect
	): SurfaceInsertionIndicator => {
		const rootRemPx = getRootRemPixels();
		const scaleX = Math.max(surfaceMetrics.scaleX, 1e-6);
		const scaleY = Math.max(surfaceMetrics.scaleY, 1e-6);
		const trackLeft = (trackRect.left - surfaceRect.left) / scaleX;
		const trackRight = (trackRect.right - surfaceRect.left) / scaleX;
		const targetLeft = targetRect ? (targetRect.left - surfaceRect.left) / scaleX : null;
		const targetRight = targetRect ? (targetRect.right - surfaceRect.left) / scaleX : null;
		const left = Math.max(trackLeft + 0.3 * rootRemPx, 0);
		const indicatorLeft = targetLeft !== null ? Math.max(targetLeft + 0.3 * rootRemPx, left) : left;
		const right =
			targetRight !== null
				? Math.min(targetRight - 0.3 * rootRemPx, trackRight - 0.3 * rootRemPx)
				: trackRight - 0.3 * rootRemPx;
		const width = Math.max(right - indicatorLeft, 1.5 * rootRemPx);
		return {
			left: indicatorLeft,
			top: Math.max(0, yPx / scaleY - 0.1 * rootRemPx),
			width,
			height: 0.2 * rootRemPx
		};
	};

	const resolveSurfaceInsertionTarget = (
		surfaceNode: UiNodeDto,
		surfaceElement: HTMLElement,
		targetLayoutKind: DashboardLayoutKind,
		clientX: number,
		clientY: number,
		ignoredNodeIds: ReadonlySet<NodeId>
	): SurfaceInsertionTarget | null => {
		if (!graph) {
			return null;
		}
		if (targetLayoutKind === 'free') {
			return {
				targetIndex: getDirectItemChildren(graph, surfaceNode).length,
				indicator: null
			};
		}

		const rootRemPx = getRootRemPixels();
		const surfaceRect = surfaceElement.getBoundingClientRect();
		const surfaceMetrics = getElementSurfaceMetrics(surfaceElement, rootRemPx);
		const trackElement = resolveSurfaceContentElement(surfaceElement, targetLayoutKind);
		const trackRect = trackElement.getBoundingClientRect();
		const anchors = resolveSurfaceOrderAnchors(
			surfaceNode,
			surfaceElement,
			targetLayoutKind,
			ignoredNodeIds
		);

		if (anchors.length === 0) {
			if (targetLayoutKind === 'horizontal' || targetLayoutKind === 'tabs') {
				return {
					targetIndex: 0,
					indicator: buildVerticalInsertionIndicator(
						surfaceRect,
						trackRect,
						surfaceMetrics,
						trackRect.left - surfaceRect.left + 0.5 * rootRemPx
					)
				};
			}
			return {
				targetIndex: 0,
				indicator: buildHorizontalInsertionIndicator(
					surfaceRect,
					trackRect,
					surfaceMetrics,
					trackRect.top - surfaceRect.top + 0.5 * rootRemPx
				)
			};
		}

		if (targetLayoutKind === 'horizontal' || targetLayoutKind === 'tabs') {
			for (let index = 0; index < anchors.length; index += 1) {
				const anchor = anchors[index];
				if (clientX < anchor.rect.left + anchor.rect.width * 0.5) {
					return {
						targetIndex: index,
						indicator: buildVerticalInsertionIndicator(
							surfaceRect,
							trackRect,
							surfaceMetrics,
							anchor.rect.left - surfaceRect.left,
							anchor.rect
						)
					};
				}
			}
			const last = anchors[anchors.length - 1];
			return {
				targetIndex: anchors.length,
				indicator: buildVerticalInsertionIndicator(
					surfaceRect,
					trackRect,
					surfaceMetrics,
					last.rect.right - surfaceRect.left,
					last.rect
				)
			};
		}

		if (targetLayoutKind === 'vertical' || targetLayoutKind === 'accordion') {
			for (let index = 0; index < anchors.length; index += 1) {
				const anchor = anchors[index];
				if (clientY < anchor.rect.top + anchor.rect.height * 0.5) {
					return {
						targetIndex: index,
						indicator: buildHorizontalInsertionIndicator(
							surfaceRect,
							trackRect,
							surfaceMetrics,
							anchor.rect.top - surfaceRect.top,
							anchor.rect
						)
					};
				}
			}
			const last = anchors[anchors.length - 1];
			return {
				targetIndex: anchors.length,
				indicator: buildHorizontalInsertionIndicator(
					surfaceRect,
					trackRect,
					surfaceMetrics,
					last.rect.bottom - surfaceRect.top,
					last.rect
				)
			};
		}

		const gridDirection = getDashboardGridSettings(graph, surfaceNode).direction;
		if (gridDirection === 'column') {
			for (let index = 0; index < anchors.length; index += 1) {
				const anchor = anchors[index];
				if (
					clientX < anchor.rect.left ||
					(clientX <= anchor.rect.right && clientY < anchor.rect.top + anchor.rect.height * 0.5)
				) {
					return {
						targetIndex: index,
						indicator: buildHorizontalInsertionIndicator(
							surfaceRect,
							trackRect,
							surfaceMetrics,
							anchor.rect.top - surfaceRect.top,
							anchor.rect
						)
					};
				}
			}
			const last = anchors[anchors.length - 1];
			return {
				targetIndex: anchors.length,
				indicator: buildHorizontalInsertionIndicator(
					surfaceRect,
					trackRect,
					surfaceMetrics,
					last.rect.bottom - surfaceRect.top,
					last.rect
				)
			};
		}

		for (let index = 0; index < anchors.length; index += 1) {
			const anchor = anchors[index];
			if (
				clientY < anchor.rect.top ||
				(clientY <= anchor.rect.bottom && clientX < anchor.rect.left + anchor.rect.width * 0.5)
			) {
				return {
					targetIndex: index,
					indicator: buildVerticalInsertionIndicator(
						surfaceRect,
						trackRect,
						surfaceMetrics,
						anchor.rect.left - surfaceRect.left,
						anchor.rect
					)
				};
			}
		}
		const last = anchors[anchors.length - 1];
		return {
			targetIndex: anchors.length,
			indicator: buildVerticalInsertionIndicator(
				surfaceRect,
				trackRect,
				surfaceMetrics,
				last.rect.right - surfaceRect.left,
				last.rect
			)
		};
	};

	const setFreeLayoutHoverSurface = (nextSurface: HTMLElement | null): void => {
		if (freeLayoutHoverSurfaceElement === nextSurface) {
			return;
		}
		freeLayoutHoverSurfaceElement?.classList.remove('dashboard-reparent-target');
		freeLayoutHoverSurfaceElement = nextSurface;
		freeLayoutHoverSurfaceElement?.classList.add('dashboard-reparent-target');
	};

	const setFreeLayoutAncestorShells = (target: EventTarget | null): void => {
		for (const shell of freeLayoutAncestorShells) {
			shell.classList.remove('dashboard-descendant-free-layout-active');
		}
		freeLayoutAncestorShells = [];
		if (!(target instanceof Element)) {
			return;
		}

		let cursor = target.parentElement?.closest('.dashboard-widget-shell');
		while (cursor instanceof HTMLElement) {
			cursor.classList.add('dashboard-descendant-free-layout-active');
			freeLayoutAncestorShells.push(cursor);
			cursor = cursor.parentElement?.closest('.dashboard-widget-shell');
		}
	};

	const widgetMovePreviewEventName = 'gc-dashboard-widget-move-preview';
	const widgetMoveCommitEventName = 'gc-dashboard-widget-move-commit';

	const buildCurrentWidgetMovePreview = (
		dropTarget: Pick<WidgetMoveDropTarget, 'placement' | 'targetIndex'>
	): SurfaceDropPreview => {
		const genericWidgetKind = isGenericWidget ? (widgetKind as DashboardGenericPreviewKind) : null;
		return {
			label: widgetLabelText.length > 0 ? widgetLabelText : liveNode.meta.label,
			targetKind: isNodeWidget || isContainerWidget ? 'node' : 'parameter',
			placement: dropTarget.placement,
			targetIndex: dropTarget.targetIndex,
			genericWidgetKind,
			previewText: isGenericWidget
				? genericDisplayValue || textConfig || widgetLabelText || null
				: null,
			previewPlaceholder: isGenericWidget ? placeholderConfig || widgetLabelText || null : null,
			multiline: isGenericWidget ? multiline : false
		};
	};

	const publishWidgetMoveSurfacePreview = (
		dropTarget: Pick<WidgetMoveDropTarget, 'surfaceNodeId' | 'placement' | 'targetIndex'> | null
	): void => {
		if (typeof window === 'undefined') {
			return;
		}
		const detail: WidgetMovePreviewEventDetail = dropTarget
			? {
					surfaceNodeId: dropTarget.surfaceNodeId,
					preview: buildCurrentWidgetMovePreview(dropTarget),
					sourceSurfaceNodeId: parentNode?.node_id ?? null,
					sourceWidgetNodeId: liveNode.node_id
				}
			: {
					surfaceNodeId: null,
					preview: null,
					sourceSurfaceNodeId: null,
					sourceWidgetNodeId: null
				};
		window.dispatchEvent(
			new CustomEvent<WidgetMovePreviewEventDetail>(widgetMovePreviewEventName, { detail })
		);
	};

	const publishWidgetMoveCommit = (
		dropTarget: Pick<WidgetMoveDropTarget, 'surfaceNodeId' | 'placement' | 'targetIndex'> | null,
		sourceSurfaceNodeId: NodeId | null = null
	): void => {
		if (typeof window === 'undefined') {
			return;
		}
		const detail: WidgetMoveCommitEventDetail = dropTarget
			? {
					targetSurfaceNodeId: dropTarget.surfaceNodeId,
					preview: buildCurrentWidgetMovePreview(dropTarget),
					sourceSurfaceNodeId,
					sourceWidgetNodeId: liveNode.node_id
				}
			: {
					targetSurfaceNodeId: null,
					preview: null,
					sourceSurfaceNodeId: null,
					sourceWidgetNodeId: null
				};
		window.dispatchEvent(
			new CustomEvent<WidgetMoveCommitEventDetail>(widgetMoveCommitEventName, { detail })
		);
	};

	const clearFreeLayoutInteractionChrome = (): void => {
		setFreeLayoutHoverSurface(null);
		clearSurfaceInsertionIndicator();
		publishWidgetMoveSurfacePreview(null);
		setFreeLayoutAncestorShells(null);
		freeLayoutDropTarget = null;
		widgetMoveDropTarget = null;
	};

	const normalizeClientRect = (
		startClient: [number, number],
		currentClient: [number, number]
	): FreeLayoutRect => {
		const left = Math.min(startClient[0], currentClient[0]);
		const top = Math.min(startClient[1], currentClient[1]);
		return {
			left,
			top,
			width: Math.abs(currentClient[0] - startClient[0]),
			height: Math.abs(currentClient[1] - startClient[1])
		};
	};

	const rectsIntersect = (
		left: DOMRect | FreeLayoutRect,
		right: DOMRect | FreeLayoutRect
	): boolean => {
		return (
			left.left < right.left + right.width &&
			left.left + left.width > right.left &&
			left.top < right.top + right.height &&
			left.top + left.height > right.top
		);
	};

	const collectMarqueeNodeIds = (selection: MarqueeSelectionState): NodeId[] => {
		const childNodeIdByString = new Map(
			childWidgets.map((child) => [String(child.node_id), child.node_id])
		);
		const selector = `.dashboard-widget-shell[data-dashboard-surface-id="${String(selection.surfaceNodeId)}"]`;
		const selectionRect = normalizeClientRect(selection.startClient, selection.currentClient);
		const nodeIds: NodeId[] = [];
		for (const element of selection.surfaceElement.querySelectorAll<HTMLElement>(selector)) {
			const nodeId = childNodeIdByString.get(element.dataset.nodeId ?? '');
			if (nodeId === undefined) {
				continue;
			}
			const rect = element.getBoundingClientRect();
			if (rect.width <= 0 || rect.height <= 0) {
				continue;
			}
			if (!rectsIntersect(selectionRect, rect)) {
				continue;
			}
			nodeIds.push(nodeId);
		}
		return nodeIds;
	};

	const applyMarqueeSelection = (selection: MarqueeSelectionState): void => {
		const intersectingNodeIds = collectMarqueeNodeIds(selection);
		let nextSelection = intersectingNodeIds;
		if (selection.selectionMode === 'ADD') {
			nextSelection = [...selection.baseSelection];
			for (const nodeId of intersectingNodeIds) {
				if (!nextSelection.includes(nodeId)) {
					nextSelection.push(nodeId);
				}
			}
		} else if (selection.selectionMode === 'TOGGLE') {
			const toggled = new Set(selection.baseSelection);
			for (const nodeId of intersectingNodeIds) {
				if (toggled.has(nodeId)) {
					toggled.delete(nodeId);
				} else {
					toggled.add(nodeId);
				}
			}
			nextSelection = Array.from(toggled);
		}
		session?.selectNodes(nextSelection, 'REPLACE');
	};

	const canScrollAxis = (element: HTMLElement, axis: 'x' | 'y', delta: number): boolean => {
		if (delta === 0) {
			return false;
		}
		const style = window.getComputedStyle(element);
		const overflowValue = axis === 'x' ? style.overflowX : style.overflowY;
		if (!['auto', 'scroll', 'overlay'].includes(overflowValue)) {
			return false;
		}
		const scrollOffset = axis === 'x' ? element.scrollLeft : element.scrollTop;
		const scrollExtent =
			axis === 'x'
				? element.scrollWidth - element.clientWidth
				: element.scrollHeight - element.clientHeight;
		if (scrollExtent <= 1) {
			return false;
		}
		return delta < 0 ? scrollOffset > 1 : scrollOffset < scrollExtent - 1;
	};

	const wheelTargetsScrollableContent = (event: WheelEvent): boolean => {
		if (!(event.target instanceof Element) || !pageViewportElement) {
			return false;
		}
		let current: Element | null = event.target;
		while (current && current !== pageViewportElement) {
			if (
				current instanceof HTMLElement &&
				(canScrollAxis(current, 'y', event.deltaY) || canScrollAxis(current, 'x', event.deltaX))
			) {
				return true;
			}
			current = current.parentElement;
		}
		return false;
	};

	const normalizeWheelDeltaY = (event: WheelEvent): number => {
		if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) {
			return event.deltaY * getRootRemPixels();
		}
		if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) {
			return event.deltaY * pageViewportHeightPx;
		}
		return event.deltaY;
	};

	const createCssUnitContext = (
		interaction: Pick<
			FreeLayoutInteraction,
			'rootRemPx' | 'surfaceWidthPx' | 'surfaceHeightPx' | 'viewportWidthPx' | 'viewportHeightPx'
		>,
		axis: 'x' | 'y'
	): CssUnitConversionContext => ({
		rootRemPx: interaction.rootRemPx,
		axisBasePx: axis === 'x' ? interaction.surfaceWidthPx : interaction.surfaceHeightPx,
		viewportWidthPx: interaction.viewportWidthPx,
		viewportHeightPx: interaction.viewportHeightPx
	});

	const sameNodeIdList = (left: NodeId[], right: NodeId[]): boolean => {
		if (left.length !== right.length) {
			return false;
		}
		for (let index = 0; index < left.length; index += 1) {
			if (left[index] !== right[index]) {
				return false;
			}
		}
		return true;
	};

	const moveNodeIdInList = (
		nodeIds: NodeId[],
		nodeId: NodeId,
		targetIndex: number | null
	): NodeId[] => {
		const nextNodeIds = nodeIds.filter((candidate) => candidate !== nodeId);
		const nextIndex =
			targetIndex === null
				? nextNodeIds.length
				: Math.max(0, Math.min(nextNodeIds.length, Math.round(targetIndex)));
		nextNodeIds.splice(nextIndex, 0, nodeId);
		return nextNodeIds;
	};

	const isFreeLayoutTransformableWidget = (candidate: UiNodeDto): boolean => {
		return (
			candidate.node_type === 'dashboard_widget_container' ||
			candidate.node_type === 'dashboard_node_widget' ||
			candidate.node_type === 'dashboard_generic_widget'
		);
	};

	const cloneFreeLayoutPreview = (preview: FreeLayoutPreview): FreeLayoutPreview => ({
		position: {
			x: preview.position.x,
			y: preview.position.y
		},
		size: {
			width: { ...preview.size.width },
			height: { ...preview.size.height }
		}
	});

	const cloneFreeLayoutPreviewSet = (previewSet: FreeLayoutPreviewSet): FreeLayoutPreviewSet => ({
		primaryNodeId: previewSet.primaryNodeId,
		previews: previewSet.previews.map((entry) => ({
			target: entry.target,
			preview: cloneFreeLayoutPreview(entry.preview)
		}))
	});

	const resolveFreeLayoutPreview = (
		previewSet: FreeLayoutPreviewSet | null,
		nodeId: NodeId
	): FreeLayoutPreview | null => {
		if (!previewSet) {
			return null;
		}
		for (const entry of previewSet.previews) {
			if (entry.target.nodeId === nodeId) {
				return entry.preview;
			}
		}
		return null;
	};

	const buildPreviewSetFromTargets = (
		primaryNodeId: NodeId,
		targets: FreeLayoutTarget[]
	): FreeLayoutPreviewSet => ({
		primaryNodeId,
		previews: targets.map((target) => ({
			target,
			preview: cloneFreeLayoutPreview(target.startPreview)
		}))
	});

	const currentPlacementPreview = (): FreeLayoutPreview => ({
		position: {
			x: placement.position.x,
			y: placement.position.y
		},
		size: {
			width: { ...placement.size.width },
			height: { ...placement.size.height }
		}
	});

	const sameCssValue = (left: CssValueData, right: CssValueData): boolean =>
		left.value === right.value && left.unit === right.unit;

	const samePosition = (left: { x: number; y: number }, right: { x: number; y: number }): boolean =>
		left.x === right.x && left.y === right.y;

	const samePlacementSize = (
		left: { width: CssValueData; height: CssValueData },
		right: { width: CssValueData; height: CssValueData }
	): boolean => sameCssValue(left.width, right.width) && sameCssValue(left.height, right.height);

	const snapToGridPx = (valuePx: number, snapPx: number): number => {
		if (!(snapPx > 0)) {
			return valuePx;
		}
		return Math.round(valuePx / snapPx) * snapPx;
	};

	const cancelScheduledFreeLayoutCommit = (): void => {
		if (typeof window !== 'undefined' && freeLayoutCommitFrame !== null) {
			window.cancelAnimationFrame(freeLayoutCommitFrame);
		}
		freeLayoutCommitFrame = null;
		pendingFreeLayoutCommit = null;
	};

	const currentDraggedPlacement = (
		previewSet: FreeLayoutPreviewSet | null
	): DashboardWidgetCreationPlacement => {
		const currentPreview = resolveFreeLayoutPreview(previewSet, liveNode.node_id);
		return {
			anchor: placement.anchor,
			position: currentPreview ? currentPreview.position : placement.position,
			size: currentPreview ? currentPreview.size : placement.size,
			sizeEnabled: {
				width: placement.sizeEnabled.width,
				height: placement.sizeEnabled.height
			}
		};
	};

	const anchorXFromAnchor = (anchor: DashboardAnchor): 'left' | 'center' | 'right' => {
		if (anchor.endsWith('right')) {
			return 'right';
		}
		if (anchor === 'center' || anchor.endsWith('center')) {
			return 'center';
		}
		return 'left';
	};

	const anchorYFromAnchor = (anchor: DashboardAnchor): 'top' | 'center' | 'bottom' => {
		if (anchor.startsWith('bottom')) {
			return 'bottom';
		}
		if (anchor === 'center' || anchor.startsWith('center')) {
			return 'center';
		}
		return 'top';
	};

	const anchorBasePx = (
		anchor: 'left' | 'center' | 'right' | 'top' | 'center' | 'bottom',
		axisExtentPx: number
	): number => {
		if (anchor === 'center') {
			return axisExtentPx * 0.5;
		}
		if (anchor === 'right' || anchor === 'bottom') {
			return axisExtentPx;
		}
		return 0;
	};

	const anchorOffsetPx = (
		anchor: 'left' | 'center' | 'right' | 'top' | 'center' | 'bottom',
		sizePx: number
	): number => {
		if (anchor === 'center') {
			return sizePx * 0.5;
		}
		if (anchor === 'right' || anchor === 'bottom') {
			return sizePx;
		}
		return 0;
	};

	const anchorBaseExpression = (
		anchor: 'left' | 'center' | 'right' | 'top' | 'center' | 'bottom'
	): string => {
		if (anchor === 'center') {
			return '50%';
		}
		if (anchor === 'right' || anchor === 'bottom') {
			return '100%';
		}
		return '0px';
	};

	const freeLayoutRectFromPlacement = (
		placementLike: {
			position: { x: number; y: number };
			size: { width: CssValueData; height: CssValueData };
			anchor: DashboardAnchor;
		},
		interaction: Pick<
			FreeLayoutInteraction,
			'surfaceWidthPx' | 'surfaceHeightPx' | 'rootRemPx' | 'viewportWidthPx' | 'viewportHeightPx'
		>
	): FreeLayoutRect => {
		const xContext = createCssUnitContext(interaction, 'x');
		const yContext = createCssUnitContext(interaction, 'y');
		const widthPx = Math.max(0, cssValueToPx(placementLike.size.width, 'x', xContext));
		const heightPx = Math.max(0, cssValueToPx(placementLike.size.height, 'y', yContext));
		const anchorX = anchorXFromAnchor(placementLike.anchor);
		const anchorY = anchorYFromAnchor(placementLike.anchor);
		const anchorPointXPx =
			anchorBasePx(anchorX, interaction.surfaceWidthPx) + placementLike.position.x;
		const anchorPointYPx =
			anchorBasePx(anchorY, interaction.surfaceHeightPx) + placementLike.position.y;
		return {
			left: anchorPointXPx - anchorOffsetPx(anchorX, widthPx),
			top: anchorPointYPx - anchorOffsetPx(anchorY, heightPx),
			width: widthPx,
			height: heightPx
		};
	};

	const buildFreeLayoutDropPlacement = (
		targetSurfaceNode: UiNodeDto,
		targetSurfaceElement: HTMLElement,
		basePlacement: DashboardWidgetCreationPlacement,
		clientX: number,
		clientY: number
	): DashboardWidgetCreationPlacement => {
		const targetLayoutKind = getDashboardLayoutKind(graph, targetSurfaceNode, 'free');
		if (targetLayoutKind !== 'free') {
			return {
				anchor: basePlacement.anchor,
				position: { ...basePlacement.position },
				size: {
					width: { ...basePlacement.size.width },
					height: { ...basePlacement.size.height }
				},
				sizeEnabled: basePlacement.sizeEnabled
					? {
							width: basePlacement.sizeEnabled.width,
							height: basePlacement.sizeEnabled.height
						}
					: undefined
			};
		}

		const layoutElement =
			getDirectDashboardLayoutElement(targetSurfaceElement) ?? targetSurfaceElement;
		const rootRemPx = getRootRemPixels();
		const metrics = getElementSurfaceMetrics(layoutElement, rootRemPx);
		const rect = layoutElement.getBoundingClientRect();
		const surfaceContext = {
			rootRemPx,
			surfaceWidthPx: metrics.width,
			surfaceHeightPx: metrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const widthPx = Math.max(
			0,
			cssValueToPx(basePlacement.size.width, 'x', createCssUnitContext(surfaceContext, 'x'))
		);
		const heightPx = Math.max(
			0,
			cssValueToPx(basePlacement.size.height, 'y', createCssUnitContext(surfaceContext, 'y'))
		);
		const localCenterX = (clientX - rect.left) / Math.max(metrics.scaleX, 1e-6);
		const localCenterY = (clientY - rect.top) / Math.max(metrics.scaleY, 1e-6);
		const snapGrid = getDashboardSnapGrid(graph, targetSurfaceNode);
		const snapPx = snapGrid.enabled ? snapGrid.step * rootRemPx : 0;
		const snappedLeft = snapToGridPx(localCenterX - widthPx * 0.5, snapPx);
		const snappedTop = snapToGridPx(localCenterY - heightPx * 0.5, snapPx);
		const anchorX = anchorXFromAnchor(basePlacement.anchor);
		const anchorY = anchorYFromAnchor(basePlacement.anchor);

		return {
			anchor: basePlacement.anchor,
			position: {
				x: snappedLeft + anchorOffsetPx(anchorX, widthPx) - anchorBasePx(anchorX, metrics.width),
				y: snappedTop + anchorOffsetPx(anchorY, heightPx) - anchorBasePx(anchorY, metrics.height)
			},
			size: {
				width: { ...basePlacement.size.width },
				height: { ...basePlacement.size.height }
			},
			sizeEnabled: basePlacement.sizeEnabled
				? {
						width: basePlacement.sizeEnabled.width,
						height: basePlacement.sizeEnabled.height
					}
				: undefined
		};
	};

	const resolveFreeLayoutDropTargetAtPoint = (
		clientX: number,
		clientY: number,
		previewSet: FreeLayoutPreviewSet | null
	): FreeLayoutDropTarget | null => {
		if (
			!graph ||
			!editMode ||
			!isWidget ||
			!freeLayoutInteraction ||
			freeLayoutInteraction.mode !== 'move' ||
			freeLayoutInteraction.targets.length !== 1
		) {
			return null;
		}

		const resolvedSurface = resolveDashboardSurfaceAtPoint(
			clientX,
			clientY,
			new Set(freeLayoutInteraction.targets.map((target) => target.nodeId))
		);
		if (
			!resolvedSurface ||
			resolvedSurface.surfaceNodeId === freeLayoutInteraction.sourceSurfaceNodeId
		) {
			return null;
		}

		const targetSurfaceNode = graph.nodesById.get(resolvedSurface.surfaceNodeId);
		if (!targetSurfaceNode) {
			return null;
		}
		if (
			!targetSurfaceNode.creatable_user_items.some(
				(candidate) => candidate.node_type === liveNode.node_type
			)
		) {
			return null;
		}

		const targetLayoutKind = getDashboardLayoutKind(graph, targetSurfaceNode, 'free');
		const insertionTarget =
			targetLayoutKind === 'free'
				? null
				: resolveSurfaceInsertionTarget(
						targetSurfaceNode,
						resolvedSurface.element,
						targetLayoutKind,
						clientX,
						clientY,
						new Set<NodeId>([liveNode.node_id])
					);

		return {
			surfaceNodeId: resolvedSurface.surfaceNodeId,
			element: resolvedSurface.element,
			placement: buildFreeLayoutDropPlacement(
				targetSurfaceNode,
				resolvedSurface.element,
				currentDraggedPlacement(previewSet),
				clientX,
				clientY
			),
			targetIndex: insertionTarget?.targetIndex ?? null,
			indicator: insertionTarget?.indicator ?? null
		};
	};

	const resolveWidgetMoveDropTargetAtPoint = (
		clientX: number,
		clientY: number
	): WidgetMoveDropTarget | null => {
		if (!graph || !editMode || !isWidget || !widgetMoveInteraction) {
			return null;
		}

		const resolvedSurface = resolveDashboardSurfaceAtPoint(
			clientX,
			clientY,
			new Set<NodeId>([liveNode.node_id])
		);
		if (!resolvedSurface) {
			return null;
		}

		const targetSurfaceNode = graph.nodesById.get(resolvedSurface.surfaceNodeId);
		if (!targetSurfaceNode) {
			return null;
		}
		if (
			!targetSurfaceNode.creatable_user_items.some(
				(candidate) => candidate.node_type === liveNode.node_type
			)
		) {
			return null;
		}

		const targetLayoutKind = getDashboardLayoutKind(graph, targetSurfaceNode, 'free');
		const insertionTarget = resolveSurfaceInsertionTarget(
			targetSurfaceNode,
			resolvedSurface.element,
			targetLayoutKind,
			clientX,
			clientY,
			new Set<NodeId>([liveNode.node_id])
		);

		return {
			surfaceNodeId: resolvedSurface.surfaceNodeId,
			element: resolvedSurface.element,
			layoutKind: targetLayoutKind,
			targetIndex: targetLayoutKind === 'free' ? null : (insertionTarget?.targetIndex ?? 0),
			placement: buildFreeLayoutDropPlacement(
				targetSurfaceNode,
				resolvedSurface.element,
				currentDraggedPlacement(null),
				clientX,
				clientY
			),
			indicator: targetLayoutKind === 'free' ? null : (insertionTarget?.indicator ?? null)
		};
	};

	const freeLayoutPreviewFromRect = (
		rect: FreeLayoutRect,
		anchor: DashboardAnchor,
		interaction: Pick<
			FreeLayoutInteraction,
			'surfaceWidthPx' | 'surfaceHeightPx' | 'rootRemPx' | 'viewportWidthPx' | 'viewportHeightPx'
		>,
		positionTemplate: { x: number; y: number },
		sizeTemplate: { width: CssValueData; height: CssValueData }
	): FreeLayoutPreview => {
		const xContext = createCssUnitContext(interaction, 'x');
		const yContext = createCssUnitContext(interaction, 'y');
		const anchorX = anchorXFromAnchor(anchor);
		const anchorY = anchorYFromAnchor(anchor);
		const anchorPointXPx = rect.left + anchorOffsetPx(anchorX, rect.width);
		const anchorPointYPx = rect.top + anchorOffsetPx(anchorY, rect.height);
		return {
			position: {
				x: anchorPointXPx - anchorBasePx(anchorX, interaction.surfaceWidthPx),
				y: anchorPointYPx - anchorBasePx(anchorY, interaction.surfaceHeightPx)
			},
			size: {
				width: pxToCssValue(rect.width, sizeTemplate.width, 'x', xContext),
				height: pxToCssValue(rect.height, sizeTemplate.height, 'y', yContext)
			}
		};
	};

	const clampPageZoom = (zoom: number): number =>
		Math.min(maxPageZoom, Math.max(minPageZoom, zoom));

	const resolveSelectedFreeLayoutWidgets = (): UiNodeDto[] => {
		if (!graph || !parentNode || !session) {
			return [];
		}
		const selectedNodeIds = new Set(session.selectedNodesIds);
		return getDirectItemChildren(graph, parentNode).filter(
			(candidate) =>
				selectedNodeIds.has(candidate.node_id) && isFreeLayoutTransformableWidget(candidate)
		);
	};

	const resolveTransformNodes = (): UiNodeDto[] => {
		const selectedWidgets = resolveSelectedFreeLayoutWidgets();
		if (isSelected && selectedWidgets.length > 1) {
			return selectedWidgets;
		}
		return [liveNode];
	};

	const buildFreeLayoutTargets = (
		nodes: UiNodeDto[],
		interaction: Pick<
			FreeLayoutInteraction,
			'rootRemPx' | 'surfaceWidthPx' | 'surfaceHeightPx' | 'viewportWidthPx' | 'viewportHeightPx'
		>
	): FreeLayoutTarget[] => {
		if (!graph) {
			return [];
		}
		const targets: FreeLayoutTarget[] = [];
		for (const targetNode of nodes) {
			const targetPlacement = getDashboardPlacement(graph, targetNode);
			const positionParamNode = getDirectParamNode(graph, targetNode, 'position');
			const widthParamNode = getDirectParamNode(graph, targetNode, 'width');
			const heightParamNode = getDirectParamNode(graph, targetNode, 'height');
			targets.push({
				nodeId: targetNode.node_id,
				anchor: targetPlacement.anchor,
				positionParamNodeId:
					positionParamNode?.data.kind === 'parameter' ? positionParamNode.node_id : null,
				positionParamBehaviour:
					positionParamNode?.data.kind === 'parameter'
						? positionParamNode.data.param.event_behaviour
						: null,
				widthParamNodeId: widthParamNode?.data.kind === 'parameter' ? widthParamNode.node_id : null,
				widthParamBehaviour:
					widthParamNode?.data.kind === 'parameter'
						? widthParamNode.data.param.event_behaviour
						: null,
				heightParamNodeId:
					heightParamNode?.data.kind === 'parameter' ? heightParamNode.node_id : null,
				heightParamBehaviour:
					heightParamNode?.data.kind === 'parameter'
						? heightParamNode.data.param.event_behaviour
						: null,
				startPreview: {
					position: {
						x: targetPlacement.position.x,
						y: targetPlacement.position.y
					},
					size: {
						width: { ...targetPlacement.size.width },
						height: { ...targetPlacement.size.height }
					}
				},
				startRect: freeLayoutRectFromPlacement(targetPlacement, interaction)
			});
		}
		return targets;
	};

	const buildSelectionRect = (targets: FreeLayoutTarget[]): FreeLayoutRect | null => {
		if (targets.length === 0) {
			return null;
		}
		let left = Number.POSITIVE_INFINITY;
		let top = Number.POSITIVE_INFINITY;
		let right = Number.NEGATIVE_INFINITY;
		let bottom = Number.NEGATIVE_INFINITY;
		for (const target of targets) {
			left = Math.min(left, target.startRect.left);
			top = Math.min(top, target.startRect.top);
			right = Math.max(right, target.startRect.left + target.startRect.width);
			bottom = Math.max(bottom, target.startRect.top + target.startRect.height);
		}
		return {
			left,
			top,
			width: Math.max(0, right - left),
			height: Math.max(0, bottom - top)
		};
	};

	const buildFreeLayoutMovePreview = (
		interaction: FreeLayoutInteraction,
		deltaXPx: number,
		deltaYPx: number,
		snapPx: number
	): FreeLayoutPreviewSet => {
		const primaryTarget =
			interaction.targets.find((target) => target.nodeId === interaction.primaryNodeId) ??
			interaction.targets[0];
		let appliedDeltaX = deltaXPx;
		let appliedDeltaY = deltaYPx;
		if (snapPx > 0) {
			appliedDeltaX =
				snapToGridPx(primaryTarget.startRect.left + appliedDeltaX, snapPx) -
				primaryTarget.startRect.left;
			appliedDeltaY =
				snapToGridPx(primaryTarget.startRect.top + appliedDeltaY, snapPx) -
				primaryTarget.startRect.top;
		}
		return {
			primaryNodeId: interaction.primaryNodeId,
			previews: interaction.targets.map((target) => ({
				target,
				preview: freeLayoutPreviewFromRect(
					{
						...target.startRect,
						left: target.startRect.left + appliedDeltaX,
						top: target.startRect.top + appliedDeltaY
					},
					target.anchor,
					interaction,
					target.startPreview.position,
					target.startPreview.size
				)
			}))
		};
	};

	const buildFreeLayoutResizePreview = (
		interaction: FreeLayoutInteraction,
		deltaXPx: number,
		deltaYPx: number,
		snapPx: number
	): FreeLayoutPreviewSet | null => {
		const resizeEdges = interaction.resizeEdges;
		if (!resizeEdges) {
			return null;
		}
		const minWidthPx = minFreeWidgetWidthRem * interaction.rootRemPx;
		const minHeightPx = minFreeWidgetHeightRem * interaction.rootRemPx;
		const startRect = interaction.selectionRect;
		let requiredScaleX = 0;
		let requiredScaleY = 0;
		for (const target of interaction.targets) {
			requiredScaleX = Math.max(
				requiredScaleX,
				target.startRect.width > 1e-6 ? minWidthPx / target.startRect.width : 1
			);
			requiredScaleY = Math.max(
				requiredScaleY,
				target.startRect.height > 1e-6 ? minHeightPx / target.startRect.height : 1
			);
		}
		const groupMinWidthPx = Math.max(minWidthPx, startRect.width * Math.max(requiredScaleX, 1e-6));
		const groupMinHeightPx = Math.max(
			minHeightPx,
			startRect.height * Math.max(requiredScaleY, 1e-6)
		);
		let nextLeftPx = startRect.left;
		let nextTopPx = startRect.top;
		let nextWidthPx = startRect.width;
		let nextHeightPx = startRect.height;

		if (resizeEdges.left) {
			const appliedDeltaX = Math.min(deltaXPx, nextWidthPx - groupMinWidthPx);
			nextLeftPx += appliedDeltaX;
			nextWidthPx -= appliedDeltaX;
		}
		if (resizeEdges.right) {
			nextWidthPx = Math.max(groupMinWidthPx, nextWidthPx + deltaXPx);
		}
		if (resizeEdges.top) {
			const appliedDeltaY = Math.min(deltaYPx, nextHeightPx - groupMinHeightPx);
			nextTopPx += appliedDeltaY;
			nextHeightPx -= appliedDeltaY;
		}
		if (resizeEdges.bottom) {
			nextHeightPx = Math.max(groupMinHeightPx, nextHeightPx + deltaYPx);
		}
		if (snapPx > 0) {
			nextLeftPx = snapToGridPx(nextLeftPx, snapPx);
			nextTopPx = snapToGridPx(nextTopPx, snapPx);
			nextWidthPx = Math.max(groupMinWidthPx, snapToGridPx(nextWidthPx, snapPx));
			nextHeightPx = Math.max(groupMinHeightPx, snapToGridPx(nextHeightPx, snapPx));
		}
		const scaleX = startRect.width > 1e-6 ? nextWidthPx / startRect.width : 1;
		const scaleY = startRect.height > 1e-6 ? nextHeightPx / startRect.height : 1;
		return {
			primaryNodeId: interaction.primaryNodeId,
			previews: interaction.targets.map((target) => {
				const nextRect: FreeLayoutRect = {
					left: nextLeftPx + (target.startRect.left - startRect.left) * scaleX,
					top: nextTopPx + (target.startRect.top - startRect.top) * scaleY,
					width: Math.max(minWidthPx, target.startRect.width * scaleX),
					height: Math.max(minHeightPx, target.startRect.height * scaleY)
				};
				return {
					target,
					preview: freeLayoutPreviewFromRect(
						nextRect,
						target.anchor,
						interaction,
						target.startPreview.position,
						target.startPreview.size
					)
				};
			})
		};
	};

	const sanitizePersistedPageView = (
		candidate: DashboardPersistedPageView | null | undefined
	): DashboardPersistedPageView => {
		const panX = Number(candidate?.panX);
		const panY = Number(candidate?.panY);
		const zoom = Number(candidate?.zoom);
		return {
			panX: Number.isFinite(panX) ? panX : 0,
			panY: Number.isFinite(panY) ? panY : 0,
			zoom: clampPageZoom(Number.isFinite(zoom) && zoom > 0 ? zoom : 1)
		};
	};

	const samePersistedPageView = (
		left: DashboardPersistedPageView | null | undefined,
		right: DashboardPersistedPageView | null | undefined
	): boolean => {
		return (
			(left?.panX ?? 0) === (right?.panX ?? 0) &&
			(left?.panY ?? 0) === (right?.panY ?? 0) &&
			(left?.zoom ?? 1) === (right?.zoom ?? 1)
		);
	};

	const currentPersistedPageView = (): DashboardPersistedPageView => ({
		panX: pageView.panX,
		panY: pageView.panY,
		zoom: clampPageZoom(pageView.zoom)
	});

	const setPageViewInstantly = (candidate: DashboardPersistedPageView): void => {
		const nextPageView = sanitizePersistedPageView(candidate);
		pageView = {
			pointerId: null,
			lastClient: null,
			panX: nextPageView.panX,
			panY: nextPageView.panY,
			zoom: nextPageView.zoom
		};
	};

	const cancelPageViewAnimation = (): void => {
		if (pageViewAnimationFrame !== null && typeof window !== 'undefined') {
			window.cancelAnimationFrame(pageViewAnimationFrame);
		}
		pageViewAnimationFrame = null;
		pageViewAnimationTarget = null;
	};

	const animatePageViewTo = (candidate: DashboardPersistedPageView): void => {
		const targetPageView = sanitizePersistedPageView(candidate);
		const currentPageView = currentPersistedPageView();
		cancelPageViewAnimation();
		if (samePersistedPageView(currentPageView, targetPageView) || typeof window === 'undefined') {
			setPageViewInstantly(targetPageView);
			return;
		}
		const animationDurationMs = 220;
		const startedAtMs = typeof performance !== 'undefined' ? performance.now() : Date.now();
		const interpolate = (start: number, end: number, progress: number): number =>
			start + (end - start) * progress;
		pageViewAnimationTarget = targetPageView;
		const step = (nowMs: number): void => {
			const elapsedMs = Math.max(0, nowMs - startedAtMs);
			const progress = Math.min(1, elapsedMs / animationDurationMs);
			const easedProgress = 1 - Math.pow(1 - progress, 3);
			setPageViewInstantly({
				panX: interpolate(currentPageView.panX, targetPageView.panX, easedProgress),
				panY: interpolate(currentPageView.panY, targetPageView.panY, easedProgress),
				zoom: interpolate(currentPageView.zoom, targetPageView.zoom, easedProgress)
			});
			if (progress >= 1) {
				pageViewAnimationFrame = null;
				pageViewAnimationTarget = null;
				setPageViewInstantly(targetPageView);
				return;
			}
			pageViewAnimationFrame = window.requestAnimationFrame(step);
		};
		pageViewAnimationFrame = window.requestAnimationFrame(step);
	};

	const applyPageView = (
		candidate: DashboardPersistedPageView,
		options: { animate?: boolean } = {}
	): void => {
		if (options.animate === false) {
			cancelPageViewAnimation();
			setPageViewInstantly(candidate);
			return;
		}
		animatePageViewTo(candidate);
	};

	const homePageView = (options: { animate?: boolean } = {}): void => {
		const geometry = getPageFrameGeometry();
		if (!geometry) {
			applyPageView(
				{
					panX: 0,
					panY: 0,
					zoom: 1
				},
				options
			);
			return;
		}
		applyPageView(
			{
				panX: geometry.viewportCenterX - geometry.baseCenterX,
				panY: geometry.viewportCenterY - geometry.baseCenterY,
				zoom: 1
			},
			options
		);
	};

	const frameWidgetElements = (
		widgetElements: HTMLElement[],
		options: { animate?: boolean; fillRatio?: number } = {}
	): boolean => {
		const geometry = getPageFrameGeometry();
		if (!geometry) {
			return false;
		}
		if (widgetElements.length === 0) {
			homePageView(options);
			return true;
		}
		const fillRatio = Math.min(Math.max(options.fillRatio ?? 0.84, 0.1), 0.98);
		const { viewportRect, frameCenterX, frameCenterY, baseCenterX, baseCenterY, currentScale } =
			geometry;
		const availableWidth = Math.max(1, viewportRect.width - 2 * pageEditBleedPx);
		const availableHeight = Math.max(1, viewportRect.height - 2 * pageEditBleedPx);
		let localLeft = Number.POSITIVE_INFINITY;
		let localRight = Number.NEGATIVE_INFINITY;
		let localTop = Number.POSITIVE_INFINITY;
		let localBottom = Number.NEGATIVE_INFINITY;
		for (const widgetElement of widgetElements) {
			const widgetRect = widgetElement.getBoundingClientRect();
			localLeft = Math.min(localLeft, (widgetRect.left - frameCenterX) / currentScale);
			localRight = Math.max(localRight, (widgetRect.right - frameCenterX) / currentScale);
			localTop = Math.min(localTop, (widgetRect.top - frameCenterY) / currentScale);
			localBottom = Math.max(localBottom, (widgetRect.bottom - frameCenterY) / currentScale);
		}
		const localWidth = Math.max(1, localRight - localLeft);
		const localHeight = Math.max(1, localBottom - localTop);
		const targetScale = Math.min(
			(availableWidth * fillRatio) / localWidth,
			(availableHeight * fillRatio) / localHeight
		);
		const nextZoom = clampPageZoom(targetScale / Math.max(pageFitScale, 1e-6));
		const nextScale = Math.max(pageFitScale * nextZoom, 1e-6);
		const localCenterX = (localLeft + localRight) * 0.5;
		const localCenterY = (localTop + localBottom) * 0.5;
		applyPageView(
			{
				zoom: nextZoom,
				panX: geometry.viewportCenterX - baseCenterX - localCenterX * nextScale,
				panY: geometry.viewportCenterY - baseCenterY - localCenterY * nextScale
			},
			options
		);
		return true;
	};

	const getPageFrameGeometry = (): {
		viewportRect: DOMRect;
		frameRect: DOMRect;
		viewportCenterX: number;
		viewportCenterY: number;
		frameCenterX: number;
		frameCenterY: number;
		baseCenterX: number;
		baseCenterY: number;
		currentScale: number;
	} | null => {
		if (!pageViewportElement || !pageFrameElement) {
			return null;
		}
		const viewportRect = pageViewportElement.getBoundingClientRect();
		const frameRect = pageFrameElement.getBoundingClientRect();
		const frameCenterX = frameRect.left + frameRect.width * 0.5;
		const frameCenterY = frameRect.top + frameRect.height * 0.5;
		const currentScaleX = Math.max(frameRect.width / Math.max(pageLogicalWidthPx, 1), 1e-6);
		const currentScaleY = Math.max(frameRect.height / Math.max(pageLogicalHeightPx, 1), 1e-6);
		const currentScale = Math.max(Math.min(currentScaleX, currentScaleY), 1e-6);
		return {
			viewportRect,
			frameRect,
			viewportCenterX: viewportRect.left + viewportRect.width * 0.5,
			viewportCenterY: viewportRect.top + viewportRect.height * 0.5,
			frameCenterX,
			frameCenterY,
			baseCenterX: frameCenterX - pageView.panX,
			baseCenterY: frameCenterY - pageView.panY,
			currentScale
		};
	};

	$effect(() => {
		if (layoutKind !== 'tabs') {
			return;
		}
		if (childWidgets.length === 0) {
			activeTabNodeId = null;
			return;
		}
		if (
			activeTabNodeId !== null &&
			childWidgets.some((candidate) => candidate.node_id === activeTabNodeId)
		) {
			return;
		}
		activeTabNodeId = childWidgets[0]?.node_id ?? null;
	});

	$effect(() => {
		if (layoutKind !== 'accordion') {
			return;
		}
		const availableIds = new Set(childWidgets.map((candidate) => candidate.node_id));
		let nextOpenAccordionNodeIds = openAccordionNodeIds.filter((nodeId) =>
			availableIds.has(nodeId)
		);
		if (nextOpenAccordionNodeIds.length === 0 && childWidgets.length > 0) {
			nextOpenAccordionNodeIds = [childWidgets[0].node_id];
		}
		if (sameNodeIdList(openAccordionNodeIds, nextOpenAccordionNodeIds)) {
			return;
		}
		openAccordionNodeIds = nextOpenAccordionNodeIds;
	});

	$effect(() => {
		void liveNode.node_id;
		pendingPersistedPageView = null;
		appliedPersistedPageView = null;
	});

	$effect(() => {
		if (!isPage) {
			return;
		}
		const nextPageView = sanitizePersistedPageView(persistedPageView);
		if (appliedPersistedPageView && samePersistedPageView(nextPageView, appliedPersistedPageView)) {
			return;
		}
		appliedPersistedPageView = nextPageView;
		if (pendingPersistedPageView && samePersistedPageView(nextPageView, pendingPersistedPageView)) {
			pendingPersistedPageView = null;
			return;
		}
		if (pageView.pointerId !== null || pendingPersistedPageView || pageViewAnimationTarget) {
			return;
		}
		cancelPageViewAnimation();
		setPageViewInstantly(nextPageView);
	});

	$effect(() => {
		if (
			!isPage ||
			!onPersistedPageViewChange ||
			pageView.pointerId !== null ||
			pageViewAnimationTarget
		) {
			return;
		}
		const nextPageView = currentPersistedPageView();
		if (samePersistedPageView(nextPageView, sanitizePersistedPageView(persistedPageView))) {
			pendingPersistedPageView = null;
			return;
		}
		if (pendingPersistedPageView && samePersistedPageView(nextPageView, pendingPersistedPageView)) {
			return;
		}
		pendingPersistedPageView = nextPageView;
		onPersistedPageViewChange(liveNode.node_id, nextPageView);
	});

	$effect(() => {
		if (!pageViewportElement || typeof ResizeObserver === 'undefined') {
			return;
		}
		const observer = new ResizeObserver((entries) => {
			const entry = entries[0];
			if (!entry) {
				return;
			}
			const nextWidth = Math.max(entry.contentRect.width, 1);
			const nextHeight = Math.max(entry.contentRect.height, 1);
			if (pageViewportResizeFrame !== null && typeof window !== 'undefined') {
				window.cancelAnimationFrame(pageViewportResizeFrame);
			}
			pageViewportResizeFrame =
				typeof window === 'undefined'
					? null
					: window.requestAnimationFrame(() => {
							pageViewportResizeFrame = null;
							if (pageViewportSize.width === nextWidth && pageViewportSize.height === nextHeight) {
								return;
							}
							pageViewportSize = {
								width: nextWidth,
								height: nextHeight
							};
						});
		});
		observer.observe(pageViewportElement);
		return () => {
			if (pageViewportResizeFrame !== null && typeof window !== 'undefined') {
				window.cancelAnimationFrame(pageViewportResizeFrame);
				pageViewportResizeFrame = null;
			}
			observer.disconnect();
		};
	});

	$effect(() => {
		if (!isPage) {
			previousPageFitScale = null;
			previousPageFitScaleEditMode = null;
			return;
		}
		const nextFitScale = pageFitScale;
		if (previousPageFitScale === null || previousPageFitScaleEditMode === null) {
			previousPageFitScale = nextFitScale;
			previousPageFitScaleEditMode = editMode;
			return;
		}
		if (pageSize.enabled && previousPageFitScaleEditMode !== editMode && nextFitScale > 1e-6) {
			const preservedEffectiveScale = previousPageFitScale * pageView.zoom;
			const nextZoom = clampPageZoom(preservedEffectiveScale / nextFitScale);
			if (Math.abs(nextZoom - pageView.zoom) > 1e-9) {
				cancelPageViewAnimation();
				pageView = {
					...pageView,
					zoom: nextZoom
				};
			}
		}
		previousPageFitScale = nextFitScale;
		previousPageFitScaleEditMode = editMode;
	});

	$effect(() => {
		if (!isPage) {
			return;
		}
		if (pageViewportWidthPx <= 1 || pageViewportHeightPx <= 1) {
			return;
		}
		const hasPersistedPageView = persistedPageView !== null && persistedPageView !== undefined;
		if (pageNavigationEnabled && hasPersistedPageView) {
			return;
		}
		void liveNode.node_id;
		void pageFitScale;
		void pageLogicalWidthPx;
		void pageLogicalHeightPx;
		if (typeof window === 'undefined') {
			homePageView({ animate: false });
			return;
		}
		const frameRequest = window.requestAnimationFrame(() => {
			homePageView({ animate: false });
		});
		return () => {
			window.cancelAnimationFrame(frameRequest);
		};
	});

	const selectWidgetNode = (selectionMode: SelectionMode = 'REPLACE'): void => {
		if (!editMode) {
			return;
		}
		session?.selectNode(liveNode.node_id, selectionMode);
	};

	const selectSurfaceNode = (selectionMode: SelectionMode = 'REPLACE'): void => {
		if (!editMode || !isLayoutSurface) {
			return;
		}
		session?.selectNode(liveNode.node_id, selectionMode);
	};

	const beginSurfacePress = (event: PointerEvent, surfaceElement: HTMLElement): void => {
		const selectionMode = selectionModeFromEvent(event);
		const baseSelection = [...(session?.selectedNodesIds ?? [])];
		event.preventDefault();
		event.stopPropagation();
		selectSurfaceNode(selectionMode);
		surfacePress = {
			pointerId: event.pointerId,
			surfaceNodeId: liveNode.node_id,
			surfaceElement,
			startClient: [event.clientX, event.clientY],
			baseSelection,
			selectionMode
		};
	};

	const selectWidgetNodeFromKeyboard = (event: KeyboardEvent): void => {
		if (event.key !== 'Enter' && event.key !== ' ') {
			return;
		}
		event.preventDefault();
		selectWidgetNode(selectionModeFromEvent(event));
	};

	const resolveDraggedNode = (payload: DashboardDragPayload | null): UiNodeDto | null => {
		if (!payload) {
			return null;
		}
		return getGraph()?.nodesById.get(payload.nodeId) ?? null;
	};

	const resolveDraggedParameterNode = (payload: DashboardDragPayload | null): UiNodeDto | null => {
		const candidate = resolveDraggedNode(payload);
		if (!candidate || candidate.data.kind !== 'parameter') {
			return null;
		}
		return candidate;
	};

	const canCreateFromDrop = (payload: DashboardDragPayload | null): boolean => {
		if (!editMode) {
			return false;
		}
		if (!isLayoutSurface) {
			return false;
		}
		if (!payload) {
			return false;
		}
		if (payload.kind === 'node') {
			return resolveDraggedNode(payload) !== null;
		}
		return resolveDraggedParameterNode(payload) !== null;
	};

	const canBindNodeWidget = (payload: DashboardDragPayload | null): boolean => {
		return editMode && resolveDraggedNode(payload) !== null;
	};

	const canBindGenericWidget = (payload: DashboardDragPayload | null): boolean => {
		return editMode && resolveDraggedParameterNode(payload) !== null;
	};

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

	const widgetLabelPlacementClass = (labelPlacement: DashboardLabelPlacement | null): string => {
		if (labelPlacement === null) {
			return 'label-hidden';
		}
		return `label-${labelPlacement}`;
	};

	const buildSurfaceDropPreview = (
		event: DragEvent,
		payload: DashboardDragPayload | null
	): SurfaceDropPreview | null => {
		if (!payload) {
			return null;
		}
		const targetNode = resolveDraggedNode(payload);
		if (!targetNode) {
			return null;
		}
		const surfaceElement = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
		const layoutElement = getDirectDashboardLayoutElement(surfaceElement);
		const metricsElement =
			layoutKind === 'free' ? layoutElement : (layoutElement ?? surfaceElement);
		if (!metricsElement) {
			return null;
		}
		const rootRemPx = getRootRemPixels();
		const metrics = getElementSurfaceMetrics(metricsElement, rootRemPx);
		const context = {
			rootRemPx,
			surfaceWidthPx: metrics.width,
			surfaceHeightPx: metrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const insertionTarget =
			layoutKind === 'free' || !surfaceElement
				? null
				: resolveSurfaceInsertionTarget(
						liveNode,
						surfaceElement,
						layoutKind,
						event.clientX,
						event.clientY,
						new Set<NodeId>()
					);
		const placementOptions =
			layoutKind === 'free'
				? {
						centerPx: (() => {
							const rect = metricsElement.getBoundingClientRect();
							return {
								x: (event.clientX - rect.left) / Math.max(metrics.scaleX, 1e-6),
								y: (event.clientY - rect.top) / Math.max(metrics.scaleY, 1e-6)
							};
						})(),
						snapPx: surfaceSnapGrid.enabled ? surfaceSnapGrid.step * rootRemPx : 0
					}
				: undefined;
		return {
			label: targetNode.meta.label,
			targetKind: 'node',
			placement: getDashboardNodeWidgetCreationDefaults(targetNode, context, placementOptions),
			targetIndex: insertionTarget?.targetIndex ?? null,
			genericWidgetKind: null,
			previewText: null,
			previewPlaceholder: null,
			multiline: false
		};
	};

	const handleSurfaceDragEnter = (event: DragEvent): void => {
		const payload = readDashboardDragPayload(event);
		if (!canCreateFromDrop(payload)) {
			clearSurfaceDropState();
			return;
		}
		if (!currentSurfaceOwnsDragEvent(event)) {
			clearSurfaceDropState();
			return;
		}
		event.preventDefault();
		surfaceDragDepth += 1;
		const preview = buildSurfaceDropPreview(event, payload);
		surfaceDropPreview = preview;
		if (event.currentTarget instanceof HTMLElement && layoutKind !== 'free') {
			const insertionTarget = resolveSurfaceInsertionTarget(
				liveNode,
				event.currentTarget,
				layoutKind,
				event.clientX,
				event.clientY,
				new Set<NodeId>()
			);
			setSurfaceInsertionIndicator(event.currentTarget, insertionTarget?.indicator ?? null);
		}
	};

	const handleSurfaceDragOver = (event: DragEvent): void => {
		const payload = readDashboardDragPayload(event);
		if (!canCreateFromDrop(payload)) {
			clearSurfaceDropState();
			return;
		}
		if (!currentSurfaceOwnsDragEvent(event)) {
			clearSurfaceDropState();
			return;
		}
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'copy';
		}
		const preview = buildSurfaceDropPreview(event, payload);
		surfaceDropPreview = preview;
		if (event.currentTarget instanceof HTMLElement && layoutKind !== 'free') {
			const insertionTarget = resolveSurfaceInsertionTarget(
				liveNode,
				event.currentTarget,
				layoutKind,
				event.clientX,
				event.clientY,
				new Set<NodeId>()
			);
			setSurfaceInsertionIndicator(event.currentTarget, insertionTarget?.indicator ?? null);
		}
	};

	const handleSurfaceDragLeave = (event: DragEvent): void => {
		if (!currentSurfaceOwnsDragEvent(event)) {
			clearSurfaceDropState();
			return;
		}
		const nextDepth = Math.max(0, surfaceDragDepth - 1);
		surfaceDragDepth = nextDepth;
		if (nextDepth === 0) {
			surfaceDropPreview = null;
			clearSurfaceInsertionIndicator();
		}
	};

	const handleSurfaceDrop = async (event: DragEvent): Promise<void> => {
		const payload = readDashboardDragPayload(event);
		if (!currentSurfaceOwnsDragEvent(event)) {
			clearSurfaceDropState();
			return;
		}
		const dropPreview = surfaceDropPreview ?? buildSurfaceDropPreview(event, payload);
		clearSurfaceDropState();
		if (!canCreateFromDrop(payload)) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		const targetNode = resolveDraggedNode(payload);
		if (!targetNode) {
			return;
		}
		pendingSurfaceDrop = dropPreview
			? {
					preview: dropPreview,
					knownChildIds: childWidgets.map((child) => child.node_id),
					knownDirectChildIds: [...liveNode.children]
				}
			: null;
		const created = await createDashboardNodeWidget(getGraph, liveNode.node_id, targetNode, {
			placement: dropPreview?.placement,
			targetIndex: dropPreview?.targetIndex ?? undefined
		});
		if (!created) {
			pendingSurfaceDrop = null;
		}
	};

	const handleBindingDragEnter = (
		event: DragEvent,
		predicate: (payload: DashboardDragPayload | null) => boolean
	): void => {
		const payload = readDashboardDragPayload(event);
		if (!predicate(payload)) {
			return;
		}
		event.preventDefault();
		bindingDragDepth += 1;
	};

	const handleBindingDragOver = (
		event: DragEvent,
		predicate: (payload: DashboardDragPayload | null) => boolean
	): void => {
		const payload = readDashboardDragPayload(event);
		if (!predicate(payload)) {
			return;
		}
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'copy';
		}
	};

	const handleBindingDragLeave = (): void => {
		bindingDragDepth = Math.max(0, bindingDragDepth - 1);
	};

	const handleNodeWidgetBindingDrop = async (event: DragEvent): Promise<void> => {
		const payload = readDashboardDragPayload(event);
		bindingDragDepth = 0;
		if (!canBindNodeWidget(payload)) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		const targetNode = resolveDraggedNode(payload);
		if (!targetNode) {
			return;
		}
		await bindDashboardNodeWidgetTarget(getGraph, liveNode.node_id, targetNode);
	};

	const handleGenericWidgetBindingDrop = async (event: DragEvent): Promise<void> => {
		const payload = readDashboardDragPayload(event);
		bindingDragDepth = 0;
		if (!canBindGenericWidget(payload)) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		const targetNode = resolveDraggedParameterNode(payload);
		if (!targetNode) {
			return;
		}
		await bindDashboardGenericWidgetTarget(getGraph, liveNode.node_id, targetNode);
	};

	const sendCssParamValue = async (
		paramNodeId: NodeId | null,
		behaviour: ParamEventBehaviour | null,
		value: CssValueData
	): Promise<void> => {
		if (paramNodeId === null || behaviour === null) {
			return;
		}
		await sendSetParamIntent(
			paramNodeId,
			{ kind: 'css_value', value: value.value, unit: value.unit },
			behaviour
		);
	};

	const withStoppedPropagation = (event: Event): void => {
		event.stopPropagation();
	};

	const closeDashboardContextMenu = (): void => {
		dashboardContextMenu = {
			open: false,
			scope: 'surface',
			x: 0,
			y: 0,
			surfacePlacement: null
		};
	};

	const openDashboardContextMenu = (
		scope: DashboardContextMenuScope,
		x: number,
		y: number,
		surfacePlacement: DashboardWidgetCreationPlacement | null = null
	): void => {
		dashboardContextMenu = {
			open: true,
			scope,
			x,
			y,
			surfacePlacement
		};
	};

	const resolveSurfaceContextPlacement = (
		target: EventTarget | null,
		clientX: number,
		clientY: number
	): DashboardWidgetCreationPlacement | null => {
		if (!(target instanceof HTMLElement)) {
			return null;
		}
		const layoutElement = getDirectDashboardLayoutElement(target) ?? target;
		const rootRemPx = getRootRemPixels();
		const metrics = getElementSurfaceMetrics(layoutElement, rootRemPx);
		const context = {
			rootRemPx,
			surfaceWidthPx: metrics.width,
			surfaceHeightPx: metrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};

		if (layoutKind !== 'free') {
			return getDashboardContainerCreationDefaults(context);
		}

		const rect = layoutElement.getBoundingClientRect();
		const centerPx = {
			x: (clientX - rect.left) / Math.max(metrics.scaleX, 1e-6),
			y: (clientY - rect.top) / Math.max(metrics.scaleY, 1e-6)
		};
		const snapPx = surfaceSnapGrid.enabled ? surfaceSnapGrid.step * rootRemPx : 0;
		return getDashboardContainerCreationDefaults(context, { centerPx, snapPx });
	};

	const handleSurfaceContextMenu = (event: MouseEvent): void => {
		if (!editMode || !isLayoutSurface) {
			return;
		}
		const currentTarget = event.currentTarget;
		if (!(currentTarget instanceof Element)) {
			return;
		}
		const ownerWidgetShell = currentTarget.closest('.dashboard-widget-shell');
		const childWidgetShell =
			event.target instanceof Element ? event.target.closest('.dashboard-widget-shell') : null;
		if (
			(childWidgetShell !== null && childWidgetShell !== ownerWidgetShell) ||
			isInteractiveSurfaceTarget(event.target)
		) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		openDashboardContextMenu(
			'surface',
			event.clientX,
			event.clientY,
			resolveSurfaceContextPlacement(currentTarget, event.clientX, event.clientY)
		);
	};

	const handleWidgetContextMenu = (event: MouseEvent): void => {
		if (!editMode || !isWidget) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		selectWidgetNode('REPLACE');
		openDashboardContextMenu('widget', event.clientX, event.clientY);
	};

	const insertContainerFromContextMenu = async (): Promise<void> => {
		if (!canInsertContainer) {
			return;
		}
		const createdNodeId = await createDashboardContainerWidget(getGraph, liveNode.node_id, {
			placement: dashboardContextMenu.surfacePlacement ?? undefined
		});
		if (createdNodeId !== null) {
			session?.selectNode(createdNodeId);
		}
	};

	const duplicateWidgetFromContextMenu = async (): Promise<void> => {
		if (!canDuplicateWidget || !parentNode) {
			return;
		}
		await duplicateDashboardWidget(getGraph, liveNode.node_id);
	};

	const deleteWidgetFromContextMenu = async (): Promise<void> => {
		if (!canDeleteWidget) {
			return;
		}
		await sendRemoveNodeIntent(liveNode.node_id);
	};

	const wrapWidgetFromContextMenu = async (): Promise<void> => {
		if (!canWrapWidget) {
			return;
		}
		await wrapDashboardWidgetInContainer(getGraph, liveNode.node_id);
	};

	const dashboardContextMenuItems = $derived.by((): ContextMenuItem[] => {
		if (!editMode || !dashboardContextMenu.open) {
			return [];
		}

		if (dashboardContextMenu.scope === 'surface') {
			return [
				{
					id: 'insert-container',
					label: 'Insert Container',
					disabled: !canInsertContainer,
					action: () => {
						void insertContainerFromContextMenu();
					}
				}
			];
		}

		return [
			{
				id: 'wrap-in-container',
				label: 'Wrap In Container',
				disabled: !canWrapWidget,
				action: () => {
					void wrapWidgetFromContextMenu();
				}
			},
			{
				id: 'duplicate-widget',
				label: 'Duplicate',
				disabled: !canDuplicateWidget,
				action: () => {
					void duplicateWidgetFromContextMenu();
				}
			},
			{
				id: 'delete-widget',
				label: 'Delete',
				disabled: !canDeleteWidget,
				color: 'color-mix(in srgb, #ff8b8b 82%, white 18%)',
				hoverColor: 'color-mix(in srgb, #ff5c5c 23%, transparent)',
				action: () => {
					void deleteWidgetFromContextMenu();
				}
			},
			{ separator: true },
			{
				id: 'ordering',
				label: 'Ordering',
				submenu: [
					{
						id: 'bring-forward',
						label: 'Bring Forward',
						disabled: !canBringForward,
						action: () => {
							void moveDashboardWidgetByDelta(getGraph, liveNode.node_id, 1);
						}
					},
					{
						id: 'send-backward',
						label: 'Backward',
						disabled: !canBringBackward,
						action: () => {
							void moveDashboardWidgetByDelta(getGraph, liveNode.node_id, -1);
						}
					},
					{
						id: 'bring-to-front',
						label: 'Bring To Front',
						disabled: !canBringToFront,
						action: () => {
							void moveDashboardWidgetToFront(getGraph, liveNode.node_id);
						}
					},
					{
						id: 'send-to-back',
						label: 'Bring To Back',
						disabled: !canBringToBack,
						action: () => {
							void moveDashboardWidgetToBack(getGraph, liveNode.node_id);
						}
					}
				]
			}
		];
	});

	const dashboardContextMenuAnchor = $derived.by((): ContextMenuAnchor | null => {
		if (!dashboardContextMenu.open) {
			return null;
		}
		return {
			kind: 'point',
			x: dashboardContextMenu.x,
			y: dashboardContextMenu.y
		};
	});

	$effect(() => {
		if (editMode || !dashboardContextMenu.open) {
			return;
		}
		closeDashboardContextMenu();
	});

	$effect(() => {
		if (!dashboardContextMenu.open || dashboardContextMenuItems.length > 0) {
			return;
		}
		closeDashboardContextMenu();
	});

	const finishFreeLayoutEditSession = async (): Promise<void> => {
		const sessionToEnd = freeLayoutEditSession;
		freeLayoutEditSession = null;
		await sessionToEnd?.end();
	};

	const sendFreeLayoutPreview = async (previewSet: FreeLayoutPreviewSet): Promise<void> => {
		const operations: Promise<unknown>[] = [];
		for (const entry of previewSet.previews) {
			const compareTo =
				lastCommittedFreeLayoutPreview?.previews.find(
					(candidate) => candidate.target.nodeId === entry.target.nodeId
				)?.preview ?? entry.target.startPreview;
			if (!samePosition(entry.preview.position, compareTo.position)) {
				if (
					entry.target.positionParamNodeId !== null &&
					entry.target.positionParamBehaviour !== null
				) {
					operations.push(
						sendSetParamIntent(
							entry.target.positionParamNodeId,
							{
								kind: 'vec2',
								value: [entry.preview.position.x, entry.preview.position.y]
							},
							entry.target.positionParamBehaviour
						)
					);
				}
			}
			if (!sameCssValue(entry.preview.size.width, compareTo.size.width)) {
				operations.push(
					sendCssParamValue(
						entry.target.widthParamNodeId,
						entry.target.widthParamBehaviour,
						entry.preview.size.width
					)
				);
			}
			if (!sameCssValue(entry.preview.size.height, compareTo.size.height)) {
				operations.push(
					sendCssParamValue(
						entry.target.heightParamNodeId,
						entry.target.heightParamBehaviour,
						entry.preview.size.height
					)
				);
			}
		}
		if (operations.length === 0) {
			lastCommittedFreeLayoutPreview = cloneFreeLayoutPreviewSet(previewSet);
			return;
		}
		await Promise.all(operations);
		lastCommittedFreeLayoutPreview = cloneFreeLayoutPreviewSet(previewSet);
	};

	const flushFreeLayoutCommitQueue = async (): Promise<void> => {
		if (freeLayoutCommitInFlight) {
			return;
		}
		freeLayoutCommitInFlight = true;
		try {
			while (pendingFreeLayoutCommit) {
				const nextPreview = cloneFreeLayoutPreviewSet(pendingFreeLayoutCommit);
				pendingFreeLayoutCommit = null;
				await sendFreeLayoutPreview(nextPreview);
			}
		} finally {
			freeLayoutCommitInFlight = false;
		}
	};

	const scheduleFreeLayoutCommit = (preview: FreeLayoutPreviewSet): void => {
		pendingFreeLayoutCommit = cloneFreeLayoutPreviewSet(preview);
		if (typeof window === 'undefined') {
			void flushFreeLayoutCommitQueue();
			return;
		}
		if (freeLayoutCommitFrame !== null) {
			return;
		}
		freeLayoutCommitFrame = window.requestAnimationFrame(() => {
			freeLayoutCommitFrame = null;
			void flushFreeLayoutCommitQueue();
		});
	};

	const finalizeFreeLayoutInteraction = async (
		preview: FreeLayoutPreviewSet | null,
		mode: FreeLayoutInteractionMode | null
	): Promise<void> => {
		const dropTarget = freeLayoutDropTarget;
		clearFreeLayoutInteractionChrome();
		if (dropTarget && mode === 'move') {
			cancelScheduledFreeLayoutCommit();
			await moveDashboardWidgetToSurface(
				getGraph,
				liveNode.node_id,
				dropTarget.surfaceNodeId,
				dropTarget.placement,
				dropTarget.targetIndex ?? undefined
			);
			await finishFreeLayoutEditSession();
			return;
		}
		if (preview) {
			scheduleFreeLayoutCommit(preview);
			await flushFreeLayoutCommitQueue();
		}
		await finishFreeLayoutEditSession();
	};

	const beginFreeLayoutMove = (event: PointerEvent, transformNodes: UiNodeDto[]): void => {
		if (!editMode || !supportsFreePlacement) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		selectWidgetNode('REPLACE');
		clearFreeLayoutInteractionChrome();
		setFreeLayoutAncestorShells(event.currentTarget);
		freeLayoutEditSession = createUiEditSession('Move dashboard widget', 'dashboard-free-layout');
		void freeLayoutEditSession.begin();
		const rootRemPx = getRootRemPixels();
		const surfaceMetrics = getClosestSurfaceMetrics(event.currentTarget, rootRemPx);
		const interactionMetrics = {
			rootRemPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const targets = buildFreeLayoutTargets(transformNodes, interactionMetrics);
		const selectionRect = buildSelectionRect(targets);
		if (targets.length === 0 || !selectionRect) {
			clearFreeLayoutInteractionChrome();
			void finishFreeLayoutEditSession();
			return;
		}
		lastCommittedFreeLayoutPreview = buildPreviewSetFromTargets(liveNode.node_id, targets);
		pendingFreeLayoutCommit = null;
		freeLayoutInteraction = {
			pointerId: event.pointerId,
			mode: 'move',
			resizeEdges: null,
			primaryNodeId: liveNode.node_id,
			sourceSurfaceNodeId: parentNode?.node_id ?? liveNode.node_id,
			selectionRect,
			targets,
			startClient: [event.clientX, event.clientY],
			rootRemPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			surfaceScaleX: surfaceMetrics.scaleX,
			surfaceScaleY: surfaceMetrics.scaleY,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		freeLayoutPreview = buildPreviewSetFromTargets(liveNode.node_id, targets);
	};

	const beginFreeLayoutResize = (
		event: PointerEvent,
		resizeEdges: FreeLayoutResizeEdges,
		transformNodes: UiNodeDto[]
	): void => {
		if (!editMode || !supportsFreePlacement) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		selectWidgetNode('REPLACE');
		clearFreeLayoutInteractionChrome();
		setFreeLayoutAncestorShells(event.currentTarget);
		freeLayoutEditSession = createUiEditSession('Resize dashboard widget', 'dashboard-free-layout');
		void freeLayoutEditSession.begin();
		const rootRemPx = getRootRemPixels();
		const surfaceMetrics = getClosestSurfaceMetrics(event.currentTarget, rootRemPx);
		const interactionMetrics = {
			rootRemPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const targets = buildFreeLayoutTargets(transformNodes, interactionMetrics);
		const selectionRect = buildSelectionRect(targets);
		if (targets.length === 0 || !selectionRect) {
			clearFreeLayoutInteractionChrome();
			void finishFreeLayoutEditSession();
			return;
		}
		lastCommittedFreeLayoutPreview = buildPreviewSetFromTargets(liveNode.node_id, targets);
		pendingFreeLayoutCommit = null;
		freeLayoutInteraction = {
			pointerId: event.pointerId,
			mode: 'resize',
			resizeEdges,
			primaryNodeId: liveNode.node_id,
			sourceSurfaceNodeId: parentNode?.node_id ?? liveNode.node_id,
			selectionRect,
			targets,
			startClient: [event.clientX, event.clientY],
			rootRemPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			surfaceScaleX: surfaceMetrics.scaleX,
			surfaceScaleY: surfaceMetrics.scaleY,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		freeLayoutPreview = buildPreviewSetFromTargets(liveNode.node_id, targets);
	};

	const beginWidgetMoveInteraction = (event: PointerEvent): void => {
		if (!editMode || !supportsOrdering) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		clearFreeLayoutInteractionChrome();
		setFreeLayoutAncestorShells(event.currentTarget);
		widgetMoveInteraction = {
			pointerId: event.pointerId,
			sourceSurfaceNodeId: parentNode?.node_id ?? liveNode.node_id,
			startClient: [event.clientX, event.clientY]
		};
	};

	const beginConstrainedResize = (
		event: PointerEvent,
		resizeEdges: FreeLayoutResizeEdges
	): void => {
		if (!editMode || !supportsStackSizing) {
			return;
		}
		const axis = parentLayoutKind === 'horizontal' ? 'x' : 'y';
		const isLeadingEdge = axis === 'x' ? resizeEdges.left : resizeEdges.top;
		const declId = axis === 'x' ? 'width' : 'height';
		const paramNode = getDirectParamNode(graph, liveNode, declId);
		if (paramNode?.data.kind !== 'parameter') {
			return;
		}

		event.preventDefault();
		event.stopPropagation();
		void sendPatchMetaIntent(paramNode.node_id, { enabled: true });

		const rootRemPx = getRootRemPixels();
		const surfaceMetrics = getClosestSurfaceMetrics(event.currentTarget, rootRemPx);
		const sizeContext = {
			rootRemPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const startSize = axis === 'x' ? placement.size.width : placement.size.height;
		const startSizePx = Math.max(
			(axis === 'x' ? minFreeWidgetWidthRem : minFreeWidgetHeightRem) * rootRemPx,
			cssValueToPx(startSize, axis, createCssUnitContext(sizeContext, axis))
		);

		constrainedResizeInteraction = {
			pointerId: event.pointerId,
			axis,
			sign: isLeadingEdge ? -1 : 1,
			startClient: [event.clientX, event.clientY],
			paramNodeId: paramNode.node_id,
			paramBehaviour: paramNode.data.param.event_behaviour,
			startSize,
			startSizePx,
			rootRemPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			surfaceScaleX: surfaceMetrics.scaleX,
			surfaceScaleY: surfaceMetrics.scaleY,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx(),
			lastSent: null
		};
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

	const handleSurfacePointerDown = (event: PointerEvent): void => {
		focusClosestPageViewport(event.currentTarget);
		if (!editMode || !isLayoutSurface || event.button !== 0) {
			return;
		}
		if (freeLayoutInteraction || marqueeSelection || surfacePress) {
			return;
		}
		if (!(event.currentTarget instanceof HTMLElement)) {
			return;
		}
		const ownerWidgetShell = event.currentTarget.closest('.dashboard-widget-shell');
		const childWidgetShell =
			event.target instanceof Element ? event.target.closest('.dashboard-widget-shell') : null;
		if (
			(childWidgetShell !== null && childWidgetShell !== ownerWidgetShell) ||
			isInteractiveSurfaceTarget(event.target)
		) {
			return;
		}
		const surfaceElement =
			isPage && event.currentTarget instanceof HTMLElement
				? ((event.currentTarget.closest('.dashboard-page-scene') as HTMLElement | null) ??
					event.currentTarget)
				: event.currentTarget;
		beginSurfacePress(event, surfaceElement);
	};

	const handlePageScenePointerDown = (event: PointerEvent): void => {
		pageViewportElement?.focus();
		if (!isPage || !editMode || event.button !== 0) {
			return;
		}
		if (freeLayoutInteraction || marqueeSelection || surfacePress) {
			return;
		}
		if (!(event.currentTarget instanceof HTMLElement)) {
			return;
		}
		const sceneElement = event.currentTarget.parentElement;
		if (!(sceneElement instanceof HTMLElement)) {
			return;
		}
		beginSurfacePress(event, sceneElement);
	};

	$effect(() => {
		if (!surfacePress) {
			return;
		}

		const handlePointerMove = (event: PointerEvent): void => {
			if (!surfacePress || event.pointerId !== surfacePress.pointerId) {
				return;
			}
			const deltaX = event.clientX - surfacePress.startClient[0];
			const deltaY = event.clientY - surfacePress.startClient[1];
			if (Math.hypot(deltaX, deltaY) < marqueeDragThresholdPx) {
				return;
			}
			event.preventDefault();
			const selection: MarqueeSelectionState = {
				pointerId: surfacePress.pointerId,
				surfaceNodeId: surfacePress.surfaceNodeId,
				surfaceElement: surfacePress.surfaceElement,
				startClient: surfacePress.startClient,
				currentClient: [event.clientX, event.clientY],
				baseSelection: surfacePress.baseSelection,
				selectionMode: surfacePress.selectionMode
			};
			surfacePress = null;
			marqueeSelection = selection;
			applyMarqueeSelection(selection);
		};

		const finishPress = (event: PointerEvent): void => {
			if (!surfacePress || event.pointerId !== surfacePress.pointerId) {
				return;
			}
			surfacePress = null;
		};

		window.addEventListener('pointermove', handlePointerMove);
		window.addEventListener('pointerup', finishPress);
		window.addEventListener('pointercancel', finishPress);

		return () => {
			window.removeEventListener('pointermove', handlePointerMove);
			window.removeEventListener('pointerup', finishPress);
			window.removeEventListener('pointercancel', finishPress);
		};
	});

	$effect(() => {
		if (!freeLayoutInteraction) {
			return;
		}

		const handlePointerMove = (event: PointerEvent): void => {
			if (!freeLayoutInteraction || event.pointerId !== freeLayoutInteraction.pointerId) {
				return;
			}
			event.preventDefault();
			const deltaXPx =
				(event.clientX - freeLayoutInteraction.startClient[0]) /
				Math.max(freeLayoutInteraction.surfaceScaleX, 1e-6);
			const deltaYPx =
				(event.clientY - freeLayoutInteraction.startClient[1]) /
				Math.max(freeLayoutInteraction.surfaceScaleY, 1e-6);
			const minWidthPx = minFreeWidgetWidthRem * freeLayoutInteraction.rootRemPx;
			const minHeightPx = minFreeWidgetHeightRem * freeLayoutInteraction.rootRemPx;
			const snapPx = parentSnapGrid.enabled
				? parentSnapGrid.step * freeLayoutInteraction.rootRemPx
				: 0;
			if (freeLayoutInteraction.mode === 'move') {
				const nextPreview = buildFreeLayoutMovePreview(
					freeLayoutInteraction,
					deltaXPx,
					deltaYPx,
					snapPx
				);
				freeLayoutPreview = nextPreview;
				const nextDropTarget = resolveFreeLayoutDropTargetAtPoint(
					event.clientX,
					event.clientY,
					nextPreview
				);
				freeLayoutDropTarget = nextDropTarget;
				setFreeLayoutHoverSurface(nextDropTarget?.element ?? null);
				setSurfaceInsertionIndicator(
					nextDropTarget?.element ?? null,
					nextDropTarget?.indicator ?? null
				);
				publishWidgetMoveSurfacePreview(nextDropTarget);
				if (nextDropTarget) {
					cancelScheduledFreeLayoutCommit();
					return;
				}
				scheduleFreeLayoutCommit(nextPreview);
				return;
			}
			setFreeLayoutHoverSurface(null);
			freeLayoutDropTarget = null;
			clearSurfaceInsertionIndicator();
			const nextPreview = buildFreeLayoutResizePreview(
				freeLayoutInteraction,
				deltaXPx,
				deltaYPx,
				snapPx
			);
			if (!nextPreview) {
				return;
			}
			freeLayoutPreview = nextPreview;
			scheduleFreeLayoutCommit(nextPreview);
		};

		const finishInteraction = (event: PointerEvent): void => {
			if (!freeLayoutInteraction || event.pointerId !== freeLayoutInteraction.pointerId) {
				return;
			}
			const preview = freeLayoutPreview;
			const mode = freeLayoutInteraction.mode;
			freeLayoutInteraction = null;
			freeLayoutPreview = null;
			void finalizeFreeLayoutInteraction(preview, mode);
		};

		window.addEventListener('pointermove', handlePointerMove);
		window.addEventListener('pointerup', finishInteraction);
		window.addEventListener('pointercancel', finishInteraction);

		const previousCursor = document.body.style.cursor;
		const previousUserSelect = document.body.style.userSelect;
		document.body.style.cursor =
			freeLayoutInteraction.mode === 'move'
				? 'grabbing'
				: freeLayoutInteraction.resizeEdges?.left && freeLayoutInteraction.resizeEdges?.top
					? 'nwse-resize'
					: freeLayoutInteraction.resizeEdges?.right && freeLayoutInteraction.resizeEdges?.bottom
						? 'nwse-resize'
						: freeLayoutInteraction.resizeEdges?.right && freeLayoutInteraction.resizeEdges?.top
							? 'nesw-resize'
							: freeLayoutInteraction.resizeEdges?.left && freeLayoutInteraction.resizeEdges?.bottom
								? 'nesw-resize'
								: freeLayoutInteraction.resizeEdges?.left ||
									  freeLayoutInteraction.resizeEdges?.right
									? 'ew-resize'
									: 'ns-resize';
		document.body.style.userSelect = 'none';

		return () => {
			window.removeEventListener('pointermove', handlePointerMove);
			window.removeEventListener('pointerup', finishInteraction);
			window.removeEventListener('pointercancel', finishInteraction);
			document.body.style.cursor = previousCursor;
			document.body.style.userSelect = previousUserSelect;
		};
	});

	$effect(() => {
		if (!widgetMoveInteraction) {
			return;
		}

		const handlePointerMove = (event: PointerEvent): void => {
			if (!widgetMoveInteraction || event.pointerId !== widgetMoveInteraction.pointerId) {
				return;
			}
			const deltaX = event.clientX - widgetMoveInteraction.startClient[0];
			const deltaY = event.clientY - widgetMoveInteraction.startClient[1];
			if (Math.hypot(deltaX, deltaY) < marqueeDragThresholdPx) {
				return;
			}
			event.preventDefault();
			const nextDropTarget = resolveWidgetMoveDropTargetAtPoint(event.clientX, event.clientY);
			widgetMoveDropTarget = nextDropTarget;
			setFreeLayoutHoverSurface(nextDropTarget?.element ?? null);
			setSurfaceInsertionIndicator(
				nextDropTarget?.element ?? null,
				nextDropTarget?.indicator ?? null
			);
			publishWidgetMoveSurfacePreview(nextDropTarget);
		};

		const finishInteraction = (event: PointerEvent): void => {
			if (!widgetMoveInteraction || event.pointerId !== widgetMoveInteraction.pointerId) {
				return;
			}
			const dropTarget = widgetMoveDropTarget;
			const sourceSurfaceNodeId = widgetMoveInteraction.sourceSurfaceNodeId;
			const isNoOpMove =
				dropTarget !== null &&
				dropTarget.surfaceNodeId === sourceSurfaceNodeId &&
				dropTarget.targetIndex === (widgetOrderingState?.currentIndex ?? null);
			if (dropTarget && !isNoOpMove) {
				publishWidgetMoveCommit(dropTarget, sourceSurfaceNodeId);
			}
			widgetMoveInteraction = null;
			clearFreeLayoutInteractionChrome();
			if (!dropTarget || isNoOpMove) {
				if (isNoOpMove) {
					publishWidgetMoveCommit(null);
				}
				return;
			}
			void (async () => {
				const moved = await moveDashboardWidgetToSurface(
					getGraph,
					liveNode.node_id,
					dropTarget.surfaceNodeId,
					dropTarget.placement,
					dropTarget.targetIndex ?? undefined
				);
				if (!moved) {
					publishWidgetMoveCommit(null);
				}
			})();
		};

		window.addEventListener('pointermove', handlePointerMove);
		window.addEventListener('pointerup', finishInteraction);
		window.addEventListener('pointercancel', finishInteraction);

		const previousCursor = document.body.style.cursor;
		const previousUserSelect = document.body.style.userSelect;
		document.body.style.cursor = 'grabbing';
		document.body.style.userSelect = 'none';

		return () => {
			window.removeEventListener('pointermove', handlePointerMove);
			window.removeEventListener('pointerup', finishInteraction);
			window.removeEventListener('pointercancel', finishInteraction);
			document.body.style.cursor = previousCursor;
			document.body.style.userSelect = previousUserSelect;
		};
	});

	$effect(() => {
		if (!constrainedResizeInteraction) {
			return;
		}

		const handlePointerMove = (event: PointerEvent): void => {
			if (
				!constrainedResizeInteraction ||
				event.pointerId !== constrainedResizeInteraction.pointerId
			) {
				return;
			}
			event.preventDefault();
			const axis = constrainedResizeInteraction.axis;
			const scale =
				axis === 'x'
					? Math.max(constrainedResizeInteraction.surfaceScaleX, 1e-6)
					: Math.max(constrainedResizeInteraction.surfaceScaleY, 1e-6);
			const deltaPx =
				(axis === 'x'
					? event.clientX - constrainedResizeInteraction.startClient[0]
					: event.clientY - constrainedResizeInteraction.startClient[1]) / scale;
			const nextSizePx = Math.max(
				(axis === 'x' ? minFreeWidgetWidthRem : minFreeWidgetHeightRem) *
					constrainedResizeInteraction.rootRemPx,
				constrainedResizeInteraction.startSizePx + constrainedResizeInteraction.sign * deltaPx
			);
			const nextSize = pxToCssValue(
				nextSizePx,
				constrainedResizeInteraction.startSize,
				axis,
				createCssUnitContext(constrainedResizeInteraction, axis)
			);
			if (
				constrainedResizeInteraction.lastSent &&
				sameCssValue(constrainedResizeInteraction.lastSent, nextSize)
			) {
				return;
			}
			constrainedResizeInteraction = {
				...constrainedResizeInteraction,
				lastSent: nextSize
			};
			void sendSetParamIntent(
				constrainedResizeInteraction.paramNodeId,
				{ kind: 'css_value', value: nextSize.value, unit: nextSize.unit },
				constrainedResizeInteraction.paramBehaviour
			);
		};

		const finishResize = (event: PointerEvent): void => {
			if (
				!constrainedResizeInteraction ||
				event.pointerId !== constrainedResizeInteraction.pointerId
			) {
				return;
			}
			constrainedResizeInteraction = null;
		};

		window.addEventListener('pointermove', handlePointerMove);
		window.addEventListener('pointerup', finishResize);
		window.addEventListener('pointercancel', finishResize);

		const previousCursor = document.body.style.cursor;
		const previousUserSelect = document.body.style.userSelect;
		document.body.style.cursor =
			constrainedResizeInteraction.axis === 'x' ? 'ew-resize' : 'ns-resize';
		document.body.style.userSelect = 'none';

		return () => {
			window.removeEventListener('pointermove', handlePointerMove);
			window.removeEventListener('pointerup', finishResize);
			window.removeEventListener('pointercancel', finishResize);
			document.body.style.cursor = previousCursor;
			document.body.style.userSelect = previousUserSelect;
		};
	});

	$effect(() => {
		if (!marqueeSelection) {
			return;
		}

		const handlePointerMove = (event: PointerEvent): void => {
			if (!marqueeSelection || event.pointerId !== marqueeSelection.pointerId) {
				return;
			}
			event.preventDefault();
			const nextSelection = {
				...marqueeSelection,
				currentClient: [event.clientX, event.clientY] as [number, number]
			};
			marqueeSelection = nextSelection;
			applyMarqueeSelection(nextSelection);
		};

		const finishSelection = (event: PointerEvent): void => {
			if (!marqueeSelection || event.pointerId !== marqueeSelection.pointerId) {
				return;
			}
			const completedSelection = {
				...marqueeSelection,
				currentClient: [event.clientX, event.clientY] as [number, number]
			};
			applyMarqueeSelection(completedSelection);
			marqueeSelection = null;
		};

		window.addEventListener('pointermove', handlePointerMove);
		window.addEventListener('pointerup', finishSelection);
		window.addEventListener('pointercancel', finishSelection);

		const previousCursor = document.body.style.cursor;
		const previousUserSelect = document.body.style.userSelect;
		document.body.style.cursor = 'crosshair';
		document.body.style.userSelect = 'none';

		return () => {
			window.removeEventListener('pointermove', handlePointerMove);
			window.removeEventListener('pointerup', finishSelection);
			window.removeEventListener('pointercancel', finishSelection);
			document.body.style.cursor = previousCursor;
			document.body.style.userSelect = previousUserSelect;
		};
	});

	onDestroy(() => {
		if (typeof window !== 'undefined' && freeLayoutCommitFrame !== null) {
			window.cancelAnimationFrame(freeLayoutCommitFrame);
		}
		if (typeof window !== 'undefined' && pageViewportResizeFrame !== null) {
			window.cancelAnimationFrame(pageViewportResizeFrame);
		}
		widgetMoveInteraction = null;
		constrainedResizeInteraction = null;
		clearFreeLayoutInteractionChrome();
		cancelPageViewAnimation();
		void finishFreeLayoutEditSession();
	});

	$effect(() => {
		if (typeof window === 'undefined') {
			return;
		}

		const resetSurfacePreview = (): void => {
			clearSurfaceDropState();
		};

		window.addEventListener('dragend', resetSurfacePreview, true);
		window.addEventListener('drop', resetSurfacePreview, true);

		return () => {
			window.removeEventListener('dragend', resetSurfacePreview, true);
			window.removeEventListener('drop', resetSurfacePreview, true);
		};
	});

	$effect(() => {
		if (typeof window === 'undefined' || !isLayoutSurface) {
			widgetMoveSurfacePreview = null;
			widgetMoveHiddenChildNodeId = null;
			pendingWidgetMoveCommit = null;
			return;
		}
		const currentLayoutKind = layoutKind;

		const handleWidgetMovePreview = (event: Event): void => {
			const detail = (event as CustomEvent<WidgetMovePreviewEventDetail>).detail;
			if (!detail) {
				widgetMoveSurfacePreview = null;
				widgetMoveHiddenChildNodeId = null;
				return;
			}
			widgetMoveSurfacePreview = detail.surfaceNodeId === liveNode.node_id ? detail.preview : null;
			widgetMoveHiddenChildNodeId =
				currentLayoutKind !== 'free' && detail.sourceSurfaceNodeId === liveNode.node_id
					? detail.sourceWidgetNodeId
					: null;
		};

		window.addEventListener(widgetMovePreviewEventName, handleWidgetMovePreview);
		return () => {
			window.removeEventListener(widgetMovePreviewEventName, handleWidgetMovePreview);
		};
	});

	$effect(() => {
		if (typeof window === 'undefined' || !isLayoutSurface) {
			pendingWidgetMoveCommit = null;
			return;
		}
		const currentLayoutKind = layoutKind;

		const handleWidgetMoveCommit = (event: Event): void => {
			const detail = (event as CustomEvent<WidgetMoveCommitEventDetail>).detail;
			if (!detail) {
				pendingWidgetMoveCommit = null;
				return;
			}
			const preview = detail.targetSurfaceNodeId === liveNode.node_id ? detail.preview : null;
			const hiddenChildNodeId =
				currentLayoutKind !== 'free' && detail.sourceSurfaceNodeId === liveNode.node_id
					? detail.sourceWidgetNodeId
					: null;
			if (!preview && hiddenChildNodeId === null) {
				pendingWidgetMoveCommit = null;
				return;
			}
			if (detail.sourceWidgetNodeId === null) {
				pendingWidgetMoveCommit = null;
				return;
			}
			const currentChildIds = childWidgets.map((child) => child.node_id);
			const expectedChildIds =
				detail.targetSurfaceNodeId === liveNode.node_id
					? moveNodeIdInList(
							currentChildIds,
							detail.sourceWidgetNodeId,
							detail.preview?.targetIndex ?? null
						)
					: detail.sourceSurfaceNodeId === liveNode.node_id
						? currentChildIds.filter((childNodeId) => childNodeId !== detail.sourceWidgetNodeId)
						: null;
			if (expectedChildIds === null) {
				pendingWidgetMoveCommit = null;
				return;
			}
			pendingWidgetMoveCommit = {
				preview,
				hiddenChildNodeId,
				expectedChildIds
			};
		};

		window.addEventListener(widgetMoveCommitEventName, handleWidgetMoveCommit);
		return () => {
			window.removeEventListener(widgetMoveCommitEventName, handleWidgetMoveCommit);
		};
	});

	$effect(() => {
		if (pageView.pointerId === null) {
			return;
		}

		const handlePointerMove = (event: PointerEvent): void => {
			if (pageView.pointerId !== event.pointerId || !pageView.lastClient) {
				return;
			}
			event.preventDefault();
			pageView = {
				...pageView,
				lastClient: [event.clientX, event.clientY],
				panX: pageView.panX + (event.clientX - pageView.lastClient[0]),
				panY: pageView.panY + (event.clientY - pageView.lastClient[1])
			};
		};

		const finishPan = (event: PointerEvent): void => {
			if (pageView.pointerId !== event.pointerId) {
				return;
			}
			pageView = {
				...pageView,
				pointerId: null,
				lastClient: null
			};
		};

		window.addEventListener('pointermove', handlePointerMove);
		window.addEventListener('pointerup', finishPan);
		window.addEventListener('pointercancel', finishPan);

		return () => {
			window.removeEventListener('pointermove', handlePointerMove);
			window.removeEventListener('pointerup', finishPan);
			window.removeEventListener('pointercancel', finishPan);
		};
	});

	let supportsFreePlacement = $derived(editMode && parentLayoutKind === 'free');
	let supportsGridPlacement = $derived(editMode && parentLayoutKind === 'grid');
	let supportsStackSizing = $derived(
		editMode && (parentLayoutKind === 'horizontal' || parentLayoutKind === 'vertical')
	);
	let supportsOrdering = $derived(editMode && parentLayoutKind !== 'free');
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
	let pageSize = $derived(getDashboardPageSize(graph, liveNode));
	let parentSnapGrid = $derived(getDashboardSnapGrid(graph, parentNode));
	let surfaceSnapGrid = $derived(getDashboardSnapGrid(graph, liveNode));
	let snapGridVisible = $derived(editMode && surfaceSnapGrid.enabled && layoutKind === 'free');
	let currentNodeFreeLayoutPreview = $derived.by(() =>
		resolveFreeLayoutPreview(freeLayoutPreview, liveNode.node_id)
	);
	let marqueeStyle = $derived.by(() => {
		if (!marqueeSelection) {
			return '';
		}
		const rect = normalizeClientRect(marqueeSelection.startClient, marqueeSelection.currentClient);
		const surfaceRect = marqueeSelection.surfaceElement.getBoundingClientRect();
		const localLeft = rect.left - surfaceRect.left;
		const localTop = rect.top - surfaceRect.top;
		return `left: ${localLeft}px; top: ${localTop}px; inline-size: ${rect.width}px; block-size: ${rect.height}px;`;
	});
	let widgetShellStyle = $derived.by(() => {
		if (!editMode || !supportsFreePlacement || !currentNodeFreeLayoutPreview) {
			return '';
		}
		const context = freeLayoutInteraction ?? {
			rootRemPx: getRootRemPixels(),
			surfaceWidthPx: getViewportWidthPx(),
			surfaceHeightPx: getViewportHeightPx(),
			surfaceScaleX: 1,
			surfaceScaleY: 1,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const currentRect = freeLayoutRectFromPlacement(
			{ position: placement.position, size: placement.size, anchor: placement.anchor },
			context
		);
		const previewRect = freeLayoutRectFromPlacement(
			{
				position: currentNodeFreeLayoutPreview.position,
				size: currentNodeFreeLayoutPreview.size,
				anchor: placement.anchor
			},
			context
		);
		return `transform: translate(${previewRect.left - currentRect.left}px, ${previewRect.top - currentRect.top}px); inline-size: ${previewRect.width}px; block-size: ${previewRect.height}px; z-index: 4;`;
	});

	const freeSurfaceHeightPx = $derived.by(() => {
		if (layoutKind !== 'free' || isPage) {
			return 20 * getRootRemPixels();
		}
		const rootRemPx = getRootRemPixels();
		const surfaceContext = {
			rootRemPx,
			surfaceWidthPx: getViewportWidthPx(),
			surfaceHeightPx: getViewportHeightPx(),
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		let maxExtent = 18 * rootRemPx;
		for (const child of renderedChildWidgets) {
			const childPlacement = getDashboardPlacement(graph, child);
			const childRect = freeLayoutRectFromPlacement(
				childPlacement,
				surfaceContext as FreeLayoutInteraction
			);
			maxExtent = Math.max(maxExtent, childRect.top + childRect.height + 2 * rootRemPx);
		}
		return maxExtent;
	});

	$effect(() => {
		if (!pendingSurfaceDrop) {
			return;
		}
		const knownChildIds = new Set(pendingSurfaceDrop.knownChildIds);
		const knownDirectChildIds = new Set(pendingSurfaceDrop.knownDirectChildIds);
		if (
			liveNode.children.length !== pendingSurfaceDrop.knownDirectChildIds.length ||
			childWidgets.length !== pendingSurfaceDrop.knownChildIds.length ||
			liveNode.children.some((childId) => !knownDirectChildIds.has(childId)) ||
			childWidgets.some((child) => !knownChildIds.has(child.node_id))
		) {
			pendingSurfaceDrop = null;
		}
	});

	$effect(() => {
		if (!pendingWidgetMoveCommit) {
			return;
		}
		const currentChildIds = childWidgets.map((child) => child.node_id);
		if (sameNodeIdList(currentChildIds, pendingWidgetMoveCommit.expectedChildIds)) {
			pendingWidgetMoveCommit = null;
		}
	});

	$effect(() => {
		if (!pendingSurfaceDrop || typeof window === 'undefined') {
			return;
		}
		const timeout = window.setTimeout(() => {
			pendingSurfaceDrop = null;
		}, 2000);
		return () => {
			window.clearTimeout(timeout);
		};
	});

	$effect(() => {
		if (!pendingWidgetMoveCommit || typeof window === 'undefined') {
			return;
		}
		const timeout = window.setTimeout(() => {
			pendingWidgetMoveCommit = null;
		}, 2000);
		return () => {
			window.clearTimeout(timeout);
		};
	});

	const surfaceStyle = $derived.by((): string => {
		const snapStepPx = surfaceSnapGrid.enabled ? surfaceSnapGrid.step * getRootRemPixels() : 0;
		const gapX =
			layoutKind === 'horizontal' || layoutKind === 'grid'
				? gap.x
				: { value: 0, unit: 'rem' as const };
		const gapY =
			layoutKind === 'vertical' || layoutKind === 'grid'
				? gap.y
				: { value: 0, unit: 'rem' as const };
		const gapStyle = `--dashboard-gap-x: ${formatCssValue(gapX)}; --dashboard-gap-y: ${formatCssValue(gapY)}; --dashboard-snap-grid-step: ${snapStepPx}px;`;
		if (layoutKind === 'free') {
			if (isPage) {
				return `${gapStyle} inline-size: 100%; block-size: 100%; min-block-size: 0;`;
			}
			return `${gapStyle};`;
		}
		if (layoutKind === 'grid') {
			return `${gapStyle} --dashboard-grid-flow: ${resolvedGridDimensions.direction}; --dashboard-grid-columns: ${resolvedGridDimensions.columns}; --dashboard-grid-rows: ${resolvedGridDimensions.rows};`;
		}
		return gapStyle;
	});

	const pageViewportWidthPx = $derived(Math.max(pageViewportSize.width, 1));
	const pageViewportHeightPx = $derived(Math.max(pageViewportSize.height, 1));
	const pageEditBleedPx = $derived.by(() => {
		if (!editMode || !pageSize.enabled) {
			return 0;
		}
		const rootRemPx = getRootRemPixels();
		return Math.max(minFreeWidgetWidthRem * rootRemPx, 2 * rootRemPx);
	});
	const pageSceneStyle = $derived.by(() => `padding: ${pageEditBleedPx}px;`);
	const pageLogicalWidthPx = $derived.by(() =>
		pageSize.enabled ? pageSize.widthPx : pageViewportWidthPx
	);
	const pageLogicalHeightPx = $derived.by(() =>
		pageSize.enabled ? pageSize.heightPx : pageViewportHeightPx
	);
	const pageFitScale = $derived.by(() => {
		if (!pageSize.enabled) {
			return 1;
		}
		const availableWidthPx = Math.max(1, pageViewportWidthPx - 2 * pageEditBleedPx);
		const availableHeightPx = Math.max(1, pageViewportHeightPx - 2 * pageEditBleedPx);
		return Math.min(availableWidthPx / pageSize.widthPx, availableHeightPx / pageSize.heightPx);
	});
	const pageEffectiveScale = $derived.by(() =>
		Math.max(1e-6, pageFitScale * clampPageZoom(pageView.zoom))
	);
	const pageFrameStyle = $derived.by(() => {
		return `inline-size: ${pageLogicalWidthPx}px; block-size: ${pageLogicalHeightPx}px; transform: translate(${pageView.panX}px, ${pageView.panY}px) scale(${pageEffectiveScale});`;
	});
	const pageBleedFrameStyle = $derived.by(() => {
		return `inline-size: ${pageLogicalWidthPx}px; block-size: ${pageLogicalHeightPx}px; transform: translate(${pageView.panX}px, ${pageView.panY}px) scale(${pageEffectiveScale}); z-index: 0;`;
	});

	const sumCssLengthExpressions = (expressions: string[]): string => {
		if (expressions.length === 0) {
			return '0px';
		}
		if (expressions.length === 1) {
			return expressions[0];
		}
		return expressions.map((expression) => `(${expression})`).join(' + ');
	};

	const placementHasExplicitSize = (
		axis: 'horizontal' | 'vertical',
		placementLike: DashboardPlacement | DashboardWidgetCreationPlacement
	): boolean => {
		if (!('sizeEnabled' in placementLike) || placementLike.sizeEnabled === undefined) {
			return false;
		}
		return axis === 'horizontal'
			? placementLike.sizeEnabled.width
			: placementLike.sizeEnabled.height;
	};

	const stackAutoBasisExpression = (
		axis: 'horizontal' | 'vertical',
		placements: Array<DashboardPlacement | DashboardWidgetCreationPlacement>
	): string => {
		const sizeSelector =
			axis === 'horizontal'
				? (placementLike: DashboardPlacement | DashboardWidgetCreationPlacement) =>
						formatCssValue(placementLike.size.width)
				: (placementLike: DashboardPlacement | DashboardWidgetCreationPlacement) =>
						formatCssValue(placementLike.size.height);
		const hardSizeExpressions = placements
			.filter((placementLike) => placementHasExplicitSize(axis, placementLike))
			.map(sizeSelector);
		const autoCount = Math.max(
			1,
			placements.filter((placementLike) => !placementHasExplicitSize(axis, placementLike)).length
		);
		const gapExpression = axis === 'horizontal' ? formatCssValue(gap.x) : formatCssValue(gap.y);
		const totalGapExpression =
			placements.length > 1 ? `(${gapExpression} * ${placements.length - 1})` : '0px';
		return `max(calc((100% - ${totalGapExpression} - (${sumCssLengthExpressions(hardSizeExpressions)})) / ${autoCount}), 0px)`;
	};

	const childPlacements = $derived.by(() =>
		renderedChildWidgets.map((child) => getDashboardPlacement(graph, child))
	);
	const stackPlacements = $derived.by(() => {
		const placements: Array<DashboardPlacement | DashboardWidgetCreationPlacement> = [
			...childPlacements
		];
		if (
			rendersInlineSurfaceDropPreview &&
			displayedSurfaceDropPreview &&
			(layoutKind === 'horizontal' || layoutKind === 'vertical')
		) {
			placements.push(displayedSurfaceDropPreview.placement);
		}
		return placements;
	});
	const stackAutoBasis = $derived.by(() => {
		if (layoutKind === 'horizontal') {
			return stackAutoBasisExpression('horizontal', stackPlacements);
		}
		if (layoutKind === 'vertical') {
			return stackAutoBasisExpression('vertical', stackPlacements);
		}
		return null;
	});
	const resolvedGridDimensions = $derived.by(() => {
		if (layoutKind !== 'grid') {
			return { direction: 'row' as DashboardGridDirection, columns: 1, rows: 1 };
		}
		const lineCount = Math.max(1, gridSettings.lineCount);
		const itemCount = Math.max(
			renderedChildWidgets.length +
				(rendersInlineSurfaceDropPreview && displayedSurfaceDropPreview ? 1 : 0),
			1
		);
		if (gridSettings.direction === 'column') {
			return {
				direction: gridSettings.direction,
				columns: Math.max(1, Math.ceil(itemCount / lineCount)),
				rows: lineCount
			};
		}
		return {
			direction: gridSettings.direction,
			columns: lineCount,
			rows: Math.max(1, Math.ceil(itemCount / lineCount))
		};
	});

	const placementStyle = (
		childPlacement: DashboardPlacement | DashboardWidgetCreationPlacement
	): string => {
		const hasExplicitWidth =
			layoutKind === 'free' ? true : placementHasExplicitSize('horizontal', childPlacement);
		const hasExplicitHeight =
			layoutKind === 'free' ? true : placementHasExplicitSize('vertical', childPlacement);
		if (layoutKind === 'free') {
			const width = `max(${formatCssValue(childPlacement.size.width)}, ${minFreeWidgetWidthRem}rem)`;
			const height = `max(${formatCssValue(childPlacement.size.height)}, ${minFreeWidgetHeightRem}rem)`;
			const anchorX = anchorXFromAnchor(childPlacement.anchor);
			const anchorY = anchorYFromAnchor(childPlacement.anchor);
			const left =
				anchorX === 'left'
					? `${childPlacement.position.x}px`
					: anchorX === 'center'
						? `calc(${anchorBaseExpression(anchorX)} + ${childPlacement.position.x}px - (${width} / 2))`
						: `calc(${anchorBaseExpression(anchorX)} + ${childPlacement.position.x}px - ${width})`;
			const top =
				anchorY === 'top'
					? `${childPlacement.position.y}px`
					: anchorY === 'center'
						? `calc(${anchorBaseExpression(anchorY)} + ${childPlacement.position.y}px - (${height} / 2))`
						: `calc(${anchorBaseExpression(anchorY)} + ${childPlacement.position.y}px - ${height})`;
			return `left: ${left}; top: ${top}; inline-size: ${width}; block-size: ${height};`;
		}
		if (layoutKind === 'grid') {
			return 'inline-size: 100%; block-size: 100%; min-inline-size: 0; min-block-size: 0;';
		}
		if (layoutKind === 'horizontal') {
			const widthExpression = hasExplicitWidth
				? formatCssValue(childPlacement.size.width)
				: (stackAutoBasis ?? '0px');
			return `flex: 0 0 ${widthExpression}; inline-size: ${widthExpression}; block-size: 100%; min-inline-size: 0; min-block-size: 0;`;
		}
		const heightExpression = hasExplicitHeight
			? formatCssValue(childPlacement.size.height)
			: (stackAutoBasis ?? '0px');
		return `flex: 0 0 ${heightExpression}; inline-size: 100%; block-size: ${heightExpression}; min-inline-size: 0; min-block-size: 0;`;
	};

	const displayedWidgetPlacement = $derived.by(() => ({
		...placement,
		position: currentNodeFreeLayoutPreview
			? currentNodeFreeLayoutPreview.position
			: placement.position,
		size: currentNodeFreeLayoutPreview ? currentNodeFreeLayoutPreview.size : placement.size
	}));

	const anchorGuides = $derived.by((): AnchorGuide[] => {
		if (!editMode || !supportsFreePlacement || !isSelected) {
			return [];
		}
		const anchorX = anchorXFromAnchor(displayedWidgetPlacement.anchor);
		const anchorY = anchorYFromAnchor(displayedWidgetPlacement.anchor);
		const anchorInline = anchorX === 'left' ? '0%' : anchorX === 'center' ? '50%' : '100%';
		const anchorBlock = anchorY === 'top' ? '0%' : anchorY === 'center' ? '50%' : '100%';
		const guides: AnchorGuide[] = [];

		const pushHorizontalGuide = (direction: 'left' | 'right', lengthPx: number): void => {
			if (!(lengthPx > 0.5)) {
				return;
			}
			const start = direction === 'left' ? `calc(${anchorInline} - ${lengthPx}px)` : anchorInline;
			guides.push({
				axis: 'horizontal',
				direction,
				style: `inset-inline-start: ${start}; inset-block-start: calc(${anchorBlock} - 0.04rem); inline-size: ${lengthPx}px;`
			});
		};

		const pushVerticalGuide = (direction: 'up' | 'down', lengthPx: number): void => {
			if (!(lengthPx > 0.5)) {
				return;
			}
			const start = direction === 'up' ? `calc(${anchorBlock} - ${lengthPx}px)` : anchorBlock;
			guides.push({
				axis: 'vertical',
				direction,
				style: `inset-inline-start: calc(${anchorInline} - 0.04rem); inset-block-start: ${start}; block-size: ${lengthPx}px;`
			});
		};

		if (anchorX === 'left') {
			pushHorizontalGuide('left', Math.max(0, displayedWidgetPlacement.position.x));
		} else if (anchorX === 'right') {
			pushHorizontalGuide('right', Math.max(0, -displayedWidgetPlacement.position.x));
		} else if (displayedWidgetPlacement.position.x !== 0) {
			pushHorizontalGuide(
				displayedWidgetPlacement.position.x > 0 ? 'left' : 'right',
				Math.abs(displayedWidgetPlacement.position.x)
			);
		}

		if (anchorY === 'top') {
			pushVerticalGuide('up', Math.max(0, displayedWidgetPlacement.position.y));
		} else if (anchorY === 'bottom') {
			pushVerticalGuide('down', Math.max(0, -displayedWidgetPlacement.position.y));
		} else if (displayedWidgetPlacement.position.y !== 0) {
			pushVerticalGuide(
				displayedWidgetPlacement.position.y > 0 ? 'up' : 'down',
				Math.abs(displayedWidgetPlacement.position.y)
			);
		}

		return guides;
	});

	const slotStyle = (child: UiNodeDto): string =>
		placementStyle(getDashboardPlacement(graph, child));
	const inlineSurfaceDropPreviewStyle = (): string =>
		displayedSurfaceDropPreview ? placementStyle(displayedSurfaceDropPreview.placement) : '';

	const isHiddenSourceSurfaceChild = (child: UiNodeDto): boolean =>
		effectiveHiddenChildNodeId !== null &&
		layoutKind !== 'free' &&
		child.node_id === effectiveHiddenChildNodeId;

	const childPreviewInsertionIndexByNodeId = $derived.by(() => {
		const indices = new Map<NodeId, number>();
		let visibleIndex = 0;
		for (const child of childWidgets) {
			indices.set(child.node_id, visibleIndex);
			if (!isHiddenSourceSurfaceChild(child)) {
				visibleIndex += 1;
			}
		}
		return indices;
	});

	const shouldRenderSurfaceDropPreviewBeforeChild = (child: UiNodeDto): boolean => {
		if (!rendersInlineSurfaceDropPreview || !displayedSurfaceDropPreview) {
			return false;
		}
		if (displayedSurfaceDropPreview.targetIndex === null) {
			return false;
		}
		if (isHiddenSourceSurfaceChild(child)) {
			return false;
		}
		return (
			childPreviewInsertionIndexByNodeId.get(child.node_id) ===
			displayedSurfaceDropPreview.targetIndex
		);
	};

	const nonFreeSurfaceFlowRenderItems = $derived.by((): NonFreeSurfaceFlowRenderItem[] => {
		const items: NonFreeSurfaceFlowRenderItem[] = [];
		const preview = rendersInlineSurfaceDropPreview ? displayedSurfaceDropPreview : null;
		let previewInserted = false;

		for (const child of childWidgets) {
			if (
				preview &&
				!previewInserted &&
				!isHiddenSourceSurfaceChild(child) &&
				preview.targetIndex !== null &&
				childPreviewInsertionIndexByNodeId.get(child.node_id) === preview.targetIndex
			) {
				items.push({
					key: '__surface-drop-preview__',
					kind: 'preview',
					preview
				});
				previewInserted = true;
			}
			items.push({
				key: `child-${String(child.node_id)}`,
				kind: 'child',
				child
			});
		}

		if (preview && !previewInserted) {
			items.push({
				key: '__surface-drop-preview__',
				kind: 'preview',
				preview
			});
		}

		return items;
	});

	const shouldRenderSurfaceDropPreviewAtEnd = (): boolean => {
		if (!rendersInlineSurfaceDropPreview || !displayedSurfaceDropPreview) {
			return false;
		}
		return (
			displayedSurfaceDropPreview.targetIndex === null ||
			displayedSurfaceDropPreview.targetIndex >= renderedChildWidgets.length
		);
	};

	const widgetKind = $derived.by(() => {
		const widgetKindParam = getDirectParam(graph, liveNode, 'widget_kind');
		return widgetKindParam?.value.kind === 'enum' ? widgetKindParam.value.value : 'text';
	});
	let textParam = $derived(getDirectParam(graph, liveNode, 'text'));
	let placeholderParam = $derived(getDirectParam(graph, liveNode, 'placeholder'));
	let valueRangeParam = $derived(getDirectParam(graph, liveNode, 'value_range'));
	let stepParam = $derived(getDirectParam(graph, liveNode, 'step'));
	let multilineParam = $derived(getDirectParam(graph, liveNode, 'multiline'));
	let defaultCheckedParam = $derived(getDirectParam(graph, liveNode, 'default_checked'));
	let targetNodeParam = $derived(getDirectParam(graph, liveNode, 'target_node'));
	let targetParamParam = $derived(getDirectParam(graph, liveNode, 'target_param'));
	let widgetTypeParam = $derived(getDirectParam(graph, liveNode, 'widget_type'));
	let maxChildLevelParam = $derived(getDirectParam(graph, liveNode, 'max_child_level'));
	let showEnableButtonParam = $derived(getDirectParam(graph, liveNode, 'show_enable_button'));

	let boundNode = $derived(resolveReferenceTarget(graph, targetNodeParam?.value ?? null));
	let boundParamNode = $derived(resolveReferenceTarget(graph, targetParamParam?.value ?? null));
	let boundParam = $derived(
		boundParamNode?.data.kind === 'parameter' ? boundParamNode.data.param : null
	);

	const textConfig = $derived(textParam?.value.kind === 'str' ? textParam.value.value : '');
	const placeholderConfig = $derived(
		placeholderParam?.value.kind === 'str' ? placeholderParam.value.value : ''
	);
	const valueRange = $derived(
		valueRangeParam?.value.kind === 'vec2' ? valueRangeParam.value.value : [0, 1]
	);
	const sliderStep = $derived(stepParam?.value.kind === 'float' ? stepParam.value.value : 0.01);
	const multiline = $derived(
		multilineParam?.value.kind === 'bool' ? multilineParam.value.value : false
	);
	const defaultChecked = $derived(
		defaultCheckedParam?.value.kind === 'bool' ? defaultCheckedParam.value.value : false
	);
	const requestedWidgetType = $derived.by(() => {
		if (widgetTypeParam?.value.kind === 'enum' || widgetTypeParam?.value.kind === 'str') {
			return widgetTypeParam.value.value;
		}
		return 'auto';
	});
	const maxChildLevel = $derived.by(() => {
		if (maxChildLevelParam?.value.kind === 'int' || maxChildLevelParam?.value.kind === 'float') {
			return Math.max(0, Math.round(maxChildLevelParam.value.value));
		}
		return 2;
	});
	const showEnableButton = $derived(
		showEnableButtonParam?.value.kind === 'bool' ? showEnableButtonParam.value.value : true
	);
	const resolvedNodeWidgetType = $derived.by(() =>
		boundNode ? resolveDashboardNodeWidgetType(boundNode, requestedWidgetType) : null
	);
	const NodeWidgetDisplayComponent = $derived(resolvedNodeWidgetType?.component ?? null);
	const widgetUsesLabelPlacement = $derived.by(() => {
		if (isNodeWidget) {
			return resolvedNodeWidgetType?.usesLabelPlacement ?? false;
		}
		return true;
	});
	const widgetWrapperLabelPlacement = $derived.by(() => {
		if (
			!widgetUsesLabelPlacement ||
			widgetLabelPlacement === null ||
			widgetLabelPlacement === 'inside'
		) {
			return null;
		}
		return widgetLabelPlacement;
	});
	const hasExternalWidgetLabel = $derived(
		widgetWrapperLabelPlacement !== null && widgetLabelText.length > 0
	);
	const hasInsideWidgetLabel = $derived(
		widgetUsesLabelPlacement && widgetLabelPlacement === 'inside' && widgetLabelText.length > 0
	);

	const genericDisplayValue = $derived.by(() => {
		if (widgetKind === 'text' && boundParam) {
			return formatParamValue(boundParam.value);
		}
		return textConfig;
	});
	const genericSliderValue = $derived.by(() => {
		if (boundParam?.value.kind === 'float' || boundParam?.value.kind === 'int') {
			return boundParam.value.value;
		}
		return valueRange[0];
	});
	const genericSliderDisabled = $derived(
		editMode ||
			!boundParam ||
			(boundParam.value.kind !== 'float' && boundParam.value.kind !== 'int') ||
			boundParamNode?.meta.enabled === false
	);

	const applyGenericParamValue = async (value: ParamValue): Promise<void> => {
		if (!boundParamNode || boundParamNode.data.kind !== 'parameter') {
			return;
		}
		await sendSetParamIntent(
			boundParamNode.node_id,
			value,
			boundParamNode.data.param.event_behaviour
		);
	};

	$effect(() => {
		const isWidget = isContainerWidget || isNodeWidget || isGenericWidget;
		if (!isWidget) {
			observedAnchorPlacementCache = null;
			return;
		}

		const currentPlacement: ObservedAnchorPlacement = {
			nodeId: liveNode.node_id,
			anchor: placement.anchor,
			position: { ...placement.position },
			size: { width: { ...placement.size.width }, height: { ...placement.size.height } }
		};

		if (!editMode || !supportsFreePlacement || freeLayoutInteraction) {
			observedAnchorPlacementCache = currentPlacement;
			return;
		}

		const previousPlacement = observedAnchorPlacementCache;
		observedAnchorPlacementCache = currentPlacement;

		if (
			!previousPlacement ||
			previousPlacement.nodeId !== currentPlacement.nodeId ||
			previousPlacement.anchor === currentPlacement.anchor
		) {
			return;
		}

		const rootRemPx = getRootRemPixels();
		const surfaceMetrics = widgetShellElement
			? getClosestSurfaceMetrics(widgetShellElement, rootRemPx)
			: {
					width: getViewportWidthPx(),
					height: getViewportHeightPx(),
					scaleX: 1,
					scaleY: 1
				};
		const placementContext = {
			rootRemPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const previousRect = freeLayoutRectFromPlacement(previousPlacement, placementContext);
		const nextPlacement = freeLayoutPreviewFromRect(
			previousRect,
			currentPlacement.anchor,
			placementContext,
			currentPlacement.position,
			currentPlacement.size
		);
		if (samePosition(nextPlacement.position, currentPlacement.position)) {
			return;
		}

		observedAnchorPlacementCache = {
			...currentPlacement,
			position: { ...nextPlacement.position }
		};

		const positionParamNode = getDirectParamNode(graph, liveNode, 'position');
		if (!positionParamNode || positionParamNode.data.kind !== 'parameter') {
			return;
		}
		void sendSetParamIntent(
			positionParamNode.node_id,
			{ kind: 'vec2', value: [nextPlacement.position.x, nextPlacement.position.y] },
			positionParamNode.data.param.event_behaviour
		);
	});
	const triggerGenericAction = async (): Promise<void> => {
		if (boundParam?.value.kind === 'trigger') {
			await applyGenericParamValue({ kind: 'trigger' });
			return;
		}
		if (boundParam?.value.kind === 'bool') {
			await applyGenericParamValue({
				kind: 'bool',
				value: !boundParam.value.value
			});
		}
	};

	const applySliderValue = async (raw: string): Promise<void> => {
		if (!boundParam) {
			return;
		}
		const numericValue = Number(raw);
		if (!Number.isFinite(numericValue)) {
			return;
		}
		if (boundParam.value.kind === 'int') {
			await applyGenericParamValue({ kind: 'int', value: Math.round(numericValue) });
			return;
		}
		if (boundParam.value.kind === 'float') {
			await applyGenericParamValue({ kind: 'float', value: numericValue });
		}
	};

	const applyStringValue = async (raw: string): Promise<void> => {
		if (!boundParam) {
			return;
		}
		if (boundParam.value.kind === 'css_value') {
			const parsed = parseCssValue(raw, boundParam.value.unit);
			if (!parsed) {
				return;
			}
			await applyGenericParamValue({ kind: 'css_value', value: parsed.value, unit: parsed.unit });
			return;
		}
		if (
			boundParam.value.kind === 'str' ||
			boundParam.value.kind === 'file' ||
			boundParam.value.kind === 'enum'
		) {
			await applyGenericParamValue({ kind: boundParam.value.kind, value: raw });
		}
	};

	const toggleAccordionNode = (nodeId: NodeId): void => {
		if (openAccordionNodeIds.includes(nodeId)) {
			openAccordionNodeIds = openAccordionNodeIds.filter((candidate) => candidate !== nodeId);
			return;
		}
		openAccordionNodeIds = [...openAccordionNodeIds, nodeId];
	};

	const zoomPageAtClientPoint = (clientX: number, clientY: number, factor: number): void => {
		const geometry = getPageFrameGeometry();
		if (!geometry) {
			return;
		}
		cancelPageViewAnimation();
		const { frameCenterX, frameCenterY, baseCenterX, baseCenterY, currentScale } = geometry;
		const nextZoom = clampPageZoom(pageView.zoom * factor);
		if (Math.abs(nextZoom - pageView.zoom) <= 1e-9) {
			return;
		}
		const nextScale = Math.max(pageFitScale * nextZoom, 1e-6);
		const localCenterX = (clientX - frameCenterX) / currentScale;
		const localCenterY = (clientY - frameCenterY) / currentScale;
		pageView = {
			...pageView,
			zoom: nextZoom,
			panX: clientX - baseCenterX - localCenterX * nextScale,
			panY: clientY - baseCenterY - localCenterY * nextScale
		};
	};

	const frameSelectedWidgets = (): boolean => {
		const viewportElement = pageViewportElement;
		if (!viewportElement) {
			return false;
		}
		const selectedWidgetElements = (session?.selectedNodesIds ?? [])
			.map((nodeId) =>
				viewportElement.querySelector<HTMLElement>(`[data-node-id="${String(nodeId)}"]`)
			)
			.filter((element): element is HTMLElement => element !== null);
		return frameWidgetElements(selectedWidgetElements, {
			animate: true,
			fillRatio: 0.78
		});
	};

	const handlePageViewportPointerDown = (event: PointerEvent): void => {
		if (!pageNavigationEnabled) {
			return;
		}
		pageViewportElement?.focus();
		if (event.button !== 1) {
			return;
		}
		cancelPageViewAnimation();
		event.preventDefault();
		event.stopPropagation();
		pageView = {
			...pageView,
			pointerId: event.pointerId,
			lastClient: [event.clientX, event.clientY]
		};
	};

	const handlePageViewportWheel = (event: WheelEvent): void => {
		if (!pageNavigationEnabled) {
			return;
		}
		if (event.defaultPrevented) {
			return;
		}
		if (wheelTargetsScrollableContent(event)) {
			return;
		}
		event.preventDefault();
		const normalizedDelta = Math.max(-480, Math.min(480, normalizeWheelDeltaY(event)));
		const factor = Math.exp(-normalizedDelta / 1800);
		zoomPageAtClientPoint(event.clientX, event.clientY, factor);
	};

	$effect(() => {
		if (!isPage || !pageNavigationEnabled) {
			return;
		}
		const unregisterHandlers = [
			registerCommandHandler(
				'view.frame',
				() => {
					if (!isPageViewportFocused) {
						return false;
					}
					return frameSelectedWidgets();
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'view.home',
				() => {
					if (!isPageViewportFocused) {
						return false;
					}
					homePageView();
					return true;
				},
				{ priority: 200 }
			)
		];

		return () => {
			for (const unregister of unregisterHandlers) {
				unregister();
			}
		};
	});
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

{#snippet dashboardSurfaceContent(
	compact: boolean = false,
	dropIndicatorLabel: string = 'Drop to add a child widget',
	renderMarqueeInSurface: boolean = false
)}
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
						activeTabNodeId = child.node_id;
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
		<div class="dashboard-layout">
			{#each childWidgets as child (child.node_id)}
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
		class="dashboard-page-viewport"
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
					{@render dashboardSurfaceContent(false, 'Drop to create a widget')}
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
		class="dashboard-widget-shell dashboard-container"
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
					{@render dashboardSurfaceContent(true, 'Drop to add a child widget', true)}
				</div>
			</div>
		</div>
	</section>
{:else if isNodeWidget}
	<section
		class="dashboard-widget-shell dashboard-node-widget"
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
		class="dashboard-widget-shell dashboard-generic-widget"
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

<style>
	:global(.gc-dashboard-context-menu) {
		min-inline-size: 12rem;
	}

	:global(.gc-dashboard-context-menu .gc-context-item) {
		font-size: 0.78rem;
	}

	.dashboard-surface {
		position: relative;
		inline-size: 100%;
		block-size: auto;
		border-radius: 0.5rem;
		background: rgb(from var(--gc-color-background) r g b / 0.45);
		border: dashed 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.55);
	}

	.dashboard-surface.snap-grid-visible::before {
		content: '';
		position: absolute;
		inset: 0;
		pointer-events: none;
		background-image:
			linear-gradient(
				to right,
				rgb(from var(--gc-color-panel-outline) r g b / 0.16) 0.06rem,
				transparent 0.06rem
			),
			linear-gradient(
				to bottom,
				rgb(from var(--gc-color-panel-outline) r g b / 0.16) 0.06rem,
				transparent 0.06rem
			);
		background-size: var(--dashboard-snap-grid-step) var(--dashboard-snap-grid-step);
		background-position: 0 0;
		z-index: 0;
	}

	.dashboard-container-surface {
		box-sizing: border-box;
		block-size: 100%;
	}

	.dashboard-page-viewport {
		position: relative;
		display: flex;
		flex-direction: column;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.dashboard-page-viewport:focus,
	.dashboard-page-viewport:focus-visible {
		outline: none;
	}

	.dashboard-page-scene {
		position: relative;
		display: grid;
		place-items: center;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		box-sizing: border-box;
	}

	.dashboard-page-scene-hitbox {
		position: absolute;
		inset: 0;
		z-index: 0;
	}

	.dashboard-page-frame {
		position: relative;
		grid-area: 1 / 1;
		flex: 0 0 auto;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: visible;
		z-index: 1;
		transform-origin: center center;
		transition: box-shadow 120ms ease;
	}

	.dashboard-page {
		box-sizing: border-box;
		block-size: 100%;
		min-block-size: 100%;
		padding: 0;
		overflow: hidden;
	}

	.dashboard-page.edit-bleed-visible,
	.dashboard-page.edit-bleed-visible > .dashboard-layout {
		overflow: visible;
	}

	.dashboard-page-visibility-overlay {
		grid-area: 1 / 1;
		position: relative;
		pointer-events: none;
		border-radius: 0.9rem;
		box-shadow:
			0 0 0 100vmax rgb(from var(--gc-color-background) r g b / 0.2),
			0 0.8rem 2rem rgb(from var(--gc-color-background) r g b / 0.1);
		border: solid 0.06rem rgb(from var(--gc-color-selection) r g b / 0.2);
		transform-origin: center center;
	}

	.dashboard-page.surface-target-active,
	.dashboard-container-surface.surface-target-active,
	.dashboard-surface.dashboard-flow-drop-target,
	.dashboard-surface.dashboard-reparent-target {
		border-color: rgb(from var(--gc-color-focus) r g b / 0.86);
		box-shadow: 0 0 0 0.08rem rgb(from var(--gc-color-focus) r g b / 0.28);
	}

	.dashboard-surface.dashboard-flow-drop-target::before {
		content: '';
		position: absolute;
		inset-inline-start: var(--dashboard-flow-drop-indicator-left);
		inset-block-start: var(--dashboard-flow-drop-indicator-top);
		inline-size: var(--dashboard-flow-drop-indicator-width);
		block-size: var(--dashboard-flow-drop-indicator-height);
		border-radius: 999rem;
		background: rgb(from var(--gc-color-focus) r g b / 0.92);
		box-shadow: 0 0 0.55rem rgb(from var(--gc-color-focus) r g b / 0.28);
		pointer-events: none;
		z-index: 4;
	}

	.dashboard-surface.surface-target-active::after,
	.dashboard-surface.dashboard-reparent-target::after {
		content: attr(data-dashboard-drop-message);
		position: absolute;
		inset-block-start: 0.55rem;
		inset-inline-end: 0.55rem;
		z-index: 4;
		padding: 0.18rem 0.5rem;
		border-radius: 999rem;
		background: rgb(from var(--gc-color-focus) r g b / 0.86);
		color: white;
		font-size: 0.6rem;
		font-weight: 700;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		pointer-events: none;
	}

	.dashboard-surface.free > .dashboard-layout {
		position: relative;
		inline-size: 100%;
		block-size: 100%;
		min-block-size: inherit;
		z-index: 1;
	}

	.dashboard-surface.free > .dashboard-layout > .dashboard-slot {
		position: absolute;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-surface.grid > .dashboard-layout {
		display: grid;
		grid-auto-flow: var(--dashboard-grid-flow);
		grid-template-columns: repeat(var(--dashboard-grid-columns), minmax(0, 1fr));
		grid-template-rows: repeat(var(--dashboard-grid-rows), minmax(0, 1fr));
		gap: var(--dashboard-gap-y) var(--dashboard-gap-x);
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 100%;
	}

	.dashboard-surface.horizontal > .dashboard-layout,
	.dashboard-surface.vertical > .dashboard-layout {
		display: flex;
		flex: 1 1 auto;
		gap: var(--dashboard-gap-y) var(--dashboard-gap-x);
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-surface.horizontal {
		overflow-x: auto;
		overflow-y: hidden;
	}

	.dashboard-surface.vertical {
		overflow-x: hidden;
		overflow-y: auto;
	}

	.dashboard-surface.horizontal > .dashboard-layout {
		flex-direction: row;
		flex-wrap: nowrap;
		align-items: stretch;
		align-content: flex-start;
	}

	.dashboard-surface.vertical > .dashboard-layout {
		flex-direction: column;
		align-items: stretch;
		justify-content: flex-start;
	}

	.dashboard-tab-strip {
		display: flex;
		gap: 0.45rem;
		padding: 0.3rem 0.3rem 0.7rem;
		flex-wrap: wrap;
	}

	.dashboard-tab-strip.compact {
		padding: 0 0 0.65rem;
	}

	.dashboard-tab-strip button,
	.dashboard-accordion-item > button {
		border-radius: 999rem;
		padding: 0.35rem 0.75rem;
		background: rgb(from var(--gc-color-background) r g b / 0.45);
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.55);
		color: var(--gc-color-text);
		font-size: 0.74rem;
	}

	.dashboard-tab-strip button {
		cursor: pointer;
	}

	.dashboard-tab-strip button.selected,
	.dashboard-accordion-item > button.selected {
		background: rgb(from var(--gc-color-selection) r g b / 0.24);
		border-color: rgb(from var(--gc-color-selection) r g b / 0.7);
		color: white;
	}

	.dashboard-tab-body,
	.dashboard-accordion {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.dashboard-tab-body.compact {
		block-size: 100%;
	}

	.dashboard-accordion-item {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
	}

	.dashboard-slot {
		display: flex;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-hidden-drag-source {
		display: none !important;
	}

	.dashboard-slot-fill {
		display: flex;
		flex: 1 1 auto;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-slot.accordion-slot {
		padding-inline-start: 0.1rem;
	}

	.dashboard-surface-hit-zones {
		position: absolute;
		inset: 0;
		z-index: 2;
		pointer-events: none;
	}

	.dashboard-surface-hit-zone {
		position: absolute;
		pointer-events: auto;
		background: transparent;
	}

	.dashboard-surface-hit-zone.top,
	.dashboard-surface-hit-zone.bottom {
		inset-inline: 0;
		block-size: 0.75rem;
	}

	.dashboard-surface-hit-zone.top {
		inset-block-start: 0;
	}

	.dashboard-surface-hit-zone.bottom {
		inset-block-end: 0;
	}

	.dashboard-surface-hit-zone.left,
	.dashboard-surface-hit-zone.right {
		inset-block: 0.75rem;
		inline-size: 0.75rem;
	}

	.dashboard-surface-hit-zone.left {
		inset-inline-start: 0;
	}

	.dashboard-surface-hit-zone.right {
		inset-inline-end: 0;
	}

	.dashboard-surface.horizontal > .dashboard-layout > .dashboard-slot,
	.dashboard-tab-body .dashboard-slot,
	.dashboard-tab-body.compact .dashboard-slot {
		block-size: 100%;
	}

	.dashboard-surface.horizontal > .dashboard-layout > .dashboard-slot {
		flex: 0 0 auto;
		align-self: stretch;
		block-size: 100%;
	}

	.dashboard-surface.vertical > .dashboard-layout > .dashboard-slot,
	.dashboard-surface.grid > .dashboard-layout > .dashboard-slot,
	.dashboard-accordion .dashboard-slot {
		inline-size: 100%;
	}

	.dashboard-surface.grid > .dashboard-layout > .dashboard-slot {
		block-size: 100%;
	}

	.dashboard-surface.vertical > .dashboard-layout > .dashboard-slot {
		flex: 0 0 auto;
		inline-size: 100%;
	}

	.dashboard-widget-shell {
		position: relative;
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		inline-size: 100%;
		block-size: 100%;
		align-self: stretch;
		min-inline-size: 0;
		min-block-size: 0;
		border-radius: 0.5rem;
		background: rgb(from var(--gc-color-background) r g b / 0);
		border: dashed 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.28);
		box-shadow: none;
		overflow: hidden;
		transition:
			border-color 120ms ease,
			border-style 120ms ease,
			box-shadow 120ms ease,
			background 120ms ease;
	}

	.dashboard-widget-shell.editable-free {
		cursor: grab;
	}

	.dashboard-widget-shell.editable-free.free-layout-active {
		cursor: grabbing;
	}

	.dashboard-widget-shell:not(.run-mode):hover {
		border-style: solid;
		border-color: rgb(from var(--gc-color-panel-outline) r g b / 0.82);
		background: rgb(from var(--gc-color-background) r g b / 0.08);
	}

	.dashboard-widget-shell.selected {
		border-style: solid;
		border-color: rgb(from var(--gc-color-selection) r g b / 0.8);
		box-shadow:
			0 0 0 0.08rem rgb(from var(--gc-color-selection) r g b / 0.35),
			0 0.45rem 1rem rgb(from var(--gc-color-selection) r g b / 0.14);
	}

	.dashboard-widget-shell.selected.editable-free {
		overflow: visible;
	}

	.dashboard-widget-shell.dashboard-descendant-free-layout-active {
		overflow: visible;
	}

	.dashboard-widget-shell.binding-active {
		border-style: solid;
		border-color: rgb(from var(--gc-color-focus) r g b / 0.84);
		box-shadow: 0 0 0 0.08rem rgb(from var(--gc-color-focus) r g b / 0.28);
	}

	.dashboard-widget-shell.free-layout-active {
		box-shadow:
			0 0 0 0.08rem rgb(from var(--gc-color-focus) r g b / 0.34),
			0 0 0.9rem rgb(from var(--gc-color-background) r g b / 0.2);
	}

	.dashboard-widget-shell.run-mode {
		border-color: transparent;
		border-style: solid;
		box-shadow: none;
		background: transparent;
	}

	.dashboard-widget-frame {
		position: relative;
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-widget-frame.label-left,
	.dashboard-widget-frame.label-right {
		flex-direction: row;
	}

	.dashboard-widget-frame.label-bottom {
		flex-direction: column;
	}

	.dashboard-widget-frame.label-right > .dashboard-widget-label,
	.dashboard-widget-frame.label-bottom > .dashboard-widget-label {
		order: 1;
	}

	.dashboard-widget-body {
		position: relative;
		display: flex;
		flex: 1 1 auto;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-widget-label {
		position: relative;
		z-index: 1;
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		min-inline-size: 0;
		padding: 0.45rem 0.7rem;
		font-size: 0.72rem;
		font-weight: 600;
		line-height: 1.2;
		color: #eef4ff;
		background: linear-gradient(
			180deg,
			rgb(from var(--gc-color-panel-outline) r g b / 0.12),
			rgb(from var(--gc-color-background) r g b / 0.2)
		);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.dashboard-widget-frame.label-top > .dashboard-widget-label {
		border-block-end: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.24);
	}

	.dashboard-widget-frame.label-bottom > .dashboard-widget-label {
		border-block-start: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.24);
	}

	.dashboard-widget-frame.label-left > .dashboard-widget-label,
	.dashboard-widget-frame.label-right > .dashboard-widget-label {
		inline-size: auto;
		block-size: 100%;
		min-inline-size: 2rem;
		padding: 0.65rem 0.45rem;
		justify-content: center;
		text-align: center;
		writing-mode: vertical-rl;
		text-orientation: mixed;
		white-space: normal;
	}

	.dashboard-widget-frame.label-left > .dashboard-widget-label {
		border-inline-end: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.24);
		transform: rotate(180deg);
	}

	.dashboard-widget-frame.label-right > .dashboard-widget-label {
		border-inline-start: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.24);
	}

	.dashboard-resize-zone {
		position: absolute;
		background: transparent;
		z-index: 3;
	}

	.dashboard-resize-zone.north,
	.dashboard-resize-zone.south {
		inset-inline: 0.75rem;
		block-size: 0.5rem;
	}

	.dashboard-resize-zone.east,
	.dashboard-resize-zone.west {
		inset-block: 0.75rem;
		inline-size: 0.5rem;
	}

	.dashboard-resize-zone.north {
		inset-block-start: 0;
		cursor: ns-resize;
	}

	.dashboard-resize-zone.south {
		inset-block-end: 0;
		cursor: ns-resize;
	}

	.dashboard-resize-zone.east {
		inset-inline-end: 0;
		cursor: ew-resize;
	}

	.dashboard-resize-zone.west {
		inset-inline-start: 0;
		cursor: ew-resize;
	}

	.dashboard-resize-zone.north-west,
	.dashboard-resize-zone.north-east,
	.dashboard-resize-zone.south-east,
	.dashboard-resize-zone.south-west {
		inline-size: 0.9rem;
		block-size: 0.9rem;
	}

	.dashboard-resize-zone.north-west {
		inset-inline-start: 0;
		inset-block-start: 0;
		cursor: nwse-resize;
	}

	.dashboard-resize-zone.north-east {
		inset-inline-end: 0;
		inset-block-start: 0;
		cursor: nesw-resize;
	}

	.dashboard-resize-zone.south-east {
		inset-inline-end: 0;
		inset-block-end: 0;
		cursor: nwse-resize;
	}

	.dashboard-resize-zone.south-west {
		inset-inline-start: 0;
		inset-block-end: 0;
		cursor: nesw-resize;
	}

	.dashboard-drop-indicator,
	.dashboard-drop-preview-kind {
		text-transform: uppercase;
		letter-spacing: 0.08em;
		font-size: 0.62rem;
		opacity: 0.7;
	}

	.dashboard-widget-content {
		display: flex;
		flex: 1 1 auto;
		padding: 0;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.dashboard-widget-content.inspector-body {
		display: flex;
		flex: 1 1 auto;
		position: relative;
		overflow: auto;
	}

	.dashboard-widget-shell:not(.run-mode) .dashboard-widget-content::after {
		content: '';
		position: absolute;
		inset: 0;
		pointer-events: none;
		z-index: 2;
		background: rgb(from var(--gc-color-background) r g b / 0.12);
		opacity: 0;
		transition:
			opacity 120ms ease,
			background 120ms ease;
	}

	.dashboard-widget-shell:not(.run-mode):hover .dashboard-widget-content::after,
	.dashboard-widget-shell.selected .dashboard-widget-content::after,
	.dashboard-widget-shell.binding-active .dashboard-widget-content::after {
		background: rgb(from var(--gc-color-background) r g b / 0.16);
		opacity: 1;
	}

	.dashboard-widget-shell.run-mode .dashboard-widget-content::after {
		display: none;
	}

	.dashboard-live-content {
		position: relative;
		z-index: 1;
		display: flex;
		flex: 1 1 auto;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: auto;
	}

	.dashboard-live-content.inert-mode {
		pointer-events: none;
		user-select: none;
	}

	.dashboard-container-surface {
		padding: 0;
		border: none;
		background: transparent;
		box-shadow: none;
	}

	.dashboard-container-surface.with-inline-label {
		padding-block-start: 2rem;
	}

	.dashboard-container-inline-label {
		position: absolute;
		inset-inline-start: 0.6rem;
		inset-block-start: 0.55rem;
		z-index: 4;
		max-inline-size: calc(100% - 1.2rem);
		padding: 0.26rem 0.5rem;
		border-radius: 999rem;
		background: rgb(from var(--gc-color-background) r g b / 0.82);
		box-shadow: 0 0.2rem 0.6rem rgb(from var(--gc-color-background) r g b / 0.16);
		font-size: 0.66rem;
		font-weight: 600;
		line-height: 1.1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		pointer-events: none;
	}

	.dashboard-anchor-guides {
		position: absolute;
		inset: 0;
		pointer-events: none;
		z-index: 5;
		overflow: visible;
	}

	.dashboard-anchor-guide {
		position: absolute;
		pointer-events: none;
		background: rgb(from var(--gc-color-selection) r g b / 0.42);
		box-shadow: 0 0 0.3rem rgb(from var(--gc-color-selection) r g b / 0.15);
	}

	.dashboard-anchor-guide.horizontal {
		block-size: 0.08rem;
	}

	.dashboard-anchor-guide.vertical {
		inline-size: 0.08rem;
	}

	.dashboard-anchor-guide::after {
		content: '';
		position: absolute;
		inline-size: 0.34rem;
		block-size: 0.34rem;
		background: rgb(from var(--gc-color-selection) r g b / 0.58);
		clip-path: polygon(50% 0%, 100% 100%, 0% 100%);
	}

	.dashboard-anchor-guide.horizontal.left::after {
		inset-inline-start: -0.18rem;
		inset-block-start: calc(50% - 0.17rem);
		transform: rotate(-90deg);
	}

	.dashboard-anchor-guide.horizontal.right::after {
		inset-inline-end: -0.18rem;
		inset-block-start: calc(50% - 0.17rem);
		transform: rotate(90deg);
	}

	.dashboard-anchor-guide.vertical.up::after {
		inset-inline-start: calc(50% - 0.17rem);
		inset-block-start: -0.18rem;
		transform: rotate(0deg);
	}

	.dashboard-anchor-guide.vertical.down::after {
		inset-inline-start: calc(50% - 0.17rem);
		inset-block-end: -0.18rem;
		transform: rotate(180deg);
	}

	.dashboard-marquee-selection {
		position: absolute;
		pointer-events: none;
		z-index: 1000;
		border: solid 0.08rem rgb(from var(--gc-color-selection) r g b / 0.92);
		background: rgb(from var(--gc-color-selection) r g b / 0.16);
		box-shadow: inset 0 0 0 0.04rem rgb(from var(--gc-color-selection) r g b / 0.24);
		border-radius: 0.2rem;
	}

	.dashboard-empty-inline,
	.dashboard-drop-indicator {
		display: flex;
		align-items: center;
		justify-content: center;
		text-align: center;
		color: var(--gc-color-text);
	}

	.dashboard-empty-inline {
		margin: 0;
		font-size: 0.76rem;
		line-height: 1.45;
		opacity: 0.78;
	}

	.dashboard-empty-inline {
		min-block-size: 4.25rem;
		border-radius: 0.75rem;
		border: dashed 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.45);
		background: rgb(from var(--gc-color-background) r g b / 0.22);
		padding: 0.55rem;
	}

	.dashboard-surface-empty {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		min-block-size: 0;
		box-sizing: border-box;
		pointer-events: none;
		z-index: 1;
	}

	.dashboard-drop-indicator {
		position: absolute;
		inset: 0.7rem;
		border-radius: 0.8rem;
		border: dashed 0.08rem rgb(from var(--gc-color-focus) r g b / 0.9);
		background: rgb(from var(--gc-color-focus) r g b / 0.16);
		color: white;
		pointer-events: none;
	}

	.dashboard-drop-preview {
		pointer-events: none;
		z-index: 2;
	}

	.dashboard-drop-preview-flow,
	.dashboard-drop-preview-panel {
		min-inline-size: 0;
		min-block-size: 4.75rem;
	}

	.dashboard-drop-preview-flow {
		align-self: stretch;
	}

	.dashboard-drop-preview-panel {
		flex: 1 1 auto;
		inline-size: 100%;
	}

	.dashboard-drop-preview-tab {
		pointer-events: none;
		border-style: dashed;
	}

	.dashboard-drop-preview.is-committing {
		opacity: 0.82;
	}

	.dashboard-drop-preview-fill {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		gap: 0.55rem;
		inline-size: 100%;
		block-size: 100%;
		padding: 0.75rem 0.85rem;
		box-sizing: border-box;
		border-radius: 0.8rem;
		border: dashed 0.08rem rgb(from var(--gc-color-focus) r g b / 0.92);
		background:
			linear-gradient(
				180deg,
				rgb(from var(--gc-color-focus) r g b / 0.18),
				rgb(from var(--gc-color-selection) r g b / 0.1)
			),
			rgb(from var(--gc-color-background) r g b / 0.62);
		box-shadow: 0 0.4rem 1rem rgb(from var(--gc-color-background) r g b / 0.2);
		color: white;
		overflow: hidden;
	}

	.dashboard-drop-preview-meta {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
	}

	.dashboard-drop-preview-fill strong {
		font-size: 0.78rem;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.dashboard-drop-preview-kind {
		opacity: 0.72;
	}

	.dashboard-drop-preview-shell {
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		justify-content: center;
		gap: 0.45rem;
		min-block-size: 0;
	}

	.dashboard-drop-preview-node-toolbar,
	.dashboard-drop-preview-node-lines {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.dashboard-drop-preview-node-toolbar span,
	.dashboard-drop-preview-node-lines span,
	.dashboard-drop-preview-slider-track,
	.dashboard-drop-preview-input,
	.dashboard-drop-preview-text,
	.dashboard-drop-preview-button,
	.dashboard-drop-preview-checkbox {
		border-radius: 0.55rem;
	}

	.dashboard-drop-preview-node-toolbar span,
	.dashboard-drop-preview-node-lines span,
	.dashboard-drop-preview-slider-track,
	.dashboard-drop-preview-checkbox-box {
		background: rgb(from var(--gc-color-text) r g b / 0.18);
	}

	.dashboard-drop-preview-node-toolbar span {
		block-size: 0.45rem;
	}

	.dashboard-drop-preview-node-lines span {
		block-size: 0.5rem;
	}

	.dashboard-drop-preview-node-lines span.wide {
		inline-size: 100%;
	}

	.dashboard-drop-preview-node-lines span.short {
		inline-size: 58%;
	}

	.dashboard-drop-preview-button,
	.dashboard-drop-preview-text,
	.dashboard-drop-preview-input,
	.dashboard-drop-preview-checkbox,
	.dashboard-drop-preview-slider {
		display: flex;
		align-items: center;
		min-block-size: 0;
	}

	.dashboard-drop-preview-button,
	.dashboard-drop-preview-text,
	.dashboard-drop-preview-input {
		padding: 0.65rem 0.8rem;
		background: rgb(from var(--gc-color-background) r g b / 0.38);
		border: solid 0.06rem rgb(from var(--gc-color-text) r g b / 0.18);
	}

	.dashboard-drop-preview-button {
		justify-content: center;
		font-weight: 600;
	}

	.dashboard-drop-preview-text {
		align-items: flex-start;
		line-height: 1.4;
	}

	.dashboard-drop-preview-input {
		color: rgb(from var(--gc-color-text) r g b / 0.72);
	}

	.dashboard-drop-preview-input.multiline {
		align-items: flex-start;
		min-block-size: 4.4rem;
	}

	.dashboard-drop-preview-slider {
		flex-direction: column;
		align-items: stretch;
		gap: 0.45rem;
	}

	.dashboard-drop-preview-slider-label {
		font-size: 0.72rem;
		opacity: 0.78;
	}

	.dashboard-drop-preview-slider-track {
		display: block;
		block-size: 0.5rem;
		overflow: hidden;
	}

	.dashboard-drop-preview-slider-track > span {
		display: block;
		block-size: 100%;
		inline-size: 48%;
		border-radius: inherit;
		background: rgb(from var(--gc-color-selection) r g b / 0.78);
	}

	.dashboard-drop-preview-checkbox {
		gap: 0.55rem;
		padding: 0.65rem 0.1rem;
	}

	.dashboard-drop-preview-checkbox-box {
		inline-size: 1rem;
		block-size: 1rem;
		border-radius: 0.3rem;
		flex: 0 0 auto;
	}

	.generic-body {
		display: flex;
		flex: 1 1 auto;
		align-items: stretch;
		justify-content: stretch;
		padding: 0;
		min-block-size: 0;
	}

	.generic-button,
	.generic-text-display,
	.generic-slider-wrap,
	.generic-checkbox,
	.generic-input-shell {
		inline-size: 100%;
		block-size: 100%;
	}

	.generic-button {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 0.7rem;
		padding: 0.55rem 0.8rem;
		background:
			linear-gradient(
				135deg,
				rgb(from var(--gc-color-selection) r g b / 0.32),
				rgb(from var(--gc-color-binding) r g b / 0.18)
			),
			rgb(from var(--gc-color-background) r g b / 0.7);
		border: solid 0.06rem rgb(from var(--gc-color-selection) r g b / 0.6);
		color: #f2f5ff;
		font-size: 0.76rem;
		font-weight: 600;
	}

	.generic-text-display,
	.generic-slider-wrap,
	.generic-checkbox,
	.generic-input-shell {
		border-radius: 0.7rem;
		background: rgb(from var(--gc-color-background) r g b / 0.48);
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.48);
		padding: 0.5rem 0.65rem;
		box-sizing: border-box;
	}

	.generic-button.with-inside-label,
	.generic-text-display.with-inside-label,
	.generic-checkbox.with-inside-label,
	.generic-input-shell.with-inside-label {
		padding-block-start: 1.85rem;
	}

	.generic-inline-label {
		position: absolute;
		inset-inline-start: 0.65rem;
		inset-block-start: 0.55rem;
		z-index: 1;
		max-inline-size: calc(100% - 1.3rem);
		padding: 0.26rem 0.5rem;
		border-radius: 999rem;
		background: rgb(from var(--gc-color-background) r g b / 0.82);
		box-shadow: 0 0.2rem 0.6rem rgb(from var(--gc-color-background) r g b / 0.16);
		font-size: 0.66rem;
		font-weight: 600;
		line-height: 1.1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		pointer-events: none;
	}

	.generic-button-text,
	.generic-text-display-value,
	.generic-checkbox-text {
		min-inline-size: 0;
	}

	.generic-slider-wrap {
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		gap: 0.5rem;
	}

	.generic-slider-bar {
		display: flex;
		flex: 1 1 auto;
		min-block-size: 1.6rem;
	}

	.generic-slider-bar :global(.slider-label) {
		padding-inline: 0.7rem;
		font-size: 0.72rem;
	}

	.generic-text-display {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		text-align: center;
	}

	.generic-readout {
		font-size: 0.72rem;
		opacity: 0.76;
	}

	.generic-checkbox {
		position: relative;
		display: flex;
		flex: 1 1 auto;
		gap: 0.6rem;
		align-items: center;
	}

	.generic-checkbox input {
		flex: 0 0 auto;
	}

	.generic-input-shell {
		position: relative;
		display: flex;
		flex: 1 1 auto;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.generic-input-shell > input[type='text'],
	.generic-input-shell > textarea {
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		border: none;
		background: transparent;
		padding: 0;
		outline: none;
		font: inherit;
		color: inherit;
	}

	.generic-input-shell > textarea {
		resize: none;
		min-block-size: 0;
	}
</style>
