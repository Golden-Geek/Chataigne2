import { onDestroy, tick } from 'svelte';
import type { NodeId, UiNodeDto } from '../../../../types';

interface SurfaceRetentionContext {
	readonly element: HTMLElement | null;
	readonly identity: string;
	readonly viewport: HTMLElement | null;
	getNode(id: NodeId): UiNodeDto | undefined;
}

export function directWidgetShell(surface: HTMLElement, target: Node | null): HTMLElement | null {
	const element = target instanceof Element ? target : target?.parentElement;
	const owner = surface.closest('.dashboard-widget-shell');
	let shell = element?.closest<HTMLElement>('.dashboard-widget-shell') ?? null;
	while (shell && surface.contains(shell)) {
		const parent = shell.parentElement?.closest<HTMLElement>('.dashboard-widget-shell') ?? null;
		if (parent === owner) return shell;
		shell = parent;
	}
	return null;
}

const directItem = (surface: HTMLElement, target: Node | null): HTMLElement | null => {
	const shell = directWidgetShell(surface, target);
	if (shell) return shell;
	const element = target instanceof Element ? target : target?.parentElement;
	const header = element?.closest<HTMLElement>('[data-dashboard-child-id]') ?? null;
	return header?.closest('.dashboard-surface') === surface ? header : null;
};
const itemId = (element: HTMLElement): NodeId =>
	Number(element.dataset.nodeId ?? element.dataset.dashboardChildId);

/** One retention policy for free, flow, tab and accordion presentation changes. */
export function createSurfaceRetention(context: SurfaceRetentionContext) {
	let epoch = $state(0);
	let sequence = 0;
	let queued = false;
	let destroyed = false;
	let pendingFocus: HTMLElement | null = null;
	let rememberedFocus: { id: NodeId; uuid: string; element: HTMLElement } | null = null;
	const pointers = new Map<number, { target: Node; id: NodeId; uuid: string }>();
	const refresh = () => {
		if (queued || destroyed) return;
		queued = true;
		queueMicrotask(() => {
			queued = false;
			if (!destroyed) epoch = ++sequence;
		});
	};
	onDestroy(() => {
		destroyed = true;
		pointers.clear();
	});
	$effect(() => {
		const element = context.element,
			identity = context.identity;
		if (!element) return;
		const down = (event: PointerEvent) => {
			if (!(event.target instanceof Node)) return;
			const shell = directItem(element, event.target);
			const owner = shell ? context.getNode(itemId(shell)) : undefined;
			if (owner)
				pointers.set(event.pointerId, {
					target: event.target,
					id: owner.node_id,
					uuid: owner.uuid
				});
			refresh();
		};
		const up = (event: PointerEvent) => {
			pointers.delete(event.pointerId);
			refresh();
		};
		const blur = () => {
			pointers.clear();
			refresh();
		};
		window.addEventListener('pointerdown', down, true);
		window.addEventListener('pointerup', up, true);
		window.addEventListener('pointercancel', up, true);
		window.addEventListener('blur', blur);
		for (const event of ['focusin', 'focusout', 'selectionchange'])
			window.addEventListener(event, refresh, true);
		return () => {
			window.removeEventListener('pointerdown', down, true);
			window.removeEventListener('pointerup', up, true);
			window.removeEventListener('pointercancel', up, true);
			window.removeEventListener('blur', blur);
			for (const event of ['focusin', 'focusout', 'selectionchange'])
				window.removeEventListener(event, refresh, true);
			if (context.element !== element || context.identity !== identity) {
				pointers.clear();
				pendingFocus = null;
				rememberedFocus = null;
			}
		};
	});
	return {
		read(): ReadonlySet<NodeId> {
			void epoch;
			const element = context.element;
			const ids = new Set<NodeId>();
			if (!element) return ids;
			const ownShell = (target: Node | null) => directItem(element, target);
			const retain = (shell: HTMLElement | null) => {
				if (!shell) return;
				const id = itemId(shell);
				if (context.getNode(id)) ids.add(id);
			};
			const active = document.activeElement;
			const focused = active instanceof HTMLElement && ownShell(active) ? active : pendingFocus;
			const shell = ownShell(focused);
			retain(shell);
			for (const pointer of pointers.values())
				if (context.getNode(pointer.id)?.uuid === pointer.uuid) retain(ownShell(pointer.target));
			if (focused && shell && rememberedFocus?.element !== focused) {
				const item = context.getNode(itemId(shell));
				if (item) rememberedFocus = { id: item.node_id, uuid: item.uuid, element: focused };
			}
			if (
				!focused &&
				rememberedFocus &&
				(rememberedFocus.element.isConnected ||
					context.getNode(rememberedFocus.id)?.uuid === rememberedFocus.uuid)
			)
				rememberedFocus = null;
			const retired =
				rememberedFocus && context.getNode(rememberedFocus.id)?.uuid !== rememberedFocus.uuid
					? rememberedFocus
					: null;
			const shells = [
				...element.querySelectorAll<HTMLElement>(
					'.dashboard-widget-shell, [data-dashboard-child-id]'
				)
			].filter((item) => ownShell(item) === item);
			for (const item of shells) if (item.classList.contains('free-layout-active')) retain(item);
			const selection = window.getSelection();
			if (selection && !selection.isCollapsed) {
				const selected = new Set<NodeId>();
				for (const endpoint of [selection.anchorNode, selection.focusNode]) {
					const item = ownShell(endpoint);
					if (item) selected.add(itemId(item));
				}
				for (const item of shells) {
					if (selected.size >= 64) break;
					if (selection.containsNode(item, true)) selected.add(itemId(item));
				}
				for (const id of selected) if (context.getNode(id)) ids.add(id);
			}
			if (retired) {
				pendingFocus = null;
				rememberedFocus = null;
				void tick().then(() => {
					if (
						!destroyed &&
						element.isConnected &&
						(document.activeElement === document.body || document.activeElement === retired.element)
					)
						context.viewport?.focus({ preventScroll: true });
				});
			} else if (focused) {
				pendingFocus = focused;
				void tick().then(() => {
					if (pendingFocus !== focused) return;
					pendingFocus = null;
					if (!destroyed && focused.isConnected && document.activeElement === document.body)
						focused.focus({ preventScroll: true });
				});
			} else if (active instanceof HTMLElement && element.contains(active)) {
				// Header/browser controls can retire without belonging to a widget shell.
				void tick().then(() => {
					if (
						!destroyed &&
						element.isConnected &&
						!active.isConnected &&
						document.activeElement === document.body
					)
						context.viewport?.focus({ preventScroll: true });
				});
			}
			return ids;
		}
	};
}
