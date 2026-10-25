import { onDestroy } from 'svelte';
import { marqueeDragThresholdPx } from './dom';
import type { GraphState } from '../../../../store/graph.svelte';
import type { DashboardPlacement, DashboardLayoutKind } from '../dashboard-model';
import type { createDashboardInteractionChrome } from './interaction-chrome.svelte';
import type { createDashboardInteractionGeometry } from './interaction-geometry.svelte';
import type { getDashboardWidgetOrderingState } from '../dashboard-actions';
import {
	type FreeLayoutResizeEdges,
	type WidgetMoveDropTarget,
	type WidgetMoveInteraction,
	type ConstrainedResizeInteraction
} from './contracts';
import {
	minFreeWidgetWidthRem,
	minFreeWidgetHeightRem,
	createCssUnitContext,
	sameCssValue
} from './geometry';
import {
	getRootRemPixels,
	getViewportWidthPx,
	getViewportHeightPx,
	getClosestSurfaceMetrics
} from './dom';
import { sendPatchMetaIntent, sendSetParamIntent } from '../../../../store/ui-intents';
import { type UiNodeDto } from '../../../../types';
import { cssValueToPx, pxToCssValue } from '../../../../css-value';
import { moveDashboardWidgetToSurface } from '../dashboard-actions';
import { getDirectParamNode } from '../dashboard-model';
interface DashboardOrderedLayoutContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly parent: UiNodeDto | null;
	readonly placement: DashboardPlacement;
	readonly editMode: boolean;
	readonly parentLayoutKind: DashboardLayoutKind;
	readonly ordering: ReturnType<typeof getDashboardWidgetOrderingState>;
	readonly chrome: ReturnType<typeof createDashboardInteractionChrome>;
	readonly geometry: ReturnType<typeof createDashboardInteractionGeometry>;
	clearChrome(): void;
}
export function createDashboardOrderedLayout(context: DashboardOrderedLayoutContext) {
	let graph = $derived(context.graph);
	let liveNode = $derived(context.node);
	let parentNode = $derived(context.parent);
	let placement = $derived(context.placement);
	let editMode = $derived(context.editMode);
	let parentLayoutKind = $derived(context.parentLayoutKind);
	let widgetOrderingState = $derived(context.ordering);
	const getGraph = () => context.graph;
	const clearFreeLayoutInteractionChrome = () => context.clearChrome();
	const {
		setFreeLayoutAncestorShells,
		setFreeLayoutHoverSurface,
		setSurfaceInsertionIndicator,
		publishWidgetMoveSurfacePreview,
		publishWidgetMoveCommit
	} = context.chrome;
	const { resolveWidgetMoveDropTargetAtPoint } = context.geometry;
	let widgetMoveInteraction = $state<WidgetMoveInteraction | null>(null);

	let widgetMoveDropTarget = $state<WidgetMoveDropTarget | null>(null);

	let constrainedResizeInteraction = $state<ConstrainedResizeInteraction | null>(null);

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
			emPx: surfaceMetrics.emPx,
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
			emPx: surfaceMetrics.emPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			surfaceScaleX: surfaceMetrics.scaleX,
			surfaceScaleY: surfaceMetrics.scaleY,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx(),
			lastSent: null
		};
	};

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

	let supportsStackSizing = $derived(
		editMode && (parentLayoutKind === 'horizontal' || parentLayoutKind === 'vertical')
	);

	let supportsOrdering = $derived(editMode && parentLayoutKind !== 'free');
	const resetOrderedDropTarget = () => {
		widgetMoveDropTarget = null;
	};
	onDestroy(() => {
		widgetMoveInteraction = null;
		constrainedResizeInteraction = null;
	});
	return {
		get widgetMoveInteraction() {
			return widgetMoveInteraction;
		},
		get constrainedResizeInteraction() {
			return constrainedResizeInteraction;
		},
		get supportsOrdering() {
			return supportsOrdering;
		},
		beginWidgetMoveInteraction,
		beginConstrainedResize,
		resetOrderedDropTarget
	};
}
