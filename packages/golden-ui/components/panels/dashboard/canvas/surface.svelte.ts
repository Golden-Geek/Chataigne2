import { onDestroy, untrack } from 'svelte';
import type { DashboardViewContext } from './view-context.svelte';
import type { GraphState } from '../../../../store/graph.svelte';
import type { createDashboardInteractionChrome } from './interaction-chrome.svelte';
import type { createDashboardInteractionGeometry } from './interaction-geometry.svelte';
import { resolveDashboardDragNode, resolveDashboardDragParameterNode } from '../dashboard-drag';
import { widgetMovePreviewEventName, widgetMoveCommitEventName } from './contracts';
import {
	type SurfaceDropPreview,
	type PendingSurfaceDrop,
	type PendingWidgetMoveCommit,
	type NonFreeSurfaceFlowRenderItem,
	type WidgetMovePreviewEventDetail,
	type WidgetMoveCommitEventDetail
} from './contracts';
import {
	minFreeWidgetWidthRem,
	minFreeWidgetHeightRem,
	sameNodeIdList,
	moveNodeIdInList,
	anchorXFromAnchor,
	anchorYFromAnchor,
	anchorBaseExpression,
	sumCssLengthExpressions,
	placementHasExplicitSize
} from './geometry';
import {
	getRootRemPixels,
	getViewportWidthPx,
	getViewportHeightPx,
	getElementSurfaceMetrics,
	getDirectDashboardLayoutElement,
	currentSurfaceOwnsDragEvent
} from './dom';
import { type NodeId, type UiNodeDto } from '../../../../types';
import { formatCssValue } from '../../../../css-value';
import {
	createDashboardNodeWidget,
	getDashboardNodeWidgetCreationDefaults,
	type DashboardWidgetCreationPlacement
} from '../dashboard-actions';
import { readDashboardDragPayload, type DashboardDragPayload } from '../dashboard-drag';
import {
	type DashboardGridDirection,
	getDashboardGridSettings,
	getDashboardLayoutKind,
	getDashboardPlacement,
	getDashboardSnapGrid,
	getDirectItemChildren,
	getGap,
	type DashboardPlacement
} from '../dashboard-model';
interface DashboardSurfaceContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly editMode: boolean;
	readonly view: DashboardViewContext | null;
	readonly chrome: ReturnType<typeof createDashboardInteractionChrome>;
	readonly geometry: ReturnType<typeof createDashboardInteractionGeometry>;
}
export function createDashboardSurface(context: DashboardSurfaceContext) {
	let graph = $derived(context.graph);
	let liveNode = $derived(context.node);
	let editMode = $derived(context.editMode);
	let isPage = $derived(liveNode.node_type === 'dashboard_page');
	let isLayoutSurface = $derived(isPage || liveNode.node_type === 'dashboard_widget_container');
	const getGraph = () => context.graph;
	const resolveDraggedNode = (payload: DashboardDragPayload | null) =>
		resolveDashboardDragNode(context.graph, payload);
	const resolveDraggedParameterNode = (payload: DashboardDragPayload | null) =>
		resolveDashboardDragParameterNode(context.graph, payload);
	const clearSurfaceInsertionIndicator = () => context.chrome.clearSurfaceInsertionIndicator();
	const setSurfaceInsertionIndicator = (
		...args: Parameters<
			ReturnType<typeof createDashboardInteractionChrome>['setSurfaceInsertionIndicator']
		>
	) => context.chrome.setSurfaceInsertionIndicator(...args);
	const resolveSurfaceInsertionTarget = (
		...args: Parameters<
			ReturnType<typeof createDashboardInteractionGeometry>['resolveSurfaceInsertionTarget']
		>
	) => context.geometry.resolveSurfaceInsertionTarget(...args);
	let layoutKind = $derived(getDashboardLayoutKind(graph, liveNode, isPage ? 'free' : 'vertical'));

	let gap = $derived(getGap(graph, liveNode));

	let gridSettings = $derived(getDashboardGridSettings(graph, liveNode));

	let childWidgets = $derived(getDirectItemChildren(graph, liveNode));

	let renderedChildWidgets = $derived.by(() =>
		effectiveHiddenChildNodeId === null || layoutKind === 'free'
			? childWidgets
			: childWidgets.filter((child) => child.node_id !== effectiveHiddenChildNodeId)
	);

	let activeTabNodeId = $state<NodeId | null>(null);

	let openAccordionNodeIds = $state<NodeId[]>([]);
	let restoredUuid: string | null = null;
	let restoredChildren: readonly NodeId[] | null = null;
	$effect(() => {
		const owner = liveNode;
		if (!isLayoutSurface || (restoredUuid === owner.uuid && restoredChildren === owner.children))
			return;
		restoredUuid = owner.uuid;
		restoredChildren = owner.children;
		untrack(() => {
			const state = context.view?.readPresentation(owner.uuid);
			if (!state) {
				activeTabNodeId = null;
				openAccordionNodeIds = [];
				return;
			}
			const identities = new Map(childWidgets.map((child) => [child.uuid, child.node_id]));
			activeTabNodeId = state.activeTabUuid ? (identities.get(state.activeTabUuid) ?? null) : null;
			openAccordionNodeIds = state.openAccordionUuids.flatMap((uuid) => {
				const id = identities.get(uuid);
				return id === undefined ? [] : [id];
			});
		});
	});
	const savePresentation = () => {
		if (!isLayoutSurface || (layoutKind !== 'tabs' && layoutKind !== 'accordion')) return;
		const owner = liveNode;
		const active = activeTabNodeId;
		const open = openAccordionNodeIds;
		untrack(() => {
			if (graph?.nodesById.get(owner.node_id)?.uuid !== owner.uuid) return;
			context.view?.writePresentation(owner.uuid, {
				activeTabUuid: active === null ? null : (graph?.nodesById.get(active)?.uuid ?? null),
				openAccordionUuids: open.flatMap((id) => {
					const uuid = graph?.nodesById.get(id)?.uuid;
					return uuid ? [uuid] : [];
				})
			});
		});
	};
	$effect(savePresentation);
	$effect(() => {
		if (!isLayoutSurface) return;
		return context.view?.registerPresentation(liveNode.node_id, {
			activateTab: (id) => activateDashboardTab(id),
			toggleAccordion: (id) => toggleAccordionNode(id)
		});
	});
	onDestroy(() => untrack(savePresentation));

	let surfaceDragDepth = $state(0);

	let surfaceDropPreview = $state<SurfaceDropPreview | null>(null);

	let widgetMoveSurfacePreview = $state<SurfaceDropPreview | null>(null);

	let widgetMoveHiddenChildNodeId = $state<NodeId | null>(null);

	let pendingSurfaceDrop = $state<PendingSurfaceDrop | null>(null);

	let pendingWidgetMoveCommit = $state<PendingWidgetMoveCommit | null>(null);

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

	const clearSurfaceDropState = (): void => {
		surfaceDragDepth = 0;
		surfaceDropPreview = null;
		clearSurfaceInsertionIndicator();
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
			emPx: metrics.emPx,
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

	let surfaceSnapGrid = $derived(getDashboardSnapGrid(graph, liveNode));

	let snapGridVisible = $derived(editMode && surfaceSnapGrid.enabled && layoutKind === 'free');

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

	const toggleAccordionNode = (nodeId: NodeId): boolean => {
		if (!childWidgets.some((child) => child.node_id === nodeId)) return false;
		if (openAccordionNodeIds.includes(nodeId)) {
			openAccordionNodeIds = openAccordionNodeIds.filter((candidate) => candidate !== nodeId);
			return true;
		}
		openAccordionNodeIds = [...openAccordionNodeIds, nodeId];
		return true;
	};
	const activateDashboardTab = (nodeId: NodeId) => {
		if (!childWidgets.some((child) => child.node_id === nodeId)) return false;
		activeTabNodeId = nodeId;
		return true;
	};
	return {
		get graph() {
			return graph;
		},
		get node() {
			return liveNode;
		},
		get layoutKind() {
			return layoutKind;
		},
		get childWidgets(): UiNodeDto[] {
			return childWidgets;
		},
		get renderedChildWidgets(): UiNodeDto[] {
			return renderedChildWidgets;
		},
		get activeTabNodeId() {
			return activeTabNodeId;
		},
		get openAccordionNodeIds() {
			return openAccordionNodeIds;
		},
		get surfaceDragDepth() {
			return surfaceDragDepth;
		},
		get surfaceDropPreview() {
			return surfaceDropPreview;
		},
		get pendingSurfaceDrop() {
			return pendingSurfaceDrop;
		},
		get displayedSurfaceDropPreview() {
			return displayedSurfaceDropPreview;
		},
		get surfaceSnapGrid() {
			return surfaceSnapGrid;
		},
		get snapGridVisible() {
			return snapGridVisible;
		},
		get surfaceStyle() {
			return surfaceStyle;
		},
		get nonFreeSurfaceFlowRenderItems() {
			return nonFreeSurfaceFlowRenderItems;
		},
		handleSurfaceDragEnter,
		handleSurfaceDragOver,
		handleSurfaceDragLeave,
		handleSurfaceDrop,
		slotStyle,
		inlineSurfaceDropPreviewStyle,
		isHiddenSourceSurfaceChild,
		shouldRenderSurfaceDropPreviewBeforeChild,
		shouldRenderSurfaceDropPreviewAtEnd,
		toggleAccordionNode,
		activateDashboardTab
	};
}
