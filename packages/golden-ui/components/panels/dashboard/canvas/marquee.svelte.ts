import type { WorkbenchSession } from '../../../../store/workbench.svelte';
import { marqueeDragThresholdPx } from './dom';
import {
	type MarqueeSelectionState,
	type SurfacePressState,
	type FreeLayoutRect
} from './contracts';
import { normalizeClientRect, rectsIntersect } from './geometry';
import {
	selectionModeFromEvent,
	isInteractiveSurfaceTarget,
	focusClosestPageViewport
} from './dom';
import { type SelectionMode } from '../../../../store/workbench.svelte';
import { type NodeId, type UiNodeDto } from '../../../../types';
interface DashboardMarqueeContext {
	readonly node: UiNodeDto;
	readonly session: WorkbenchSession | null;
	readonly editMode: boolean;
	readonly children: readonly UiNodeDto[];
	readonly viewport: HTMLDivElement | null;
	readonly transforming: boolean;
	queryRect(surfaceId: NodeId, rect: FreeLayoutRect): NodeId[] | null;
}
type SelectionRegion = Omit<MarqueeSelectionState, 'pointerId'>;
export function createDashboardMarquee(context: DashboardMarqueeContext) {
	let liveNode = $derived(context.node);
	let session = $derived(context.session);
	let editMode = $derived(context.editMode);
	let childWidgets = $derived(context.children);
	let pageViewportElement = $derived(context.viewport);
	let transforming = $derived(context.transforming);
	let isPage = $derived(liveNode.node_type === 'dashboard_page');
	let isLayoutSurface = $derived(isPage || liveNode.node_type === 'dashboard_widget_container');
	let marqueeSelection = $state<MarqueeSelectionState | null>(null);

	let surfacePress = $state<SurfacePressState | null>(null);

	const collectMarqueeNodeIds = (selection: SelectionRegion): NodeId[] => {
		const selectionRect = normalizeClientRect(selection.startClient, selection.currentClient);
		const logical = context.queryRect(selection.surfaceNodeId, selectionRect);
		if (logical !== null) return logical;
		const childNodeIdByString = new Map(
			childWidgets.map((child) => [String(child.node_id), child.node_id])
		);
		const selector = `.dashboard-widget-shell[data-dashboard-surface-id="${String(selection.surfaceNodeId)}"]`;
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

	const applyMarqueeSelection = (selection: SelectionRegion): void => {
		const intersectingNodeIds = collectMarqueeNodeIds(selection);
		let nextSelection = intersectingNodeIds;
		if (selection.selectionMode === 'ADD') {
			nextSelection = [...new Set([...selection.baseSelection, ...intersectingNodeIds])];
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

	const handleSurfacePointerDown = (event: PointerEvent): void => {
		focusClosestPageViewport(event.currentTarget);
		if (!editMode || !isLayoutSurface || event.button !== 0) {
			return;
		}
		if (transforming || marqueeSelection || surfacePress) {
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
		if (transforming || marqueeSelection || surfacePress) {
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

	return {
		selectInRect: (element: HTMLElement, rect: FreeLayoutRect, selectionMode: SelectionMode) => {
			applyMarqueeSelection({
				surfaceElement: element,
				surfaceNodeId: liveNode.node_id,
				startClient: [rect.left, rect.top],
				currentClient: [rect.left + rect.width, rect.top + rect.height],
				baseSelection: [...(session?.selectedNodesIds ?? [])],
				selectionMode
			});
		},
		get marqueeSelection() {
			return marqueeSelection;
		},
		get marqueeStyle() {
			return marqueeStyle;
		},
		handleSurfacePointerDown,
		handlePageScenePointerDown
	};
}
