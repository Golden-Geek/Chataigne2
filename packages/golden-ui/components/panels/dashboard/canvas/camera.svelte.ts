import { onDestroy } from 'svelte';
import type { DashboardFrameMeasurement } from './view-context.svelte';
import type { GraphState } from '../../../../store/graph.svelte';
import {
	type DashboardPersistedPageView,
	type DashboardPersistedPageViewChangeHandler,
	type DashboardPageViewState
} from './contracts';
import type { FreeLayoutRect } from './contracts';
import { minFreeWidgetWidthRem } from './geometry';
import { getRootRemPixels, canScrollAxis } from './dom';
import { clampPageZoom, sanitizePersistedPageView, samePersistedPageView } from './page-view';
import { registerCommandHandler } from '../../../../store/commands.svelte';
import { type NodeId, type UiNodeDto } from '../../../../types';
import { getDashboardPageSize } from '../dashboard-model';
interface DashboardCameraContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly editMode: boolean;
	readonly navigationEnabled: boolean;
	readonly persistedView: DashboardPersistedPageView | null;
	readonly onPersistedViewChange: DashboardPersistedPageViewChangeHandler | null;
	readonly viewport: HTMLDivElement | null;
	readonly frame: HTMLDivElement | null;
	readonly focused: boolean;
	readonly selectedNodeIds: readonly NodeId[];
	frameRects(ids: readonly NodeId[]): Promise<DashboardFrameMeasurement | null>;
}
export function createDashboardCamera(context: DashboardCameraContext) {
	let graph = $derived(context.graph);
	let liveNode = $derived(context.node);
	let isPage = $derived(liveNode.node_type === 'dashboard_page');
	let editMode = $derived(context.editMode);
	let pageNavigationEnabled = $derived(context.navigationEnabled);
	let persistedPageView = $derived(context.persistedView);
	let onPersistedPageViewChange = $derived(context.onPersistedViewChange);
	let pageViewportElement = $derived(context.viewport);
	let pageFrameElement = $derived(context.frame);
	let isPageViewportFocused = $derived(context.focused);
	let selectedNodeIds = $derived(context.selectedNodeIds);
	let pageViewportSize = $state({ width: 0, height: 0 });

	let pageViewportResizeFrame = $state<number | null>(null);

	let pendingPersistedPageView = $state<DashboardPersistedPageView | null>(null);

	let appliedPersistedPageView = $state<DashboardPersistedPageView | null>(null);

	let pageView = $state<DashboardPageViewState>({
		pointerId: null,
		lastClient: null,
		panX: 0,
		panY: 0,
		zoom: 1
	});

	let pageViewAnimationFrame = $state<number | null>(null);
	let animationCompletion: Promise<boolean> = Promise.resolve(true);
	let resolveAnimation: ((completed: boolean) => void) | null = null;
	let pendingFrame: number | null = null;

	let pageViewAnimationTarget = $state<DashboardPersistedPageView | null>(null);
	let navigationGeneration = 0;

	let previousPageFitScale = $state<number | null>(null);

	let previousPageFitScaleEditMode = $state<boolean | null>(null);

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
		navigationGeneration++;
		pendingFrame = null;
		resolveAnimation?.(false);
		resolveAnimation = null;
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
			animationCompletion = Promise.resolve(true);
			setPageViewInstantly(targetPageView);
			return;
		}
		const animationDurationMs = 220;
		animationCompletion = new Promise((resolve) => {
			resolveAnimation = resolve;
		});
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
				resolveAnimation?.(true);
				resolveAnimation = null;
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

	const frameWidgetRects = (
		widgetRects: FreeLayoutRect[],
		options: { animate?: boolean; fillRatio?: number; preserveZoom?: boolean } = {}
	): boolean => {
		const geometry = getPageFrameGeometry();
		if (!geometry) {
			return false;
		}
		if (widgetRects.length === 0) {
			homePageView(options);
			return true;
		}
		const fillRatio = Math.min(Math.max(options.fillRatio ?? 0.84, 0.1), 0.98);
		const { viewportRect, baseCenterX, baseCenterY } = geometry;
		const availableWidth = Math.max(1, viewportRect.width - 2 * pageEditBleedPx);
		const availableHeight = Math.max(1, viewportRect.height - 2 * pageEditBleedPx);
		let localLeft = Number.POSITIVE_INFINITY;
		let localRight = Number.NEGATIVE_INFINITY;
		let localTop = Number.POSITIVE_INFINITY;
		let localBottom = Number.NEGATIVE_INFINITY;
		for (const widgetRect of widgetRects) {
			localLeft = Math.min(localLeft, widgetRect.left - pageLogicalWidthPx / 2);
			localRight = Math.max(
				localRight,
				widgetRect.left + widgetRect.width - pageLogicalWidthPx / 2
			);
			localTop = Math.min(localTop, widgetRect.top - pageLogicalHeightPx / 2);
			localBottom = Math.max(
				localBottom,
				widgetRect.top + widgetRect.height - pageLogicalHeightPx / 2
			);
		}
		const localWidth = Math.max(1, localRight - localLeft);
		const localHeight = Math.max(1, localBottom - localTop);
		const targetScale = Math.min(
			(availableWidth * fillRatio) / localWidth,
			(availableHeight * fillRatio) / localHeight
		);
		const nextZoom = options.preserveZoom
			? pageView.zoom
			: clampPageZoom(targetScale / Math.max(pageFitScale, 1e-6));
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

	let pageSize = $derived(getDashboardPageSize(graph, liveNode));

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

	const frameSelectedWidgets = async (): Promise<boolean> => {
		const viewportElement = pageViewportElement;
		if (!viewportElement) {
			return false;
		}
		cancelPageViewAnimation();
		const generation = navigationGeneration;
		const measurement = await context.frameRects([...selectedNodeIds]);
		if (!measurement || generation !== navigationGeneration) return true;
		const handled = frameWidgetRects(measurement.rects, {
			animate: true,
			fillRatio: 0.78
		});
		if (!handled || !measurement.refine) return handled;
		const framingGeneration = navigationGeneration;
		pendingFrame = framingGeneration;
		try {
			if (!(await animationCompletion) || framingGeneration !== navigationGeneration) return true;
			const settled = await measurement.refine();
			if (settled && framingGeneration === navigationGeneration)
				frameWidgetRects(settled, { animate: false, preserveZoom: true });
			return true;
		} finally {
			if (pendingFrame === framingGeneration) pendingFrame = null;
		}
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
	onDestroy(() => {
		cancelPageViewAnimation();
		if (pageViewportResizeFrame !== null) cancelAnimationFrame(pageViewportResizeFrame);
	});
	return {
		get navigationState() {
			return {
				view: currentPersistedPageView(),
				animating: pageViewAnimationFrame !== null || pendingFrame !== null,
				ready: pageViewportSize.width > 1 && pageViewportSize.height > 1
			};
		},
		get logicalSize() {
			return { width: pageLogicalWidthPx, height: pageLogicalHeightPx };
		},
		get viewSignal() {
			return [
				editMode,
				selectedNodeIds,
				pageView.panX,
				pageView.panY,
				pageView.zoom,
				pageFitScale,
				pageViewportWidthPx,
				pageViewportHeightPx
			];
		},
		get viewportReady() {
			return pageViewportSize.width > 1 && pageViewportSize.height > 1;
		},
		get pageSize() {
			return pageSize;
		},
		get pageSceneStyle() {
			return pageSceneStyle;
		},
		get pageFrameStyle() {
			return pageFrameStyle;
		},
		get pageBleedFrameStyle() {
			return pageBleedFrameStyle;
		},
		handlePageViewportPointerDown,
		handlePageViewportWheel
	};
}
