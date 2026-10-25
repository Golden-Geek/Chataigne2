import { onDestroy } from 'svelte';
import { createFreeLayoutCommitter } from './free-layout-committer';
import { captureLayoutGesture } from './gesture-fence';
import type { GraphState } from '../../../../store/graph.svelte';
import type { SelectionMode, WorkbenchSession } from '../../../../store/workbench.svelte';
import type { EditOperation } from '../../../../store/session/edit-operations';
import { appState } from '../../../../store/workbench.svelte';
import type {
	DashboardPlacement,
	DashboardLayoutKind,
	DashboardSnapGrid
} from '../dashboard-model';
import type { createDashboardInteractionChrome } from './interaction-chrome.svelte';
import type { createDashboardInteractionGeometry } from './interaction-geometry.svelte';
import {
	type AnchorGuide,
	type LayoutPoint,
	type FreeLayoutEdit,
	type FreeLayoutInteractionMode,
	type FreeLayoutResizeEdges,
	type FreeLayoutPreviewSet,
	type FreeLayoutDropTarget,
	type FreeLayoutInteraction
} from './contracts';
import {
	minFreeWidgetWidthRem,
	minFreeWidgetHeightRem,
	resolveFreeLayoutPreview,
	buildPreviewSetFromTargets,
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
import { createUiEditSession, sendUiIntentBatch } from '../../../../store/ui-intents';
import { type UiNodeDto } from '../../../../types';
import { moveDashboardWidgetToSurface } from '../dashboard-actions';
interface DashboardFreeLayoutContext {
	readonly session: WorkbenchSession | null;
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

	type Gesture = {
		primary: number;
		host: WorkbenchSession;
		scope: EditOperation<ReturnType<typeof createFreeLayoutCommitter>>;
	};
	const gestures = new Set<Gesture>();
	let operationCount = $state(0);
	let gesture: Gesture | null = null;
	let destroyed = false;
	let nativePointerId: number | null = null;
	const closeGesture = (operation: Gesture, cancel: boolean, after?: () => Promise<void>) => {
		const done = cancel ? operation.scope.cancel() : operation.scope.confirm(after);
		const remove = () => {
			gestures.delete(operation);
			if (!destroyed) operationCount = gestures.size;
		};
		void done.then(remove, remove);
		return done;
	};
	const cancelActiveGesture = () => {
		nativePointerId = null;
		const operation = gesture;
		gesture = null;
		freeLayoutInteraction = null;
		freeLayoutPreview = null;
		return operation ? closeGesture(operation, true) : Promise.resolve();
	};
	const startCommitter = (initial: FreeLayoutPreviewSet, label: string): boolean => {
		const host = context.session;
		if (!host) return false;
		const fence = captureLayoutGesture(
			context.graph,
			initial.previews.map(({ target }) => target)
		);
		if (!fence) return false;
		const revision = host.graphTransitionRevision;
		const scope = host.editOperations.open({
			current: () =>
				appState.session === host &&
				host.graphTransitionRevision === revision &&
				!host.graph.state.requiresResync &&
				fence.isCurrent(host.graph.state),
			build: (ownership) => {
				const session = createUiEditSession(
					label,
					'dashboard-free-layout',
					host,
					ownership.applied
				);
				return createFreeLayoutCommitter(initial, {
					current: () => ownership.current,
					begin: async () => {
						await ownership.ready;
						if (ownership.current) await session.begin();
					},
					end: () => session.end(),
					send: async (edits) => (await sendUiIntentBatch(edits, host, ownership.applied)).success,
					publish: (edits) => ownership.publish(edits),
					frame: (callback) => {
						const id = requestAnimationFrame(callback);
						return () => cancelAnimationFrame(id);
					}
				});
			}
		});
		if (!scope) return false;
		gesture = { primary: initial.primaryNodeId, host, scope };
		gestures.add(gesture);
		operationCount = gestures.size;
		return true;
	};
	const cancelScheduledFreeLayoutCommit = () => gesture?.scope.work.discardPending();
	const scheduleFreeLayoutCommit = (preview: FreeLayoutPreviewSet) =>
		gesture?.scope.work.schedule(preview);
	$effect(() => {
		if (operationCount === 0) return;
		void context.graph;
		const enabled = context.editMode;
		for (const operation of gestures) {
			if (operation.scope.confirmed || (enabled && operation.scope.current)) continue;
			if (gesture === operation) {
				cancelActiveGesture();
				clearFreeLayoutInteractionChrome();
			} else void closeGesture(operation, true);
		}
	});
	const finalizeFreeLayoutInteraction = async (
		preview: FreeLayoutPreviewSet | null,
		mode: FreeLayoutInteractionMode | null
	): Promise<void> => {
		const operation = gesture,
			dropTarget = freeLayoutDropTarget;
		gesture = null;
		clearFreeLayoutInteractionChrome();
		if (!operation) return;
		if (dropTarget && mode === 'move') {
			operation.scope.work.discardPending();
			await closeGesture(operation, false, async () => {
				await moveDashboardWidgetToSurface(
					() => operation.host.graph.state,
					operation.primary,
					dropTarget.surfaceNodeId,
					dropTarget.placement,
					dropTarget.targetIndex ?? undefined
				);
			});
		} else {
			if (preview) operation.scope.work.schedule(preview);
			await closeGesture(operation, false);
		}
	};

	const startLayoutOperation = (
		mode: FreeLayoutInteractionMode,
		origin: LayoutPoint,
		resizeEdges: FreeLayoutResizeEdges | null,
		transformNodes: UiNodeDto[]
	): boolean => {
		if (!editMode || !supportsFreePlacement || !widgetShellElement?.isConnected) return false;
		cancelActiveGesture();
		clearFreeLayoutInteractionChrome();
		if (!isSelected) selectWidgetNode('REPLACE');
		setFreeLayoutAncestorShells(widgetShellElement);
		const rootRemPx = getRootRemPixels();
		const surfaceMetrics = getClosestSurfaceMetrics(widgetShellElement.parentElement, rootRemPx);
		const metrics = {
			rootRemPx,
			emPx: surfaceMetrics.emPx,
			surfaceWidthPx: surfaceMetrics.width,
			surfaceHeightPx: surfaceMetrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const targets = buildFreeLayoutTargets(transformNodes, metrics),
			selectionRect = buildSelectionRect(targets);
		if (!targets.length || !selectionRect) {
			clearFreeLayoutInteractionChrome();
			return false;
		}
		const initial = buildPreviewSetFromTargets(liveNode.node_id, targets);
		if (
			!startCommitter(
				initial,
				mode === 'move' ? 'Move dashboard widget' : 'Resize dashboard widget'
			)
		) {
			clearFreeLayoutInteractionChrome();
			return false;
		}
		freeLayoutInteraction = {
			mode,
			resizeEdges,
			primaryNodeId: liveNode.node_id,
			sourceSurfaceNodeId: parentNode?.node_id ?? liveNode.node_id,
			selectionRect,
			targets,
			startClient: [origin.x, origin.y],
			...metrics,
			surfaceScaleX: surfaceMetrics.scaleX,
			surfaceScaleY: surfaceMetrics.scaleY
		};
		freeLayoutPreview = initial;
		return true;
	};
	const beginFreeLayoutMove = (event: PointerEvent, nodes: UiNodeDto[]): void => {
		if (!startLayoutOperation('move', { x: event.clientX, y: event.clientY }, null, nodes)) return;
		nativePointerId = event.pointerId;
		event.preventDefault();
		event.stopPropagation();
	};
	const beginFreeLayoutResize = (
		event: PointerEvent,
		edges: FreeLayoutResizeEdges,
		nodes: UiNodeDto[]
	): void => {
		if (!startLayoutOperation('resize', { x: event.clientX, y: event.clientY }, edges, nodes))
			return;
		nativePointerId = event.pointerId;
		event.preventDefault();
		event.stopPropagation();
	};
	const updateLayout = (point: LayoutPoint): boolean => {
		if (!freeLayoutInteraction || !gesture) return false;
		if (!editMode || !gesture.scope.current) {
			cancelActiveGesture();
			clearFreeLayoutInteractionChrome();
			return false;
		}
		const deltaXPx =
			(point.x - freeLayoutInteraction.startClient[0]) /
			Math.max(freeLayoutInteraction.surfaceScaleX, 1e-6);
		const deltaYPx =
			(point.y - freeLayoutInteraction.startClient[1]) /
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
			const nextDropTarget = resolveFreeLayoutDropTargetAtPoint(point.x, point.y, nextPreview);
			freeLayoutDropTarget = nextDropTarget;
			setFreeLayoutHoverSurface(nextDropTarget?.element ?? null);
			setSurfaceInsertionIndicator(
				nextDropTarget?.element ?? null,
				nextDropTarget?.indicator ?? null
			);
			publishWidgetMoveSurfacePreview(nextDropTarget);
			if (nextDropTarget) {
				cancelScheduledFreeLayoutCommit();
				return true;
			}
			scheduleFreeLayoutCommit(nextPreview);
			return true;
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
			return true;
		}
		freeLayoutPreview = nextPreview;
		scheduleFreeLayoutCommit(nextPreview);
		return true;
	};
	const finishLayout = async (cancel: boolean): Promise<boolean> => {
		if (!freeLayoutInteraction || !gesture) return false;
		if (cancel || !editMode || !gesture.scope.current) {
			const closing = cancelActiveGesture();
			clearFreeLayoutInteractionChrome();
			await closing;
			return true;
		}
		const preview = freeLayoutPreview,
			mode = freeLayoutInteraction.mode;
		freeLayoutInteraction = null;
		freeLayoutPreview = null;
		nativePointerId = null;
		await finalizeFreeLayoutInteraction(preview, mode);
		return true;
	};
	const edit = async (command: FreeLayoutEdit, targets: () => UiNodeDto[]): Promise<boolean> => {
		switch (command.kind) {
			case 'move':
				return startLayoutOperation('move', command.origin, null, targets());
			case 'resize':
				return startLayoutOperation('resize', command.origin, command.edges, targets());
			case 'update':
				return updateLayout(command.point);
			case 'finish':
				return finishLayout(false);
			case 'cancel':
				return finishLayout(true);
		}
	};
	$effect(() => {
		if (!freeLayoutInteraction) return;
		const handlePointerMove = (event: PointerEvent) => {
			if (
				event.pointerId === nativePointerId &&
				updateLayout({ x: event.clientX, y: event.clientY })
			)
				event.preventDefault();
		};
		const finishInteraction = (event: PointerEvent) => {
			if (event.pointerId === nativePointerId) void finishLayout(event.type === 'pointercancel');
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
		destroyed = true;
		cancelActiveGesture();
		for (const operation of gestures) operation.scope.detach();
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
		edit,
		beginFreeLayoutMove,
		beginFreeLayoutResize,
		resetFreeDropTarget
	};
}
