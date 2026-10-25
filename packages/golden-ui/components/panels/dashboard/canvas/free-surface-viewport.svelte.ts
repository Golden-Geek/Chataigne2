import { onDestroy, tick } from 'svelte';
import type { GraphState } from '../../../../store/graph.svelte';
import type { NodeId, UiNodeDto } from '../../../../types';
import type { DashboardViewContext } from './view-context.svelte';
import { FreeSurfaceGeometry } from './free-surface-geometry';
import { getRootRemPixels, getElementEmPixels } from './dom';

interface FreeSurfaceViewportContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly enabled: boolean;
	readonly element: HTMLElement | null;
	readonly view: DashboardViewContext | null;
}

const directWidgetShell = (layout: HTMLElement, target: Node | null): HTMLElement | null => {
	const element = target instanceof Element ? target : target?.parentElement;
	let shell = element?.closest<HTMLElement>('.dashboard-widget-shell') ?? null;
	while (shell && layout.contains(shell)) {
		if (shell.parentElement?.parentElement?.parentElement === layout) return shell;
		shell = shell.parentElement?.closest<HTMLElement>('.dashboard-widget-shell') ?? null;
	}
	return null;
};

export function createFreeSurfaceViewport(context: FreeSurfaceViewportContext) {
	const geometry = new FreeSurfaceGeometry();
	let epoch = $state(0);
	let sequence = 0;
	let mounted = $state.raw<UiNodeDto[]>([]);
	let queuedRefresh = false;
	let destroyed = false;
	let pendingFocus: HTMLElement | null = null;
	const pointerTargets = new Map<number, { target: Node; id: NodeId; uuid: string }>();
	// The DOM can disappear before this effect observes the corresponding graph removal.
	let rememberedFocus: { id: NodeId; uuid: string; element: HTMLElement } | null = null;
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
		pointerTargets.clear();
	});

	$effect(() => {
		const element = context.element;
		const view = context.view;
		if (!context.enabled || !element || !view) return;
		const identity = context.node.uuid;
		const unregister = view.register(context.node.node_id, { element, geometry });
		const retainPointer = (event: PointerEvent) => {
			if (!(event.target instanceof Node)) return;
			const shell = directWidgetShell(element, event.target);
			const id = shell ? Number(shell.dataset.nodeId) : undefined;
			const owner = id === undefined ? undefined : geometry.getNode(id);
			if (owner)
				pointerTargets.set(event.pointerId, {
					target: event.target,
					id: owner.node_id,
					uuid: owner.uuid
				});
			refresh();
		};
		const releasePointer = (event: PointerEvent) => {
			pointerTargets.delete(event.pointerId);
			refresh();
		};
		const releasePointers = () => {
			pointerTargets.clear();
			refresh();
		};
		window.addEventListener('pointerdown', retainPointer, true);
		window.addEventListener('pointerup', releasePointer, true);
		window.addEventListener('pointercancel', releasePointer, true);
		window.addEventListener('blur', releasePointers);
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
		const events = ['resize', 'scroll', 'focusin', 'focusout', 'selectionchange'] as const;
		for (const event of events) window.addEventListener(event, refresh, true);
		return () => {
			unregister();
			if (!context.enabled || context.element !== element || context.node.uuid !== identity)
				pointerTargets.clear();
			window.removeEventListener('pointerdown', retainPointer, true);
			window.removeEventListener('pointerup', releasePointer, true);
			window.removeEventListener('pointercancel', releasePointer, true);
			window.removeEventListener('blur', releasePointers);
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
			geometry.clear();
			pendingFocus = null;
			rememberedFocus = null;
			return;
		}
		const bounds = element.getBoundingClientRect();
		const width = element.clientWidth;
		const height = element.clientHeight;
		if (!(bounds.width > 0 && bounds.height > 0 && width > 0 && height > 0)) {
			mounted = [];
			return;
		}
		const rem = getRootRemPixels();
		geometry.sync(graph, node, {
			rootRemPx: rem,
			emPx: getElementEmPixels(element, rem),
			surfaceWidthPx: width,
			surfaceHeightPx: height,
			viewportWidthPx: window.innerWidth,
			viewportHeightPx: window.innerHeight
		});
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
		const visible =
			right > left && bottom > top
				? geometry.query({
						left: (left - bounds.left) / scaleX - overscan,
						top: (top - bounds.top) / scaleY - overscan,
						width: (right - left) / scaleX + 2 * overscan,
						height: (bottom - top) / scaleY + 2 * overscan
					})
				: [];
		const ids = new Set(visible);
		for (const id of view.retainedFor(node.node_id)) if (geometry.getNode(id)) ids.add(id);
		const selector = ':scope > .dashboard-slot > .dashboard-slot-fill > .dashboard-widget-shell';
		const ownShell = (target: Node | null) => directWidgetShell(element, target);
		const retain = (shell: HTMLElement | null) => {
			if (!shell) return;
			const id = Number(shell.dataset.nodeId);
			if (geometry.getNode(id)) ids.add(id);
		};
		const focusedElement =
			document.activeElement instanceof HTMLElement && ownShell(document.activeElement)
				? document.activeElement
				: pendingFocus;
		const focusedShell = ownShell(focusedElement);
		retain(focusedShell);
		for (const pointer of pointerTargets.values()) {
			if (geometry.getNode(pointer.id)?.uuid === pointer.uuid) retain(ownShell(pointer.target));
		}
		if (focusedElement && focusedShell && rememberedFocus?.element !== focusedElement) {
			const id = Number(focusedShell.dataset.nodeId);
			const item = geometry.getNode(id);
			if (item) rememberedFocus = { id, uuid: item.uuid, element: focusedElement };
		}
		if (
			!focusedElement &&
			rememberedFocus &&
			(rememberedFocus.element.isConnected ||
				geometry.getNode(rememberedFocus.id)?.uuid === rememberedFocus.uuid)
		)
			rememberedFocus = null;
		const retiredFocus =
			rememberedFocus && geometry.getNode(rememberedFocus.id)?.uuid !== rememberedFocus.uuid
				? rememberedFocus
				: null;
		for (const shell of element.querySelectorAll<HTMLElement>(selector + '.free-layout-active'))
			retain(shell);
		const selection = window.getSelection();
		if (selection && !selection.isCollapsed) {
			const nextSelection = new Set<NodeId>();
			for (const endpoint of [selection.anchorNode, selection.focusNode]) {
				const shell = ownShell(endpoint);
				if (shell) nextSelection.add(Number(shell.dataset.nodeId));
			}
			for (const shell of element.querySelectorAll<HTMLElement>(selector)) {
				if (nextSelection.size >= 64) break;
				if (selection.containsNode(shell, true)) nextSelection.add(Number(shell.dataset.nodeId));
			}
			for (const id of nextSelection) if (geometry.getNode(id)) ids.add(id);
		}
		// Query order is authored paint order. Retained offscreen rows keep that same order.
		mounted = geometry.orderedNodes(ids);
		if (retiredFocus) {
			pendingFocus = null;
			rememberedFocus = null;
			void tick().then(() => {
				if (
					!destroyed &&
					element.isConnected &&
					(document.activeElement === document.body ||
						document.activeElement === retiredFocus.element)
				)
					view.viewport?.focus({ preventScroll: true });
			});
		} else if (focusedElement) {
			pendingFocus = focusedElement;
			void tick().then(() => {
				if (pendingFocus !== focusedElement) return;
				pendingFocus = null;
				if (!destroyed && focusedElement.isConnected && document.activeElement === document.body)
					focusedElement.focus({ preventScroll: true });
			});
		}
	});

	return {
		geometry,
		get mounted() {
			return mounted;
		}
	};
}
