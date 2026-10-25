import type { GraphState } from '../../../../store/graph.svelte';
import type { WorkbenchSession } from '../../../../store/workbench.svelte';
import type { DashboardPlacement } from '../dashboard-model';
import {
	type FreeLayoutPreview,
	type FreeLayoutTarget,
	type FreeLayoutPreviewSet,
	type FreeLayoutDropTarget,
	type FreeLayoutInteraction,
	type SurfaceInsertionTarget,
	type WidgetMoveDropTarget,
	type WidgetMoveInteraction,
	type SurfaceOrderAnchor
} from './contracts';
import {
	createCssUnitContext,
	isFreeLayoutTransformableWidget,
	resolveFreeLayoutPreview,
	snapToGridPx,
	anchorXFromAnchor,
	anchorYFromAnchor,
	anchorBasePx,
	anchorOffsetPx,
	freeLayoutRectFromPlacement
} from './geometry';
import {
	getRootRemPixels,
	getViewportWidthPx,
	getViewportHeightPx,
	getElementSurfaceMetrics,
	getDirectDashboardLayoutElement,
	resolveDashboardSurfaceAtPoint,
	resolveSurfaceContentElement,
	buildVerticalInsertionIndicator,
	buildHorizontalInsertionIndicator
} from './dom';
import { type NodeId, type UiNodeDto } from '../../../../types';
import { cssValueToPx } from '../../../../css-value';
import { type DashboardWidgetCreationPlacement } from '../dashboard-actions';
import {
	getDashboardGridSettings,
	getDashboardLayoutKind,
	getDashboardPlacement,
	getDashboardSnapGrid,
	getDirectItemChildren,
	getDirectParamNode,
	type DashboardLayoutKind
} from '../dashboard-model';
interface DashboardInteractionGeometryContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly parent: UiNodeDto | null;
	readonly placement: DashboardPlacement;
	readonly session: WorkbenchSession | null;
	readonly editMode: boolean;
	readonly isSelected: boolean;
	readonly freeInteraction: FreeLayoutInteraction | null;
	readonly orderedInteraction: WidgetMoveInteraction | null;
}
export function createDashboardInteractionGeometry(context: DashboardInteractionGeometryContext) {
	let graph = $derived(context.graph);
	let liveNode = $derived(context.node);
	let parentNode = $derived(context.parent);
	let placement = $derived(context.placement);
	let session = $derived(context.session);
	let editMode = $derived(context.editMode);
	let isSelected = $derived(context.isSelected);
	let freeLayoutInteraction = $derived(context.freeInteraction);
	let widgetMoveInteraction = $derived(context.orderedInteraction);
	let isWidget = $derived(
		['dashboard_widget_container', 'dashboard_node_widget', 'dashboard_generic_widget'].includes(
			liveNode.node_type
		)
	);
	const resolveSurfaceOrderAnchors = (
		surfaceNode: UiNodeDto,
		surfaceElement: HTMLElement,
		targetLayoutKind: DashboardLayoutKind,
		ignoredNodeIds: ReadonlySet<NodeId>
	): SurfaceOrderAnchor[] => {
		if (!graph) {
			return [];
		}
		const orderedChildren = getDirectItemChildren(graph, surfaceNode).filter(
			(child) => !ignoredNodeIds.has(child.node_id)
		);
		if (orderedChildren.length === 0) {
			return [];
		}

		if (targetLayoutKind === 'tabs') {
			const buttons = Array.from(
				surfaceElement.querySelectorAll<HTMLElement>(
					'.dashboard-tab-strip > button:not(.dashboard-drop-preview-tab)'
				)
			);
			return orderedChildren
				.map((child, index) => {
					const button = buttons[index];
					return button
						? {
								nodeId: child.node_id,
								rect: button.getBoundingClientRect()
							}
						: null;
				})
				.filter((anchor): anchor is SurfaceOrderAnchor => anchor !== null);
		}

		if (targetLayoutKind === 'accordion') {
			const sections = Array.from(
				surfaceElement.querySelectorAll<HTMLElement>(
					'.dashboard-accordion > .dashboard-accordion-item'
				)
			);
			return orderedChildren
				.map((child, index) => {
					const section = sections[index];
					if (!section) {
						return null;
					}
					const button = section.querySelector<HTMLElement>('button');
					const anchorElement = button ?? section;
					return {
						nodeId: child.node_id,
						rect: anchorElement.getBoundingClientRect()
					};
				})
				.filter((anchor): anchor is SurfaceOrderAnchor => anchor !== null);
		}

		return orderedChildren
			.map((child) => {
				const shell = surfaceElement.querySelector<HTMLElement>(
					`.dashboard-widget-shell[data-node-id="${String(child.node_id)}"][data-dashboard-surface-id="${String(surfaceNode.node_id)}"]`
				);
				return shell
					? {
							nodeId: child.node_id,
							rect: shell.getBoundingClientRect()
						}
					: null;
			})
			.filter((anchor): anchor is SurfaceOrderAnchor => anchor !== null);
	};

	const resolveSurfaceInsertionTarget = (
		surfaceNode: UiNodeDto,
		surfaceElement: HTMLElement,
		targetLayoutKind: DashboardLayoutKind,
		clientX: number,
		clientY: number,
		ignoredNodeIds: ReadonlySet<NodeId>
	): SurfaceInsertionTarget | null => {
		if (!graph) {
			return null;
		}
		if (targetLayoutKind === 'free') {
			return {
				targetIndex: getDirectItemChildren(graph, surfaceNode).length,
				indicator: null
			};
		}

		const rootRemPx = getRootRemPixels();
		const surfaceRect = surfaceElement.getBoundingClientRect();
		const surfaceMetrics = getElementSurfaceMetrics(surfaceElement, rootRemPx);
		const trackElement = resolveSurfaceContentElement(surfaceElement, targetLayoutKind);
		const trackRect = trackElement.getBoundingClientRect();
		const anchors = resolveSurfaceOrderAnchors(
			surfaceNode,
			surfaceElement,
			targetLayoutKind,
			ignoredNodeIds
		);

		if (anchors.length === 0) {
			if (targetLayoutKind === 'horizontal' || targetLayoutKind === 'tabs') {
				return {
					targetIndex: 0,
					indicator: buildVerticalInsertionIndicator(
						surfaceRect,
						trackRect,
						surfaceMetrics,
						trackRect.left - surfaceRect.left + 0.5 * rootRemPx
					)
				};
			}
			return {
				targetIndex: 0,
				indicator: buildHorizontalInsertionIndicator(
					surfaceRect,
					trackRect,
					surfaceMetrics,
					trackRect.top - surfaceRect.top + 0.5 * rootRemPx
				)
			};
		}

		if (targetLayoutKind === 'horizontal' || targetLayoutKind === 'tabs') {
			for (let index = 0; index < anchors.length; index += 1) {
				const anchor = anchors[index];
				if (clientX < anchor.rect.left + anchor.rect.width * 0.5) {
					return {
						targetIndex: index,
						indicator: buildVerticalInsertionIndicator(
							surfaceRect,
							trackRect,
							surfaceMetrics,
							anchor.rect.left - surfaceRect.left,
							anchor.rect
						)
					};
				}
			}
			const last = anchors[anchors.length - 1];
			return {
				targetIndex: anchors.length,
				indicator: buildVerticalInsertionIndicator(
					surfaceRect,
					trackRect,
					surfaceMetrics,
					last.rect.right - surfaceRect.left,
					last.rect
				)
			};
		}

		if (targetLayoutKind === 'vertical' || targetLayoutKind === 'accordion') {
			for (let index = 0; index < anchors.length; index += 1) {
				const anchor = anchors[index];
				if (clientY < anchor.rect.top + anchor.rect.height * 0.5) {
					return {
						targetIndex: index,
						indicator: buildHorizontalInsertionIndicator(
							surfaceRect,
							trackRect,
							surfaceMetrics,
							anchor.rect.top - surfaceRect.top,
							anchor.rect
						)
					};
				}
			}
			const last = anchors[anchors.length - 1];
			return {
				targetIndex: anchors.length,
				indicator: buildHorizontalInsertionIndicator(
					surfaceRect,
					trackRect,
					surfaceMetrics,
					last.rect.bottom - surfaceRect.top,
					last.rect
				)
			};
		}

		const gridDirection = getDashboardGridSettings(graph, surfaceNode).direction;
		if (gridDirection === 'column') {
			for (let index = 0; index < anchors.length; index += 1) {
				const anchor = anchors[index];
				if (
					clientX < anchor.rect.left ||
					(clientX <= anchor.rect.right && clientY < anchor.rect.top + anchor.rect.height * 0.5)
				) {
					return {
						targetIndex: index,
						indicator: buildHorizontalInsertionIndicator(
							surfaceRect,
							trackRect,
							surfaceMetrics,
							anchor.rect.top - surfaceRect.top,
							anchor.rect
						)
					};
				}
			}
			const last = anchors[anchors.length - 1];
			return {
				targetIndex: anchors.length,
				indicator: buildHorizontalInsertionIndicator(
					surfaceRect,
					trackRect,
					surfaceMetrics,
					last.rect.bottom - surfaceRect.top,
					last.rect
				)
			};
		}

		for (let index = 0; index < anchors.length; index += 1) {
			const anchor = anchors[index];
			if (
				clientY < anchor.rect.top ||
				(clientY <= anchor.rect.bottom && clientX < anchor.rect.left + anchor.rect.width * 0.5)
			) {
				return {
					targetIndex: index,
					indicator: buildVerticalInsertionIndicator(
						surfaceRect,
						trackRect,
						surfaceMetrics,
						anchor.rect.left - surfaceRect.left,
						anchor.rect
					)
				};
			}
		}
		const last = anchors[anchors.length - 1];
		return {
			targetIndex: anchors.length,
			indicator: buildVerticalInsertionIndicator(
				surfaceRect,
				trackRect,
				surfaceMetrics,
				last.rect.right - surfaceRect.left,
				last.rect
			)
		};
	};

	const currentPlacementPreview = (): FreeLayoutPreview => ({
		position: {
			x: placement.position.x,
			y: placement.position.y
		},
		size: {
			width: { ...placement.size.width },
			height: { ...placement.size.height }
		}
	});

	const currentDraggedPlacement = (
		previewSet: FreeLayoutPreviewSet | null
	): DashboardWidgetCreationPlacement => {
		const currentPreview = resolveFreeLayoutPreview(previewSet, liveNode.node_id);
		return {
			anchor: placement.anchor,
			position: currentPreview ? currentPreview.position : placement.position,
			size: currentPreview ? currentPreview.size : placement.size,
			sizeEnabled: {
				width: placement.sizeEnabled.width,
				height: placement.sizeEnabled.height
			}
		};
	};

	const buildFreeLayoutDropPlacement = (
		targetSurfaceNode: UiNodeDto,
		targetSurfaceElement: HTMLElement,
		basePlacement: DashboardWidgetCreationPlacement,
		clientX: number,
		clientY: number
	): DashboardWidgetCreationPlacement => {
		const targetLayoutKind = getDashboardLayoutKind(graph, targetSurfaceNode, 'free');
		if (targetLayoutKind !== 'free') {
			return {
				anchor: basePlacement.anchor,
				position: { ...basePlacement.position },
				size: {
					width: { ...basePlacement.size.width },
					height: { ...basePlacement.size.height }
				},
				sizeEnabled: basePlacement.sizeEnabled
					? {
							width: basePlacement.sizeEnabled.width,
							height: basePlacement.sizeEnabled.height
						}
					: undefined
			};
		}

		const layoutElement =
			getDirectDashboardLayoutElement(targetSurfaceElement) ?? targetSurfaceElement;
		const rootRemPx = getRootRemPixels();
		const metrics = getElementSurfaceMetrics(layoutElement, rootRemPx);
		const rect = layoutElement.getBoundingClientRect();
		const surfaceContext = {
			rootRemPx,
			emPx: metrics.emPx,
			surfaceWidthPx: metrics.width,
			surfaceHeightPx: metrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};
		const widthPx = Math.max(
			0,
			cssValueToPx(basePlacement.size.width, 'x', createCssUnitContext(surfaceContext, 'x'))
		);
		const heightPx = Math.max(
			0,
			cssValueToPx(basePlacement.size.height, 'y', createCssUnitContext(surfaceContext, 'y'))
		);
		const localCenterX = (clientX - rect.left) / Math.max(metrics.scaleX, 1e-6);
		const localCenterY = (clientY - rect.top) / Math.max(metrics.scaleY, 1e-6);
		const snapGrid = getDashboardSnapGrid(graph, targetSurfaceNode);
		const snapPx = snapGrid.enabled ? snapGrid.step * rootRemPx : 0;
		const snappedLeft = snapToGridPx(localCenterX - widthPx * 0.5, snapPx);
		const snappedTop = snapToGridPx(localCenterY - heightPx * 0.5, snapPx);
		const anchorX = anchorXFromAnchor(basePlacement.anchor);
		const anchorY = anchorYFromAnchor(basePlacement.anchor);

		return {
			anchor: basePlacement.anchor,
			position: {
				x: snappedLeft + anchorOffsetPx(anchorX, widthPx) - anchorBasePx(anchorX, metrics.width),
				y: snappedTop + anchorOffsetPx(anchorY, heightPx) - anchorBasePx(anchorY, metrics.height)
			},
			size: {
				width: { ...basePlacement.size.width },
				height: { ...basePlacement.size.height }
			},
			sizeEnabled: basePlacement.sizeEnabled
				? {
						width: basePlacement.sizeEnabled.width,
						height: basePlacement.sizeEnabled.height
					}
				: undefined
		};
	};

	const resolveFreeLayoutDropTargetAtPoint = (
		clientX: number,
		clientY: number,
		previewSet: FreeLayoutPreviewSet | null
	): FreeLayoutDropTarget | null => {
		if (
			!graph ||
			!editMode ||
			!isWidget ||
			!freeLayoutInteraction ||
			freeLayoutInteraction.mode !== 'move' ||
			freeLayoutInteraction.targets.length !== 1
		) {
			return null;
		}

		const resolvedSurface = resolveDashboardSurfaceAtPoint(
			clientX,
			clientY,
			new Set(freeLayoutInteraction.targets.map((target) => target.nodeId))
		);
		if (
			!resolvedSurface ||
			resolvedSurface.surfaceNodeId === freeLayoutInteraction.sourceSurfaceNodeId
		) {
			return null;
		}

		const targetSurfaceNode = graph.nodesById.get(resolvedSurface.surfaceNodeId);
		if (!targetSurfaceNode) {
			return null;
		}
		if (
			!targetSurfaceNode.creatable_user_items.some(
				(candidate) => candidate.node_type === liveNode.node_type
			)
		) {
			return null;
		}

		const targetLayoutKind = getDashboardLayoutKind(graph, targetSurfaceNode, 'free');
		const insertionTarget =
			targetLayoutKind === 'free'
				? null
				: resolveSurfaceInsertionTarget(
						targetSurfaceNode,
						resolvedSurface.element,
						targetLayoutKind,
						clientX,
						clientY,
						new Set<NodeId>([liveNode.node_id])
					);

		return {
			surfaceNodeId: resolvedSurface.surfaceNodeId,
			element: resolvedSurface.element,
			placement: buildFreeLayoutDropPlacement(
				targetSurfaceNode,
				resolvedSurface.element,
				currentDraggedPlacement(previewSet),
				clientX,
				clientY
			),
			targetIndex: insertionTarget?.targetIndex ?? null,
			indicator: insertionTarget?.indicator ?? null
		};
	};

	const resolveWidgetMoveDropTargetAtPoint = (
		clientX: number,
		clientY: number
	): WidgetMoveDropTarget | null => {
		if (!graph || !editMode || !isWidget || !widgetMoveInteraction) {
			return null;
		}

		const resolvedSurface = resolveDashboardSurfaceAtPoint(
			clientX,
			clientY,
			new Set<NodeId>([liveNode.node_id])
		);
		if (!resolvedSurface) {
			return null;
		}

		const targetSurfaceNode = graph.nodesById.get(resolvedSurface.surfaceNodeId);
		if (!targetSurfaceNode) {
			return null;
		}
		if (
			!targetSurfaceNode.creatable_user_items.some(
				(candidate) => candidate.node_type === liveNode.node_type
			)
		) {
			return null;
		}

		const targetLayoutKind = getDashboardLayoutKind(graph, targetSurfaceNode, 'free');
		const insertionTarget = resolveSurfaceInsertionTarget(
			targetSurfaceNode,
			resolvedSurface.element,
			targetLayoutKind,
			clientX,
			clientY,
			new Set<NodeId>([liveNode.node_id])
		);

		return {
			surfaceNodeId: resolvedSurface.surfaceNodeId,
			element: resolvedSurface.element,
			layoutKind: targetLayoutKind,
			targetIndex: targetLayoutKind === 'free' ? null : (insertionTarget?.targetIndex ?? 0),
			placement: buildFreeLayoutDropPlacement(
				targetSurfaceNode,
				resolvedSurface.element,
				currentDraggedPlacement(null),
				clientX,
				clientY
			),
			indicator: targetLayoutKind === 'free' ? null : (insertionTarget?.indicator ?? null)
		};
	};

	const resolveSelectedFreeLayoutWidgets = (): UiNodeDto[] => {
		if (!graph || !parentNode || !session) {
			return [];
		}
		const selectedNodeIds = new Set(session.selectedNodesIds);
		return getDirectItemChildren(graph, parentNode).filter(
			(candidate) =>
				selectedNodeIds.has(candidate.node_id) && isFreeLayoutTransformableWidget(candidate)
		);
	};

	const resolveTransformNodes = (): UiNodeDto[] => {
		const selectedWidgets = resolveSelectedFreeLayoutWidgets();
		if (isSelected && selectedWidgets.length > 1) {
			return selectedWidgets;
		}
		return [liveNode];
	};

	const buildFreeLayoutTargets = (
		nodes: UiNodeDto[],
		interaction: Pick<
			FreeLayoutInteraction,
			| 'rootRemPx'
			| 'emPx'
			| 'surfaceWidthPx'
			| 'surfaceHeightPx'
			| 'viewportWidthPx'
			| 'viewportHeightPx'
		>
	): FreeLayoutTarget[] => {
		if (!graph) {
			return [];
		}
		const targets: FreeLayoutTarget[] = [];
		for (const targetNode of nodes) {
			const targetPlacement = getDashboardPlacement(graph, targetNode);
			const positionParamNode = getDirectParamNode(graph, targetNode, 'position');
			const widthParamNode = getDirectParamNode(graph, targetNode, 'width');
			const heightParamNode = getDirectParamNode(graph, targetNode, 'height');
			targets.push({
				nodeId: targetNode.node_id,
				anchor: targetPlacement.anchor,
				positionParamNodeId:
					positionParamNode?.data.kind === 'parameter' ? positionParamNode.node_id : null,
				positionParamBehaviour:
					positionParamNode?.data.kind === 'parameter'
						? positionParamNode.data.param.event_behaviour
						: null,
				widthParamNodeId: widthParamNode?.data.kind === 'parameter' ? widthParamNode.node_id : null,
				widthParamBehaviour:
					widthParamNode?.data.kind === 'parameter'
						? widthParamNode.data.param.event_behaviour
						: null,
				heightParamNodeId:
					heightParamNode?.data.kind === 'parameter' ? heightParamNode.node_id : null,
				heightParamBehaviour:
					heightParamNode?.data.kind === 'parameter'
						? heightParamNode.data.param.event_behaviour
						: null,
				startPreview: {
					position: {
						x: targetPlacement.position.x,
						y: targetPlacement.position.y
					},
					size: {
						width: { ...targetPlacement.size.width },
						height: { ...targetPlacement.size.height }
					}
				},
				startRect: freeLayoutRectFromPlacement(targetPlacement, interaction)
			});
		}
		return targets;
	};

	return {
		resolveSurfaceInsertionTarget,
		resolveFreeLayoutDropTargetAtPoint,
		resolveWidgetMoveDropTargetAtPoint,
		resolveTransformNodes,
		buildFreeLayoutTargets
	};
}
