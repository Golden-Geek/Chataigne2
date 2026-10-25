import type { createDashboardWidgetContent } from './widget-content.svelte';
import { widgetMovePreviewEventName, widgetMoveCommitEventName } from './contracts';
import {
	type DashboardGenericPreviewKind,
	type SurfaceDropPreview,
	type WidgetMovePreviewEventDetail,
	type WidgetMoveCommitEventDetail,
	type SurfaceInsertionIndicator,
	type WidgetMoveDropTarget
} from './contracts';
import { type NodeId, type UiNodeDto } from '../../../../types';
interface DashboardInteractionChromeContext {
	readonly node: UiNodeDto;
	readonly parent: UiNodeDto | null;
	readonly content: Pick<
		ReturnType<typeof createDashboardWidgetContent>,
		'widgetKind' | 'genericDisplayValue' | 'textConfig' | 'placeholderConfig' | 'multiline'
	>;
}
export function createDashboardInteractionChrome(context: DashboardInteractionChromeContext) {
	let liveNode = $derived(context.node);
	let parentNode = $derived(context.parent);
	let widgetKind = $derived(context.content.widgetKind);
	let genericDisplayValue = $derived(context.content.genericDisplayValue);
	let textConfig = $derived(context.content.textConfig);
	let placeholderConfig = $derived(context.content.placeholderConfig);
	let multiline = $derived(context.content.multiline);
	let widgetLabelText = $derived(liveNode.meta.label.trim());
	let isGenericWidget = $derived(liveNode.node_type === 'dashboard_generic_widget');
	let isNodeWidget = $derived(liveNode.node_type === 'dashboard_node_widget');
	let isContainerWidget = $derived(liveNode.node_type === 'dashboard_widget_container');
	let freeLayoutHoverSurfaceElement: HTMLElement | null = null;

	let surfaceInsertionIndicatorElement: HTMLElement | null = null;

	let freeLayoutAncestorShells: HTMLElement[] = [];
	let previewOwner: { id: NodeId; uuid: string } | null = null;
	let commitOwner: { id: NodeId; uuid: string } | null = null;

	const clearSurfaceInsertionIndicator = (): void => {
		if (!surfaceInsertionIndicatorElement) {
			return;
		}
		surfaceInsertionIndicatorElement.classList.remove('dashboard-flow-drop-target');
		surfaceInsertionIndicatorElement.style.removeProperty('--dashboard-flow-drop-indicator-left');
		surfaceInsertionIndicatorElement.style.removeProperty('--dashboard-flow-drop-indicator-top');
		surfaceInsertionIndicatorElement.style.removeProperty('--dashboard-flow-drop-indicator-width');
		surfaceInsertionIndicatorElement.style.removeProperty('--dashboard-flow-drop-indicator-height');
		surfaceInsertionIndicatorElement = null;
	};

	const setSurfaceInsertionIndicator = (
		surfaceElement: HTMLElement | null,
		indicator: SurfaceInsertionIndicator | null
	): void => {
		if (surfaceInsertionIndicatorElement && surfaceInsertionIndicatorElement !== surfaceElement) {
			clearSurfaceInsertionIndicator();
		}
		if (!surfaceElement || !indicator) {
			clearSurfaceInsertionIndicator();
			return;
		}
		surfaceInsertionIndicatorElement = surfaceElement;
		surfaceElement.classList.add('dashboard-flow-drop-target');
		surfaceElement.style.setProperty('--dashboard-flow-drop-indicator-left', `${indicator.left}px`);
		surfaceElement.style.setProperty('--dashboard-flow-drop-indicator-top', `${indicator.top}px`);
		surfaceElement.style.setProperty(
			'--dashboard-flow-drop-indicator-width',
			`${indicator.width}px`
		);
		surfaceElement.style.setProperty(
			'--dashboard-flow-drop-indicator-height',
			`${indicator.height}px`
		);
	};

	const setFreeLayoutHoverSurface = (nextSurface: HTMLElement | null): void => {
		if (freeLayoutHoverSurfaceElement === nextSurface) {
			return;
		}
		freeLayoutHoverSurfaceElement?.classList.remove('dashboard-reparent-target');
		freeLayoutHoverSurfaceElement = nextSurface;
		freeLayoutHoverSurfaceElement?.classList.add('dashboard-reparent-target');
	};

	const setFreeLayoutAncestorShells = (target: EventTarget | null): void => {
		for (const shell of freeLayoutAncestorShells) {
			shell.classList.remove('dashboard-descendant-free-layout-active');
		}
		freeLayoutAncestorShells = [];
		if (!(target instanceof Element)) {
			return;
		}

		let cursor = target.parentElement?.closest('.dashboard-widget-shell');
		while (cursor instanceof HTMLElement) {
			cursor.classList.add('dashboard-descendant-free-layout-active');
			freeLayoutAncestorShells.push(cursor);
			cursor = cursor.parentElement?.closest('.dashboard-widget-shell');
		}
	};

	const buildCurrentWidgetMovePreview = (
		dropTarget: Pick<WidgetMoveDropTarget, 'placement' | 'targetIndex'>
	): SurfaceDropPreview => {
		const genericWidgetKind = isGenericWidget ? (widgetKind as DashboardGenericPreviewKind) : null;
		return {
			label: widgetLabelText.length > 0 ? widgetLabelText : liveNode.meta.label,
			targetKind: isNodeWidget || isContainerWidget ? 'node' : 'parameter',
			placement: dropTarget.placement,
			targetIndex: dropTarget.targetIndex,
			genericWidgetKind,
			previewText: isGenericWidget
				? genericDisplayValue || textConfig || widgetLabelText || null
				: null,
			previewPlaceholder: isGenericWidget ? placeholderConfig || widgetLabelText || null : null,
			multiline: isGenericWidget ? multiline : false
		};
	};

	const publishWidgetMoveSurfacePreview = (
		dropTarget: Pick<WidgetMoveDropTarget, 'surfaceNodeId' | 'placement' | 'targetIndex'> | null
	): void => {
		if (typeof window === 'undefined') {
			return;
		}
		if (dropTarget) previewOwner = { id: liveNode.node_id, uuid: liveNode.uuid };
		const owner = previewOwner;
		if (!owner) return;
		const detail: WidgetMovePreviewEventDetail = dropTarget
			? {
					operation: owner,
					sourceWidgetUuid: owner.uuid,
					surfaceNodeId: dropTarget.surfaceNodeId,
					preview: buildCurrentWidgetMovePreview(dropTarget),
					sourceSurfaceNodeId: parentNode?.node_id ?? null,
					sourceWidgetNodeId: liveNode.node_id
				}
			: {
					operation: owner,
					sourceWidgetUuid: owner.uuid,
					surfaceNodeId: null,
					preview: null,
					sourceSurfaceNodeId: null,
					sourceWidgetNodeId: owner.id
				};
		if (!dropTarget) previewOwner = null;
		window.dispatchEvent(
			new CustomEvent<WidgetMovePreviewEventDetail>(widgetMovePreviewEventName, { detail })
		);
	};

	// Async rejection cancels this publication, even if the source starts another move meanwhile.
	const publishWidgetMoveCommit = (
		dropTarget: Pick<WidgetMoveDropTarget, 'surfaceNodeId' | 'placement' | 'targetIndex'> | null,
		sourceSurfaceNodeId: NodeId | null = null
	): (() => void) | null => {
		if (typeof window === 'undefined') {
			return null;
		}
		if (dropTarget) commitOwner = { id: liveNode.node_id, uuid: liveNode.uuid };
		const owner = commitOwner;
		if (!owner) return null;
		const detail: WidgetMoveCommitEventDetail = dropTarget
			? {
					operation: owner,
					sourceWidgetUuid: owner.uuid,
					targetSurfaceNodeId: dropTarget.surfaceNodeId,
					preview: buildCurrentWidgetMovePreview(dropTarget),
					sourceSurfaceNodeId,
					sourceWidgetNodeId: liveNode.node_id
				}
			: {
					operation: owner,
					sourceWidgetUuid: owner.uuid,
					targetSurfaceNodeId: null,
					preview: null,
					sourceSurfaceNodeId: null,
					sourceWidgetNodeId: owner.id
				};
		if (!dropTarget) commitOwner = null;
		window.dispatchEvent(
			new CustomEvent<WidgetMoveCommitEventDetail>(widgetMoveCommitEventName, { detail })
		);
		return () => {
			if (commitOwner === owner) commitOwner = null;
			window.dispatchEvent(
				new CustomEvent<WidgetMoveCommitEventDetail>(widgetMoveCommitEventName, {
					detail: { ...detail, targetSurfaceNodeId: null, preview: null }
				})
			);
		};
	};

	return {
		clearSurfaceInsertionIndicator,
		setSurfaceInsertionIndicator,
		setFreeLayoutHoverSurface,
		setFreeLayoutAncestorShells,
		publishWidgetMoveSurfacePreview,
		publishWidgetMoveCommit
	};
}
