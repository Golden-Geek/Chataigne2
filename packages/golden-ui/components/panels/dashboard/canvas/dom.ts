import type { SelectionMode } from '../../../../store/workbench.svelte';
import type { NodeId } from '../../../../types';
import type { DashboardLayoutKind } from '../dashboard-model';
import type { SurfaceInsertionIndicator, SurfaceMetrics } from './contracts';

export const marqueeDragThresholdPx = 4;

export const getRootRemPixels = (): number => {
	if (typeof window === 'undefined') {
		return 16;
	}
	const fontSize = Number.parseFloat(window.getComputedStyle(document.documentElement).fontSize);
	return Number.isFinite(fontSize) && fontSize > 0 ? fontSize : 16;
};

export const getViewportWidthPx = (): number =>
	typeof window === 'undefined' ? 1280 : Math.max(window.innerWidth, 1);

export const getViewportHeightPx = (): number =>
	typeof window === 'undefined' ? 720 : Math.max(window.innerHeight, 1);

export const getClosestSurfaceMetrics = (
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

export const getElementSurfaceMetrics = (
	element: HTMLElement,
	rootRemPx: number
): SurfaceMetrics => {
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

export const getDirectDashboardLayoutElement = (
	surface: EventTarget | null
): HTMLElement | null => {
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

export const selectionModeFromEvent = (
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

export const isInteractiveSurfaceTarget = (target: EventTarget | null): boolean => {
	if (!(target instanceof Element)) {
		return false;
	}
	return Boolean(
		target.closest(
			'button, input, textarea, select, option, label, a, [contenteditable="true"], [role="button"]'
		)
	);
};

export const focusClosestPageViewport = (target: EventTarget | null): void => {
	if (!(target instanceof Element)) {
		return;
	}
	const viewport = target.closest('.dashboard-page-viewport');
	if (viewport instanceof HTMLElement) {
		viewport.focus();
	}
};

export const pointFallsWithinElement = (
	element: HTMLElement,
	clientX: number,
	clientY: number
): boolean => {
	const rect = element.getBoundingClientRect();
	return (
		clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom
	);
};

export const resolveOwnedDashboardSurfaceElement = (
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

export const resolveDashboardSurfaceAtPoint = (
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

export const currentSurfaceOwnsDragEvent = (event: DragEvent): boolean => {
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

export const resolveSurfaceContentElement = (
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

export const buildVerticalInsertionIndicator = (
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

export const buildHorizontalInsertionIndicator = (
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

export const canScrollAxis = (element: HTMLElement, axis: 'x' | 'y', delta: number): boolean => {
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

export const withStoppedPropagation = (event: Event): void => {
	event.stopPropagation();
};
