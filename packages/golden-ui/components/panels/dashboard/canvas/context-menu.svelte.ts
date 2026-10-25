import type { GraphState } from '../../../../store/graph.svelte';
import type { WorkbenchSession, SelectionMode } from '../../../../store/workbench.svelte';
import type { DashboardLayoutKind, DashboardSnapGrid } from '../dashboard-model';
import type { getDashboardWidgetOrderingState } from '../dashboard-actions';
import { type DashboardContextMenuScope, type DashboardContextMenuState } from './contracts';
import {
	getRootRemPixels,
	getViewportWidthPx,
	getViewportHeightPx,
	getElementSurfaceMetrics,
	getDirectDashboardLayoutElement,
	isInteractiveSurfaceTarget
} from './dom';
import { sendRemoveNodeIntent } from '../../../../store/ui-intents';
import { type UiNodeDto } from '../../../../types';
import { type ContextMenuAnchor, type ContextMenuItem } from '../../../common/context-menu';
import {
	createDashboardContainerWidget,
	duplicateDashboardWidget,
	getDashboardContainerCreationDefaults,
	moveDashboardWidgetByDelta,
	moveDashboardWidgetToBack,
	moveDashboardWidgetToFront,
	type DashboardWidgetCreationPlacement,
	wrapDashboardWidgetInContainer
} from '../dashboard-actions';
interface DashboardContextMenuContext {
	readonly graph: GraphState | null;
	readonly session: WorkbenchSession | null;
	readonly node: UiNodeDto;
	readonly parent: UiNodeDto | null;
	readonly editMode: boolean;
	readonly layoutKind: DashboardLayoutKind;
	readonly snapGrid: DashboardSnapGrid;
	readonly ordering: ReturnType<typeof getDashboardWidgetOrderingState>;
	selectWidgetNode(mode?: SelectionMode): void;
}
export function createDashboardContextMenu(context: DashboardContextMenuContext) {
	let graph = $derived(context.graph);
	let session = $derived(context.session);
	let liveNode = $derived(context.node);
	let parentNode = $derived(context.parent);
	let editMode = $derived(context.editMode);
	let layoutKind = $derived(context.layoutKind);
	let surfaceSnapGrid = $derived(context.snapGrid);
	let widgetOrderingState = $derived(context.ordering);
	let isWidget = $derived(
		['dashboard_widget_container', 'dashboard_node_widget', 'dashboard_generic_widget'].includes(
			liveNode.node_type
		)
	);
	let isLayoutSurface = $derived(
		['dashboard_page', 'dashboard_widget_container'].includes(liveNode.node_type)
	);
	const getGraph = () => context.graph;
	const selectWidgetNode = (mode?: SelectionMode) => context.selectWidgetNode(mode);
	let canInsertContainer = $derived.by(() => {
		if (!editMode || !isLayoutSurface) {
			return false;
		}
		return liveNode.creatable_user_items.some(
			(candidate) => candidate.node_type === 'dashboard_widget_container'
		);
	});

	let canWrapWidget = $derived.by(() => {
		if (
			!editMode ||
			!isWidget ||
			!parentNode ||
			!liveNode.meta.user_permissions.can_remove_and_duplicate
		) {
			return false;
		}
		return parentNode.creatable_user_items.some(
			(candidate) => candidate.node_type === 'dashboard_widget_container'
		);
	});

	let canDuplicateWidget = $derived.by(() => {
		if (
			!editMode ||
			!isWidget ||
			!parentNode ||
			!liveNode.meta.user_permissions.can_remove_and_duplicate
		) {
			return false;
		}
		return parentNode.creatable_user_items.some(
			(candidate) => candidate.node_type === liveNode.node_type
		);
	});

	let canDeleteWidget = $derived(
		editMode && isWidget && liveNode.meta.user_permissions.can_remove_and_duplicate
	);

	let canBringBackward = $derived(
		editMode && isWidget && (widgetOrderingState?.currentIndex ?? 0) > 0
	);

	let canBringToBack = $derived(canBringBackward);

	let canBringForward = $derived.by(() => {
		if (!editMode || !isWidget || !widgetOrderingState) {
			return false;
		}
		return widgetOrderingState.currentIndex < widgetOrderingState.itemCount - 1;
	});

	let canBringToFront = $derived(canBringForward);

	let dashboardContextMenu = $state<DashboardContextMenuState>({
		open: false,
		scope: 'surface',
		x: 0,
		y: 0,
		surfacePlacement: null
	});

	const closeDashboardContextMenu = (): void => {
		dashboardContextMenu = {
			open: false,
			scope: 'surface',
			x: 0,
			y: 0,
			surfacePlacement: null
		};
	};

	const openDashboardContextMenu = (
		scope: DashboardContextMenuScope,
		x: number,
		y: number,
		surfacePlacement: DashboardWidgetCreationPlacement | null = null
	): void => {
		dashboardContextMenu = {
			open: true,
			scope,
			x,
			y,
			surfacePlacement
		};
	};

	const resolveSurfaceContextPlacement = (
		target: EventTarget | null,
		clientX: number,
		clientY: number
	): DashboardWidgetCreationPlacement | null => {
		if (!(target instanceof HTMLElement)) {
			return null;
		}
		const layoutElement = getDirectDashboardLayoutElement(target) ?? target;
		const rootRemPx = getRootRemPixels();
		const metrics = getElementSurfaceMetrics(layoutElement, rootRemPx);
		const context = {
			rootRemPx,
			emPx: metrics.emPx,
			surfaceWidthPx: metrics.width,
			surfaceHeightPx: metrics.height,
			viewportWidthPx: getViewportWidthPx(),
			viewportHeightPx: getViewportHeightPx()
		};

		if (layoutKind !== 'free') {
			return getDashboardContainerCreationDefaults(context);
		}

		const rect = layoutElement.getBoundingClientRect();
		const centerPx = {
			x: (clientX - rect.left) / Math.max(metrics.scaleX, 1e-6),
			y: (clientY - rect.top) / Math.max(metrics.scaleY, 1e-6)
		};
		const snapPx = surfaceSnapGrid.enabled ? surfaceSnapGrid.step * rootRemPx : 0;
		return getDashboardContainerCreationDefaults(context, { centerPx, snapPx });
	};

	const handleSurfaceContextMenu = (event: MouseEvent): void => {
		if (!editMode || !isLayoutSurface) {
			return;
		}
		const currentTarget = event.currentTarget;
		if (!(currentTarget instanceof Element)) {
			return;
		}
		const ownerWidgetShell = currentTarget.closest('.dashboard-widget-shell');
		const childWidgetShell =
			event.target instanceof Element ? event.target.closest('.dashboard-widget-shell') : null;
		if (
			(childWidgetShell !== null && childWidgetShell !== ownerWidgetShell) ||
			isInteractiveSurfaceTarget(event.target)
		) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		openDashboardContextMenu(
			'surface',
			event.clientX,
			event.clientY,
			resolveSurfaceContextPlacement(currentTarget, event.clientX, event.clientY)
		);
	};

	const handleWidgetContextMenu = (event: MouseEvent): void => {
		if (!editMode || !isWidget) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		selectWidgetNode('REPLACE');
		openDashboardContextMenu('widget', event.clientX, event.clientY);
	};

	const insertContainerFromContextMenu = async (): Promise<void> => {
		if (!canInsertContainer) {
			return;
		}
		const createdNodeId = await createDashboardContainerWidget(getGraph, liveNode.node_id, {
			placement: dashboardContextMenu.surfacePlacement ?? undefined
		});
		if (createdNodeId !== null) {
			session?.selectNode(createdNodeId);
		}
	};

	const duplicateWidgetFromContextMenu = async (): Promise<void> => {
		if (!canDuplicateWidget || !parentNode) {
			return;
		}
		await duplicateDashboardWidget(getGraph, liveNode.node_id);
	};

	const deleteWidgetFromContextMenu = async (): Promise<void> => {
		if (!canDeleteWidget) {
			return;
		}
		await sendRemoveNodeIntent(liveNode.node_id);
	};

	const wrapWidgetFromContextMenu = async (): Promise<void> => {
		if (!canWrapWidget) {
			return;
		}
		await wrapDashboardWidgetInContainer(getGraph, liveNode.node_id);
	};

	const dashboardContextMenuItems = $derived.by((): ContextMenuItem[] => {
		if (!editMode || !dashboardContextMenu.open) {
			return [];
		}

		if (dashboardContextMenu.scope === 'surface') {
			return [
				{
					id: 'insert-container',
					label: 'Insert Container',
					disabled: !canInsertContainer,
					action: () => {
						void insertContainerFromContextMenu();
					}
				}
			];
		}

		return [
			{
				id: 'wrap-in-container',
				label: 'Wrap In Container',
				disabled: !canWrapWidget,
				action: () => {
					void wrapWidgetFromContextMenu();
				}
			},
			{
				id: 'duplicate-widget',
				label: 'Duplicate',
				disabled: !canDuplicateWidget,
				action: () => {
					void duplicateWidgetFromContextMenu();
				}
			},
			{
				id: 'delete-widget',
				label: 'Delete',
				disabled: !canDeleteWidget,
				color: 'color-mix(in srgb, #ff8b8b 82%, white 18%)',
				hoverColor: 'color-mix(in srgb, #ff5c5c 23%, transparent)',
				action: () => {
					void deleteWidgetFromContextMenu();
				}
			},
			{ separator: true },
			{
				id: 'ordering',
				label: 'Ordering',
				submenu: [
					{
						id: 'bring-forward',
						label: 'Bring Forward',
						disabled: !canBringForward,
						action: () => {
							void moveDashboardWidgetByDelta(getGraph, liveNode.node_id, 1);
						}
					},
					{
						id: 'send-backward',
						label: 'Backward',
						disabled: !canBringBackward,
						action: () => {
							void moveDashboardWidgetByDelta(getGraph, liveNode.node_id, -1);
						}
					},
					{
						id: 'bring-to-front',
						label: 'Bring To Front',
						disabled: !canBringToFront,
						action: () => {
							void moveDashboardWidgetToFront(getGraph, liveNode.node_id);
						}
					},
					{
						id: 'send-to-back',
						label: 'Bring To Back',
						disabled: !canBringToBack,
						action: () => {
							void moveDashboardWidgetToBack(getGraph, liveNode.node_id);
						}
					}
				]
			}
		];
	});

	const dashboardContextMenuAnchor = $derived.by((): ContextMenuAnchor | null => {
		if (!dashboardContextMenu.open) {
			return null;
		}
		return {
			kind: 'point',
			x: dashboardContextMenu.x,
			y: dashboardContextMenu.y
		};
	});

	$effect(() => {
		if (editMode || !dashboardContextMenu.open) {
			return;
		}
		closeDashboardContextMenu();
	});

	$effect(() => {
		if (!dashboardContextMenu.open || dashboardContextMenuItems.length > 0) {
			return;
		}
		closeDashboardContextMenu();
	});

	return {
		get dashboardContextMenu() {
			return dashboardContextMenu;
		},
		get dashboardContextMenuItems() {
			return dashboardContextMenuItems;
		},
		get dashboardContextMenuAnchor() {
			return dashboardContextMenuAnchor;
		},
		closeDashboardContextMenu,
		handleSurfaceContextMenu,
		handleWidgetContextMenu
	};
}
