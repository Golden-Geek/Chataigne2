import { onDestroy, tick } from 'svelte';
import type { GraphState } from '../../../../store/graph.svelte';
import type { ParameterPreviews } from '../../../../store/session/edit-operations';
import type { NodeId, UiNodeDto } from '../../../../types';
import type { DashboardViewContext } from './view-context.svelte';
import { FreeSurfaceGeometry } from './free-surface-geometry';
import { getRootRemPixels, getElementEmPixels } from './dom';
import { rectsIntersect } from './geometry';
import type { FreeLayoutRect } from './contracts';
import type { FreeSurfaceGroup } from './free-surface-groups';

export interface FreeSurfaceOverview {
	readonly rect: FreeLayoutRect;
	readonly groups: readonly FreeSurfaceGroup[];
	readonly total: number;
	readonly scale: number;
}

interface FreeSurfaceViewportContext {
	readonly previews: ParameterPreviews | null;
	readonly budget: number;
	retained(): ReadonlySet<NodeId>;
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly enabled: boolean;
	readonly element: HTMLElement | null;
	readonly view: DashboardViewContext | null;
}

export function createFreeSurfaceViewport(context: FreeSurfaceViewportContext) {
	const geometry = new FreeSurfaceGeometry();
	let epoch = $state(0);
	let sequence = 0;
	let mounted = $state.raw<UiNodeDto[]>([]);
	let overview = $state.raw<FreeSurfaceOverview | null>(null);
	let queuedRefresh = false;
	let destroyed = false;
	const refresh = () => {
		// Focus events also fire during keyed DOM moves, when reactive writes are forbidden.
		if (queuedRefresh || destroyed) return;
		queuedRefresh = true;
		queueMicrotask(() => {
			queuedRefresh = false;
			if (!destroyed) epoch = ++sequence;
		});
	};
	onDestroy(() => {
		destroyed = true;
	});

	$effect(() => {
		const element = context.element;
		const view = context.view;
		if (!context.enabled || !element || !view) return;
		const unregister = view.register(context.node.node_id, { element, geometry });
		const observer = new ResizeObserver(refresh);
		observer.observe(element);
		const fontObserver = new MutationObserver(refresh);
		fontObserver.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['class', 'style']
		});
		fontObserver.observe(document.body, { attributes: true, attributeFilter: ['class', 'style'] });
		fontObserver.observe(element, { attributes: true, attributeFilter: ['class', 'style'] });
		for (
			let ancestor = element.parentElement;
			ancestor && ancestor !== document.body;
			ancestor = ancestor.parentElement
		)
			fontObserver.observe(ancestor, { attributes: true, attributeFilter: ['class', 'style'] });
		const events = ['resize', 'scroll'] as const;
		for (const event of events) window.addEventListener(event, refresh, true);
		return () => {
			unregister();
			observer.disconnect();
			fontObserver.disconnect();
			for (const event of events) window.removeEventListener(event, refresh, true);
		};
	});

	$effect(() => {
		void epoch;
		const view = context.view;
		void view?.camera;
		const element = context.element;
		const graph = context.graph;
		const node = context.node;
		if (!context.enabled || !element || !view?.viewport || !graph || !view.ready) {
			mounted = [];
			overview = null;
			geometry.clear();
			return;
		}
		const bounds = element.getBoundingClientRect();
		const width = element.clientWidth;
		const height = element.clientHeight;
		if (!(bounds.width > 0 && bounds.height > 0 && width > 0 && height > 0)) {
			mounted = [];
			overview = null;
			return;
		}
		const rem = getRootRemPixels();
		geometry.sync(
			graph,
			node,
			{
				rootRemPx: rem,
				emPx: getElementEmPixels(element, rem),
				surfaceWidthPx: width,
				surfaceHeightPx: height,
				viewportWidthPx: window.innerWidth,
				viewportHeightPx: window.innerHeight
			},
			context.previews
		);
		const scaleX = bounds.width / width;
		const scaleY = bounds.height / height;
		const viewport = view.viewport.getBoundingClientRect();
		let left = viewport.left,
			top = viewport.top,
			right = viewport.right,
			bottom = viewport.bottom;
		for (
			let ancestor = element.parentElement;
			ancestor && ancestor !== view.viewport;
			ancestor = ancestor.parentElement
		) {
			const style = getComputedStyle(ancestor);
			const rect = ancestor.getBoundingClientRect();
			if (style.overflowX !== 'visible') {
				left = Math.max(left, rect.left);
				right = Math.min(right, rect.right);
			}
			if (style.overflowY !== 'visible') {
				top = Math.max(top, rect.top);
				bottom = Math.min(bottom, rect.bottom);
			}
		}
		const overscan = 2 * rem;
		const visibleRect: FreeLayoutRect = {
			left: (left - bounds.left) / scaleX,
			top: (top - bounds.top) / scaleY,
			width: Math.max(0, right - left) / scaleX,
			height: Math.max(0, bottom - top) / scaleY
		};
		const queryRect = {
			left: visibleRect.left - overscan,
			top: visibleRect.top - overscan,
			width: visibleRect.width + 2 * overscan,
			height: visibleRect.height + 2 * overscan
		};
		const presentation =
			right > left && bottom > top
				? geometry.presentation(queryRect, context.budget)
				: { overview: false, ids: [], groups: [] };
		const active = document.activeElement;
		const retiringOverview =
			!presentation.overview &&
			active instanceof HTMLElement &&
			active.classList.contains('dashboard-free-overview') &&
			active.parentElement === element;
		overview = presentation.overview
			? { rect: visibleRect, groups: presentation.groups, total: geometry.size, scale: scaleY }
			: null;
		if (retiringOverview)
			void tick().then(() => {
				if (!destroyed && element.isConnected && document.activeElement === document.body)
					view.viewport?.focus({ preventScroll: true });
			});
		const ids = new Set(presentation.ids);
		if (presentation.overview) {
			// Logical bulk selection remains intact; only its latest small cohort needs controls.
			for (const id of view.selected.slice(-16)) {
				const rect = geometry.get(id);
				if (rect && rectsIntersect(queryRect, rect)) ids.add(id);
			}
		}
		// Direct free children already have logical bounds; only missing containers need mounting.
		for (const id of view.retainedFor(node.node_id))
			if (geometry.getNode(id)?.node_type === 'dashboard_widget_container') ids.add(id);
		for (const id of context.retained()) if (geometry.getNode(id)) ids.add(id);
		mounted = geometry.orderedNodes(ids);
	});

	return {
		geometry,
		get mounted() {
			return mounted;
		},
		get overview() {
			return overview;
		}
	};
}
