import { onDestroy } from 'svelte';
import type { GraphState } from '../../../../store/graph.svelte';
import type { SelectionMode } from '../../../../store/workbench.svelte';
import type {
	DashboardPlacement,
	DashboardLayoutKind,
	DashboardSnapGrid
} from '../dashboard-model';
import type { createDashboardInteractionChrome } from './interaction-chrome.svelte';
import type { createDashboardInteractionGeometry } from './interaction-geometry.svelte';
import {
	type AnchorGuide,
	type FreeLayoutInteractionMode,
	type FreeLayoutResizeEdges,
	type FreeLayoutPreviewSet,
	type FreeLayoutDropTarget,
	type FreeLayoutInteraction
} from './contracts';
import {
	minFreeWidgetWidthRem,
	minFreeWidgetHeightRem,
	cloneFreeLayoutPreviewSet,
	resolveFreeLayoutPreview,
	buildPreviewSetFromTargets,
	sameCssValue,
	samePosition,
	anchorXFromAnchor,
	anchorYFromAnchor,
	freeLayoutRectFromPlacement,
	buildSelectionRect,
	buildFreeLayoutMovePreview,
	buildFreeLayoutResizePreview
} from './geometry';
import {
	getRootRemPixels,
	getViewportWidthPx,
	getViewportHeightPx,
	getClosestSurfaceMetrics
} from './dom';
import { createUiEditSession, sendSetParamIntent } from '../../../../store/ui-intents';
import { type NodeId, type ParamEventBehaviour, type UiNodeDto } from '../../../../types';
import { type CssValueData } from '../../../../css-value';
import { moveDashboardWidgetToSurface } from '../dashboard-actions';
interface DashboardFreeLayoutContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly parent: UiNodeDto | null;
	readonly placement: DashboardPlacement;
	readonly editMode: boolean;
	readonly isSelected: boolean;
	readonly parentLayoutKind: DashboardLayoutKind;
	readonly snapGrid: DashboardSnapGrid;
	readonly widgetElement: HTMLElement | null;
	readonly chrome: ReturnType<typeof createDashboardInteractionChrome>;
	readonly geometry: ReturnType<typeof createDashboardInteractionGeometry>;
	clearChrome(): void;
	selectWidgetNode(mode?: SelectionMode): void;
}
export function createDashboardFreeLayout(context: DashboardFreeLayoutContext) {
	let graph = $derived(context.graph);
	let liveNode = $derived(context.node);
	let parentNode = $derived(context.parent);
	let placement = $derived(context.placement);
	let editMode = $derived(context.editMode);
	let isSelected = $derived(context.isSelected);
	let parentLayoutKind = $derived(context.parentLayoutKind);
	let parentSnapGrid = $derived(context.snapGrid);
	let widgetShellElement = $derived(context.widgetElement);
	const getGraph = () => context.graph;
	const clearFreeLayoutInteractionChrome = () => context.clearChrome();
	const selectWidgetNode = (mode?: SelectionMode) => context.selectWidgetNode(mode);
	const {
		setFreeLayoutAncestorShells,
		setFreeLayoutHoverSurface,
		setSurfaceInsertionIndicator,
		clearSurfaceInsertionIndicator,
		publishWidgetMoveSurfacePreview
	} = context.chrome;
	const { buildFreeLayoutTargets, resolveFreeLayoutDropTargetAtPoint } = context.geometry;
	let freeLayoutInteraction = $state<FreeLayoutInteraction | null>(null);

	let freeLayoutPreview = $state<FreeLayoutPreviewSet | null>(null);

	let freeLayoutDropTarget = $state<FreeLayoutDropTarget | null>(null);

	let freeLayoutCommitFrame: number | null = null;

	let freeLayoutCommitInFlight = false;

	let pendingFreeLayoutCommit: FreeLayoutPreviewSet | null = null;

	let lastCommittedFreeLayoutPreview: FreeLayoutPreviewSet | null = null;

	let freeLayoutEditSession: ReturnType<typeof createUiEditSession> | null = null;

	const cancelScheduledFreeLayoutCommit = (): void => {
		if (typeof window !== 'undefined' && freeLayoutCommitFrame !== null) {
			window.cancelAnimationFrame(freeLayoutCommitFrame);
		}
		freeLayoutCommitFrame = null;
		pendingFreeLayoutCommit = null;
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
			emPx: surfaceMetrics.emPx,
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
			emPx: surfaceMetrics.emPx,
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
			emPx: surfaceMetrics.emPx,
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
			emPx: surfaceMetrics.emPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			surfaceScaleX: surfaceMetrics.scaleX,
			surfaceScaleY: surfaceMetrics.scaleY,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		freeLayoutPreview = buildPreviewSetFromTargets(liveNode.node_id, targets);
	};

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

	let supportsFreePlacement = $derived(editMode && parentLayoutKind === 'free');

	let currentNodeFreeLayoutPreview = $derived.by(() =>
		resolveFreeLayoutPreview(freeLayoutPreview, liveNode.node_id)
	);

	const currentSurfaceContext = () => {
		const rootRemPx = getRootRemPixels();
		const metrics = getClosestSurfaceMetrics(widgetShellElement, rootRemPx);
		return {
			rootRemPx,
			emPx: metrics.emPx,
			surfaceWidthPx: metrics.width,
			surfaceHeightPx: metrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
	};
	let widgetShellStyle = $derived.by(() => {
		if (!editMode || !supportsFreePlacement || !currentNodeFreeLayoutPreview) {
			return '';
		}
		const context = freeLayoutInteraction ?? currentSurfaceContext();
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
	const resetFreeDropTarget = () => {
		freeLayoutDropTarget = null;
	};
	onDestroy(() => {
		cancelScheduledFreeLayoutCommit();
		void finishFreeLayoutEditSession();
	});
	return {
		get freeLayoutInteraction() {
			return freeLayoutInteraction;
		},
		get supportsFreePlacement() {
			return supportsFreePlacement;
		},
		get widgetShellStyle() {
			return widgetShellStyle;
		},
		get anchorGuides() {
			return anchorGuides;
		},
		beginFreeLayoutMove,
		beginFreeLayoutResize,
		resetFreeDropTarget
	};
}
