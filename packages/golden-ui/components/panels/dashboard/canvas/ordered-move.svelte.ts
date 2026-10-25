import { onDestroy } from 'svelte';
import { appState } from '../../../../store/workbench.svelte';
import type { EditOperation } from '../../../../store/session/edit-operations';
import {
	moveDashboardWidgetToSurface,
	type DashboardWidgetCreationPlacement
} from '../dashboard-actions';
import { captureLayoutGesture } from './gesture-fence';
import { captureSurface } from './surface-fence';
import { marqueeDragThresholdPx } from './dom';
import type { DashboardOrderedLayoutContext } from './ordered-layout-context';
import type { LayoutPoint, WidgetMoveDropTarget, WidgetMoveInteraction } from './contracts';

interface MoveWork {
	readonly accepted: boolean;
	finish(): Promise<void>;
	cancel(): Promise<void>;
}
interface Destination {
	parent: number;
	index: number | undefined;
	placement: DashboardWidgetCreationPlacement;
}

export function createOrderedMove(context: DashboardOrderedLayoutContext) {
	let interaction = $state<WidgetMoveInteraction | null>(null);
	let drop = $state.raw<WidgetMoveDropTarget | null>(null);
	let operation = $state.raw<EditOperation<MoveWork> | null>(null);
	let setDestination: ((target: WidgetMoveDropTarget | null) => void) | null = null;
	let setCancelPreview: ((cancel: () => void) => void) | null = null;
	let nativePointer: number | null = null;
	const finish = async (cancel: boolean): Promise<boolean> => {
		const pending = operation,
			target = drop,
			source = interaction?.sourceSurfaceNodeId;
		if (!pending) return false;
		const noop =
			target &&
			target.surfaceNodeId === source &&
			target.targetIndex === context.ordering?.currentIndex;
		if (!cancel && target && !noop && pending.current) {
			const cancelPreview = context.chrome.publishWidgetMoveCommit(target, source);
			if (cancelPreview) setCancelPreview?.(cancelPreview);
		}
		operation = null;
		interaction = null;
		nativePointer = null;
		setDestination = null;
		setCancelPreview = null;
		context.clearChrome();
		drop = null;
		await (cancel || !target || noop ? pending.cancel() : pending.confirm());
		return true;
	};
	$effect(() => {
		if (!operation) return;
		void context.graph;
		if (!context.editMode || !operation.current) void finish(true);
	});
	const start = (origin: LayoutPoint, element: EventTarget | null): boolean => {
		const host = context.session,
			parent = context.parent;
		if (
			!host ||
			!parent ||
			!context.editMode ||
			context.parentLayoutKind === 'free' ||
			!context.widgetElement?.isConnected
		)
			return false;
		void finish(true);
		context.clearChrome();
		const widget = context.node.node_id,
			revision = host.graphTransitionRevision;
		const fence = captureLayoutGesture(context.graph, [
			{ nodeId: widget, positionParamNodeId: null, widthParamNodeId: null, heightParamNodeId: null }
		]);
		if (!fence) return false;
		let target: Destination | null = null,
			targetFence: ReturnType<typeof captureSurface> = null;
		let cancelPreview: (() => void) | null = null;
		const opened = host.editOperations.open({
			current: () =>
				appState.session === host &&
				host.graphTransitionRevision === revision &&
				!host.graph.state.requiresResync &&
				fence.isCurrent(host.graph.state) &&
				(!targetFence || targetFence.isCurrent(host.graph.state)),
			build: (scope) => {
				let completion: Promise<void> | null = null,
					accepted = false;
				return {
					get accepted() {
						return accepted;
					},
					finish() {
						return (completion ??= (async () => {
							await scope.ready;
							if (scope.current && target)
								accepted = await moveDashboardWidgetToSurface(
									() => host.graph.state,
									widget,
									target.parent,
									target.placement,
									target.index,
									host
								);
							if (!accepted) cancelPreview?.();
						})());
					},
					cancel() {
						cancelPreview?.();
						return completion ?? Promise.resolve();
					}
				};
			}
		});
		if (!opened) return false;
		setDestination = (next) => {
			target = next
				? {
						parent: next.surfaceNodeId,
						index: next.targetIndex ?? undefined,
						placement: structuredClone(next.placement)
					}
				: null;
			targetFence = next ? captureSurface(host.graph.state, next.surfaceNodeId) : null;
		};
		setCancelPreview = (callback) => {
			cancelPreview = callback;
		};
		operation = opened;
		interaction = { sourceSurfaceNodeId: parent.node_id, startClient: [origin.x, origin.y] };
		context.chrome.setFreeLayoutAncestorShells(element);
		return true;
	};
	const update = (point: LayoutPoint): boolean => {
		if (!interaction || !operation) return false;
		if (!context.editMode || !operation.current) {
			void finish(true);
			return false;
		}
		if (
			Math.hypot(point.x - interaction.startClient[0], point.y - interaction.startClient[1]) <
			marqueeDragThresholdPx
		)
			return true;
		drop = context.geometry.resolveWidgetMoveDropTargetAtPoint(point.x, point.y);
		setDestination?.(drop);
		context.chrome.setFreeLayoutHoverSurface(drop?.element ?? null);
		context.chrome.setSurfaceInsertionIndicator(drop?.element ?? null, drop?.indicator ?? null);
		context.chrome.publishWidgetMoveSurfacePreview(drop);
		return true;
	};
	$effect(() => {
		if (!interaction) return;
		const move = (event: PointerEvent) => {
			if (event.pointerId === nativePointer && update({ x: event.clientX, y: event.clientY }))
				event.preventDefault();
		};
		const end = (event: PointerEvent) => {
			if (event.pointerId === nativePointer) void finish(event.type === 'pointercancel');
		};
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', end);
		window.addEventListener('pointercancel', end);
		const cancel = () => {
			void finish(true);
		};
		window.addEventListener('blur', cancel);
		const cursor = document.body.style.cursor,
			select = document.body.style.userSelect;
		document.body.style.cursor = 'grabbing';
		document.body.style.userSelect = 'none';
		return () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', end);
			window.removeEventListener('pointercancel', end);
			window.removeEventListener('blur', cancel);
			document.body.style.cursor = cursor;
			document.body.style.userSelect = select;
		};
	});
	onDestroy(() => {
		void finish(true);
	});
	return {
		get interaction() {
			return interaction;
		},
		start,
		update,
		finish,
		resetDropTarget() {
			drop = null;
		},
		begin(event: PointerEvent) {
			if (start({ x: event.clientX, y: event.clientY }, event.currentTarget)) {
				nativePointer = event.pointerId;
				event.preventDefault();
				event.stopPropagation();
			}
		}
	};
}
