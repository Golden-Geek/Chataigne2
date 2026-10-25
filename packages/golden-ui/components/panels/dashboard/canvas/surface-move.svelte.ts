import type { GraphState } from '../../../../store/graph.svelte';
import type { DashboardLayoutKind } from '../dashboard-model';
import type { UiNodeDto } from '../../../../types';
import { captureWidgetMove } from './move-fence';
import { moveNodeIdInList, sameNodeIdList } from './geometry';
import {
	widgetMovePreviewEventName,
	widgetMoveCommitEventName,
	type SurfaceDropPreview,
	type PendingWidgetMoveCommit,
	type WidgetMovePreviewEventDetail,
	type WidgetMoveCommitEventDetail
} from './contracts';

interface SurfaceMoveContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly enabled: boolean;
	readonly layout: DashboardLayoutKind;
	readonly children: readonly UiNodeDto[];
}
type Fence = NonNullable<ReturnType<typeof captureWidgetMove>>;

/** Own move presentation independently of the backend transaction and native input lifetime. */
export function createSurfaceMove(context: SurfaceMoveContext) {
	let active = $state.raw<{
		preview: SurfaceDropPreview | null;
		hidden: number | null;
		fence: Fence;
		operation: object;
	} | null>(null);
	let pending = $state.raw<(PendingWidgetMoveCommit & { fence: Fence; operation: object }) | null>(
		null
	);
	$effect(() => {
		if (!context.enabled || typeof window === 'undefined') return;
		const preview = (event: Event) => {
			const detail = (event as CustomEvent<WidgetMovePreviewEventDetail>).detail;
			if (detail.surfaceNodeId === null) {
				if (active?.operation === detail.operation) active = null;
				return;
			}
			const id = context.node.node_id;
			if (detail.surfaceNodeId !== id && detail.sourceSurfaceNodeId !== id) {
				active = null;
				return;
			}
			const fence = captureWidgetMove(
				context.graph,
				detail.sourceWidgetNodeId,
				detail.sourceSurfaceNodeId,
				detail.surfaceNodeId
			);
			active =
				fence && fence.source.uuid === detail.sourceWidgetUuid
					? {
							preview: detail.surfaceNodeId === id ? detail.preview : null,
							hidden:
								context.layout !== 'free' && detail.sourceSurfaceNodeId === id
									? detail.sourceWidgetNodeId
									: null,
							fence,
							operation: detail.operation
						}
					: null;
		};
		const commit = (event: Event) => {
			const detail = (event as CustomEvent<WidgetMoveCommitEventDetail>).detail;
			if (detail.targetSurfaceNodeId === null) {
				if (pending?.operation === detail.operation) pending = null;
				return;
			}
			const id = context.node.node_id;
			if (detail.targetSurfaceNodeId !== id && detail.sourceSurfaceNodeId !== id) return;
			const fence = captureWidgetMove(
				context.graph,
				detail.sourceWidgetNodeId,
				detail.sourceSurfaceNodeId,
				detail.targetSurfaceNodeId
			);
			if (!fence || fence.source.uuid !== detail.sourceWidgetUuid) return;
			active = null;
			const children = context.children.map((child) => child.node_id);
			pending = {
				preview: detail.targetSurfaceNodeId === id ? detail.preview : null,
				hiddenChildNodeId:
					context.layout !== 'free' && detail.sourceSurfaceNodeId === id
						? detail.sourceWidgetNodeId
						: null,
				expectedChildIds:
					detail.targetSurfaceNodeId === id
						? moveNodeIdInList(
								children,
								detail.sourceWidgetNodeId!,
								detail.preview?.targetIndex ?? null
							)
						: children.filter((child) => child !== detail.sourceWidgetNodeId),
				fence,
				operation: detail.operation
			};
		};
		window.addEventListener(widgetMovePreviewEventName, preview);
		window.addEventListener(widgetMoveCommitEventName, commit);
		return () => {
			window.removeEventListener(widgetMovePreviewEventName, preview);
			window.removeEventListener(widgetMoveCommitEventName, commit);
		};
	});
	$effect(() => {
		if (active && (!context.enabled || !active.fence.isCurrent(context.graph))) active = null;
		if (
			pending &&
			(!context.enabled ||
				!pending.fence.isCurrent(context.graph) ||
				sameNodeIdList(
					context.children.map((child) => child.node_id),
					pending.expectedChildIds
				))
		)
			pending = null;
	});
	$effect(() => {
		if (!pending) return;
		const timeout = setTimeout(() => {
			pending = null;
		}, 2000);
		return () => clearTimeout(timeout);
	});
	return {
		get preview() {
			return active?.preview ?? pending?.preview ?? null;
		},
		get hidden() {
			return active?.hidden ?? pending?.hiddenChildNodeId ?? null;
		}
	};
}
