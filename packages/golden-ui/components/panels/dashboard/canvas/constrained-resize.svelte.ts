import { onDestroy } from 'svelte';
import { appState } from '../../../../store/workbench.svelte';
import type { EditOperation } from '../../../../store/session/edit-operations';
import { createUiEditSession, sendResizeDashboardWidgetIntent } from '../../../../store/ui-intents';
import { cssValueToPx, pxToCssValue } from '../../../../css-value';
import type { CssValueData } from '../../../../css-value';
import { getDirectParamNode } from '../dashboard-model';
import { createFreeLayoutCommitter } from './free-layout-committer';
import { captureLayoutGesture } from './gesture-fence';
import type { DashboardOrderedLayoutContext } from './ordered-layout-context';
import type {
	ConstrainedResizeInteraction,
	FreeLayoutPreviewSet,
	FreeLayoutResizeEdges,
	LayoutPoint
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

export function createConstrainedResize(context: DashboardOrderedLayoutContext) {
	let interaction = $state<ConstrainedResizeInteraction | null>(null);
	let operation = $state.raw<EditOperation<ReturnType<typeof createFreeLayoutCommitter>> | null>(
		null
	);
	let sample: FreeLayoutPreviewSet | null = null;
	let nativePointer: number | null = null;
	let lastSize: CssValueData | null = null;
	const finish = async (cancel: boolean): Promise<boolean> => {
		const pending = operation;
		operation = null;
		sample = null;
		interaction = null;
		nativePointer = null;
		lastSize = null;
		if (!pending) return false;
		await (cancel ? pending.cancel() : pending.confirm());
		return true;
	};
	$effect(() => {
		if (!operation) return;
		void context.graph;
		if (!context.editMode || !operation.current) void finish(true);
	});
	const start = (
		origin: LayoutPoint,
		edges: FreeLayoutResizeEdges,
		element: EventTarget | null
	): boolean => {
		if (
			!context.editMode ||
			!context.widgetElement?.isConnected ||
			!['horizontal', 'vertical'].includes(context.parentLayoutKind)
		)
			return false;
		const axis = context.parentLayoutKind === 'horizontal' ? 'x' : 'y';
		const field = getDirectParamNode(
			context.graph,
			context.node,
			axis === 'x' ? 'width' : 'height'
		);
		const host = context.session;
		if (!host || field?.data.kind !== 'parameter' || field.data.param.read_only) return false;
		void finish(true);
		context.clearChrome();
		const rootRemPx = getRootRemPixels();
		const metrics = getClosestSurfaceMetrics(element, rootRemPx);
		const placement = context.placement;
		const startPreview = { position: placement.position, size: placement.size };
		const target = {
			nodeId: context.node.node_id,
			anchor: placement.anchor,
			positionParamNodeId: null,
			positionParamBehaviour: null,
			widthParamNodeId: axis === 'x' ? field.node_id : null,
			widthParamBehaviour: axis === 'x' ? field.data.param.event_behaviour : null,
			heightParamNodeId: axis === 'y' ? field.node_id : null,
			heightParamBehaviour: axis === 'y' ? field.data.param.event_behaviour : null,
			startPreview,
			startRect: { left: 0, top: 0, width: 0, height: 0 }
		};
		const initial: FreeLayoutPreviewSet = {
			primaryNodeId: context.node.node_id,
			previews: [{ target, preview: startPreview }]
		};
		const fence = captureLayoutGesture(context.graph, [target]);
		if (!fence) return false;
		const revision = host.graphTransitionRevision,
			widget = context.node.node_id;
		const opened = host.editOperations.open({
			current: () =>
				appState.session === host &&
				host.graphTransitionRevision === revision &&
				!host.graph.state.requiresResync &&
				fence.isCurrent(host.graph.state),
			build: (scope) => {
				const session = createUiEditSession(
					'Resize dashboard widget',
					'dashboard-resize',
					host,
					scope.applied
				);
				return createFreeLayoutCommitter(initial, {
					current: () => scope.current,
					begin: async () => {
						await scope.ready;
						if (scope.current) await session.begin();
					},
					end: () => session.end(),
					publish: (edits) => scope.publish(edits.map((edit) => ({ ...edit, enabled: true }))),
					send: async (edits) => {
						const value = edits[0]?.value;
						return (
							value?.kind === 'css_value' &&
							(await sendResizeDashboardWidgetIntent(
								widget,
								axis === 'x'
									? { width: { value: value.value, unit: value.unit } }
									: { height: { value: value.value, unit: value.unit } },
								host,
								scope.applied
							))
						);
					},
					frame: (callback) => {
						const id = requestAnimationFrame(callback);
						return () => cancelAnimationFrame(id);
					}
				});
			}
		});
		if (!opened) return false;
		operation = opened;
		sample = initial;
		const sizeContext = {
			rootRemPx,
			emPx: metrics.emPx,
			surfaceWidthPx: metrics.width,
			surfaceHeightPx: metrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const startSize = axis === 'x' ? placement.size.width : placement.size.height;
		const rendered = context.widgetElement.getBoundingClientRect();
		const explicit = axis === 'x' ? placement.sizeEnabled.width : placement.sizeEnabled.height;
		const initialPixels = explicit
			? cssValueToPx(startSize, axis, createCssUnitContext(sizeContext, axis))
			: axis === 'x'
				? rendered.width / Math.max(metrics.scaleX, 1e-6)
				: rendered.height / Math.max(metrics.scaleY, 1e-6);
		interaction = {
			...sizeContext,
			axis,
			sign: (axis === 'x' ? edges.left : edges.top) ? -1 : 1,
			startClient: [origin.x, origin.y],
			startSize,
			startSizePx: initialPixels,
			surfaceScaleX: metrics.scaleX,
			surfaceScaleY: metrics.scaleY
		};
		return true;
	};
	const update = (point: LayoutPoint): boolean => {
		if (!operation || !interaction || !sample) return false;
		if (!context.editMode || !operation.current) {
			void finish(true);
			return false;
		}
		const axis = interaction.axis;
		const scale = Math.max(
			axis === 'x' ? interaction.surfaceScaleX : interaction.surfaceScaleY,
			1e-6
		);
		const delta =
			(axis === 'x' ? point.x - interaction.startClient[0] : point.y - interaction.startClient[1]) /
			scale;
		const pixels = Math.max(
			(axis === 'x' ? minFreeWidgetWidthRem : minFreeWidgetHeightRem) * interaction.rootRemPx,
			interaction.startSizePx + interaction.sign * delta
		);
		const size = pxToCssValue(
			pixels,
			interaction.startSize,
			axis,
			createCssUnitContext(interaction, axis)
		);
		if (lastSize && sameCssValue(lastSize, size)) return true;
		lastSize = size;
		const current = sample.previews[0];
		operation.work.schedule({
			...sample,
			previews: [
				{
					target: current.target,
					preview: {
						...current.preview,
						size: { ...current.preview.size, [axis === 'x' ? 'width' : 'height']: size }
					}
				}
			]
		});
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
		document.body.style.cursor = interaction.axis === 'x' ? 'ew-resize' : 'ns-resize';
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
		begin(event: PointerEvent, edges: FreeLayoutResizeEdges) {
			if (start({ x: event.clientX, y: event.clientY }, edges, event.currentTarget)) {
				nativePointer = event.pointerId;
				event.preventDefault();
				event.stopPropagation();
			}
		}
	};
}
