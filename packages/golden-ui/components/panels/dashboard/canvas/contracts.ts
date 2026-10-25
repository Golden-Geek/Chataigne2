import type { SelectionMode } from '../../../../store/workbench.svelte';
import type { NodeId, ParamEventBehaviour, UiNodeDto } from '../../../../types';
import type { CssValueData } from '../../../../css-value';
import type { DashboardWidgetCreationPlacement } from '../dashboard-actions';
import type { DashboardDragPayload } from '../dashboard-drag';
import type { DashboardAnchor, DashboardLayoutKind } from '../dashboard-model';

export const widgetMovePreviewEventName = 'gc-dashboard-widget-move-preview';
export const widgetMoveCommitEventName = 'gc-dashboard-widget-move-commit';

export type DashboardGenericPreviewKind = 'button' | 'slider' | 'checkbox' | 'textInput' | 'text';

export type DashboardPersistedPageView = {
	panX: number;
	panY: number;
	zoom: number;
};

export type DashboardPersistedPageViewChangeHandler = (
	pageNodeId: NodeId,
	nextPageView: DashboardPersistedPageView
) => void;

export type SurfaceDropPreview = {
	label: string;
	targetKind: DashboardDragPayload['kind'];
	placement: DashboardWidgetCreationPlacement;
	targetIndex: number | null;
	genericWidgetKind: DashboardGenericPreviewKind | null;
	previewText: string | null;
	previewPlaceholder: string | null;
	multiline: boolean;
};

export type PendingSurfaceDrop = {
	preview: SurfaceDropPreview;
	knownChildIds: NodeId[];
	knownDirectChildIds: NodeId[];
};

export type PendingWidgetMoveCommit = {
	preview: SurfaceDropPreview | null;
	hiddenChildNodeId: NodeId | null;
	expectedChildIds: NodeId[];
};

export type NonFreeSurfaceFlowRenderItem =
	| {
			key: string;
			kind: 'child';
			child: UiNodeDto;
	  }
	| {
			key: string;
			kind: 'preview';
			preview: SurfaceDropPreview;
	  };

export type WidgetMovePreviewEventDetail = {
	surfaceNodeId: NodeId | null;
	preview: SurfaceDropPreview | null;
	sourceSurfaceNodeId: NodeId | null;
	sourceWidgetNodeId: NodeId | null;
};

export type WidgetMoveCommitEventDetail = {
	targetSurfaceNodeId: NodeId | null;
	preview: SurfaceDropPreview | null;
	sourceSurfaceNodeId: NodeId | null;
	sourceWidgetNodeId: NodeId | null;
};

export type DashboardContextMenuScope = 'surface' | 'widget';

export type DashboardContextMenuState = {
	open: boolean;
	scope: DashboardContextMenuScope;
	x: number;
	y: number;
	surfacePlacement: DashboardWidgetCreationPlacement | null;
};

export type AnchorGuideAxis = 'horizontal' | 'vertical';

export type AnchorGuideDirection = 'left' | 'right' | 'up' | 'down';

export type AnchorGuide = {
	axis: AnchorGuideAxis;
	direction: AnchorGuideDirection;
	style: string;
};

export type FreeLayoutInteractionMode = 'move' | 'resize';

export type FreeLayoutResizeEdges = {
	left: boolean;
	right: boolean;
	top: boolean;
	bottom: boolean;
};

export type FreeLayoutPreview = {
	position: { x: number; y: number };
	size: { width: CssValueData; height: CssValueData };
};

export type FreeLayoutTarget = {
	nodeId: NodeId;
	anchor: DashboardAnchor;
	positionParamNodeId: NodeId | null;
	positionParamBehaviour: ParamEventBehaviour | null;
	widthParamNodeId: NodeId | null;
	widthParamBehaviour: ParamEventBehaviour | null;
	heightParamNodeId: NodeId | null;
	heightParamBehaviour: ParamEventBehaviour | null;
	startPreview: FreeLayoutPreview;
	startRect: FreeLayoutRect;
};

export type FreeLayoutTargetPreview = {
	target: FreeLayoutTarget;
	preview: FreeLayoutPreview;
};

export type FreeLayoutPreviewSet = {
	primaryNodeId: NodeId;
	previews: FreeLayoutTargetPreview[];
};

export type FreeLayoutDropTarget = {
	surfaceNodeId: NodeId;
	element: HTMLElement;
	placement: DashboardWidgetCreationPlacement;
	targetIndex: number | null;
	indicator: SurfaceInsertionIndicator | null;
};

export type FreeLayoutInteraction = {
	pointerId: number;
	mode: FreeLayoutInteractionMode;
	resizeEdges: FreeLayoutResizeEdges | null;
	primaryNodeId: NodeId;
	sourceSurfaceNodeId: NodeId;
	selectionRect: FreeLayoutRect;
	targets: FreeLayoutTarget[];
	startClient: [number, number];
	rootRemPx: number;
	surfaceWidthPx: number;
	surfaceHeightPx: number;
	surfaceScaleX: number;
	surfaceScaleY: number;
	viewportWidthPx: number;
	viewportHeightPx: number;
};

export type SurfaceInsertionIndicator = {
	left: number;
	top: number;
	width: number;
	height: number;
};

export type SurfaceInsertionTarget = {
	targetIndex: number;
	indicator: SurfaceInsertionIndicator | null;
};

export type WidgetMoveDropTarget = {
	surfaceNodeId: NodeId;
	element: HTMLElement;
	layoutKind: DashboardLayoutKind;
	targetIndex: number | null;
	placement: DashboardWidgetCreationPlacement;
	indicator: SurfaceInsertionIndicator | null;
};

export type WidgetMoveInteraction = {
	pointerId: number;
	sourceSurfaceNodeId: NodeId;
	startClient: [number, number];
};

export type ConstrainedResizeInteraction = {
	pointerId: number;
	axis: 'x' | 'y';
	sign: -1 | 1;
	startClient: [number, number];
	paramNodeId: NodeId;
	paramBehaviour: ParamEventBehaviour;
	startSize: CssValueData;
	startSizePx: number;
	rootRemPx: number;
	surfaceWidthPx: number;
	surfaceHeightPx: number;
	surfaceScaleX: number;
	surfaceScaleY: number;
	viewportWidthPx: number;
	viewportHeightPx: number;
	lastSent: CssValueData | null;
};

export type SurfaceMetrics = {
	width: number;
	height: number;
	scaleX: number;
	scaleY: number;
};

export type FreeLayoutRect = {
	left: number;
	top: number;
	width: number;
	height: number;
};

export type DashboardPageViewState = {
	pointerId: number | null;
	lastClient: [number, number] | null;
	panX: number;
	panY: number;
	zoom: number;
};

export type FreeLayoutResizeZone = {
	name: string;
	edges: FreeLayoutResizeEdges;
};

export type MarqueeSelectionState = {
	pointerId: number;
	surfaceNodeId: NodeId;
	surfaceElement: HTMLElement;
	startClient: [number, number];
	currentClient: [number, number];
	baseSelection: NodeId[];
	selectionMode: SelectionMode;
};

export type SurfacePressState = {
	pointerId: number;
	surfaceNodeId: NodeId;
	surfaceElement: HTMLElement;
	startClient: [number, number];
	baseSelection: NodeId[];
	selectionMode: SelectionMode;
};

export type SurfaceOrderAnchor = {
	nodeId: NodeId;
	rect: DOMRect;
};
