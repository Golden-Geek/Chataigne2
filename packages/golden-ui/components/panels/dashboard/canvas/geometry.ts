import type { NodeId, UiNodeDto } from '../../../../types';
import {
	cssValueToPx,
	pxToCssValue,
	type CssUnitConversionContext,
	type CssValueData
} from '../../../../css-value';
import type { DashboardWidgetCreationPlacement } from '../dashboard-actions';
import type { DashboardAnchor, DashboardPlacement } from '../dashboard-model';
import type {
	FreeLayoutPreview,
	FreeLayoutTarget,
	FreeLayoutPreviewSet,
	FreeLayoutInteraction,
	FreeLayoutRect,
	FreeLayoutResizeZone
} from './contracts';

export const minFreeWidgetWidthRem = 0.5;

export const minFreeWidgetHeightRem = 0.5;

export const freeLayoutResizeZones: FreeLayoutResizeZone[] = [
	{ name: 'north-west', edges: { left: true, right: false, top: true, bottom: false } },
	{ name: 'north', edges: { left: false, right: false, top: true, bottom: false } },
	{ name: 'north-east', edges: { left: false, right: true, top: true, bottom: false } },
	{ name: 'east', edges: { left: false, right: true, top: false, bottom: false } },
	{ name: 'south-east', edges: { left: false, right: true, top: false, bottom: true } },
	{ name: 'south', edges: { left: false, right: false, top: false, bottom: true } },
	{ name: 'south-west', edges: { left: true, right: false, top: false, bottom: true } },
	{ name: 'west', edges: { left: true, right: false, top: false, bottom: false } }
];

export const normalizeClientRect = (
	startClient: [number, number],
	currentClient: [number, number]
): FreeLayoutRect => {
	const left = Math.min(startClient[0], currentClient[0]);
	const top = Math.min(startClient[1], currentClient[1]);
	return {
		left,
		top,
		width: Math.abs(currentClient[0] - startClient[0]),
		height: Math.abs(currentClient[1] - startClient[1])
	};
};

export const rectsIntersect = (
	left: DOMRect | FreeLayoutRect,
	right: DOMRect | FreeLayoutRect
): boolean => {
	return (
		left.left < right.left + right.width &&
		left.left + left.width > right.left &&
		left.top < right.top + right.height &&
		left.top + left.height > right.top
	);
};

export const createCssUnitContext = (
	interaction: Pick<
		FreeLayoutInteraction,
		| 'rootRemPx'
		| 'emPx'
		| 'surfaceWidthPx'
		| 'surfaceHeightPx'
		| 'viewportWidthPx'
		| 'viewportHeightPx'
	>,
	axis: 'x' | 'y'
): CssUnitConversionContext => ({
	rootRemPx: interaction.rootRemPx,
	emPx: interaction.emPx,
	axisBasePx: axis === 'x' ? interaction.surfaceWidthPx : interaction.surfaceHeightPx,
	viewportWidthPx: interaction.viewportWidthPx,
	viewportHeightPx: interaction.viewportHeightPx
});

export const sameNodeIdList = (left: NodeId[], right: NodeId[]): boolean => {
	if (left.length !== right.length) {
		return false;
	}
	for (let index = 0; index < left.length; index += 1) {
		if (left[index] !== right[index]) {
			return false;
		}
	}
	return true;
};

export const moveNodeIdInList = (
	nodeIds: NodeId[],
	nodeId: NodeId,
	targetIndex: number | null
): NodeId[] => {
	const nextNodeIds = nodeIds.filter((candidate) => candidate !== nodeId);
	const nextIndex =
		targetIndex === null
			? nextNodeIds.length
			: Math.max(0, Math.min(nextNodeIds.length, Math.round(targetIndex)));
	nextNodeIds.splice(nextIndex, 0, nodeId);
	return nextNodeIds;
};

export const isFreeLayoutTransformableWidget = (candidate: UiNodeDto): boolean => {
	return (
		candidate.node_type === 'dashboard_widget_container' ||
		candidate.node_type === 'dashboard_node_widget' ||
		candidate.node_type === 'dashboard_generic_widget'
	);
};

export const cloneFreeLayoutPreview = (preview: FreeLayoutPreview): FreeLayoutPreview => ({
	position: {
		x: preview.position.x,
		y: preview.position.y
	},
	size: {
		width: { ...preview.size.width },
		height: { ...preview.size.height }
	}
});

export const cloneFreeLayoutPreviewSet = (
	previewSet: FreeLayoutPreviewSet
): FreeLayoutPreviewSet => ({
	primaryNodeId: previewSet.primaryNodeId,
	previews: previewSet.previews.map((entry) => ({
		target: entry.target,
		preview: cloneFreeLayoutPreview(entry.preview)
	}))
});

export const resolveFreeLayoutPreview = (
	previewSet: FreeLayoutPreviewSet | null,
	nodeId: NodeId
): FreeLayoutPreview | null => {
	if (!previewSet) {
		return null;
	}
	for (const entry of previewSet.previews) {
		if (entry.target.nodeId === nodeId) {
			return entry.preview;
		}
	}
	return null;
};

export const buildPreviewSetFromTargets = (
	primaryNodeId: NodeId,
	targets: FreeLayoutTarget[]
): FreeLayoutPreviewSet => ({
	primaryNodeId,
	previews: targets.map((target) => ({
		target,
		preview: cloneFreeLayoutPreview(target.startPreview)
	}))
});

export const sameCssValue = (left: CssValueData, right: CssValueData): boolean =>
	left.value === right.value && left.unit === right.unit;

export const samePosition = (
	left: { x: number; y: number },
	right: { x: number; y: number }
): boolean => left.x === right.x && left.y === right.y;

export const samePlacementSize = (
	left: { width: CssValueData; height: CssValueData },
	right: { width: CssValueData; height: CssValueData }
): boolean => sameCssValue(left.width, right.width) && sameCssValue(left.height, right.height);

export const snapToGridPx = (valuePx: number, snapPx: number): number => {
	if (!(snapPx > 0)) {
		return valuePx;
	}
	return Math.round(valuePx / snapPx) * snapPx;
};

export const anchorXFromAnchor = (anchor: DashboardAnchor): 'left' | 'center' | 'right' => {
	if (anchor.endsWith('right')) {
		return 'right';
	}
	if (anchor === 'center' || anchor.endsWith('center')) {
		return 'center';
	}
	return 'left';
};

export const anchorYFromAnchor = (anchor: DashboardAnchor): 'top' | 'center' | 'bottom' => {
	if (anchor.startsWith('bottom')) {
		return 'bottom';
	}
	if (anchor === 'center' || anchor.startsWith('center')) {
		return 'center';
	}
	return 'top';
};

export const anchorBasePx = (
	anchor: 'left' | 'center' | 'right' | 'top' | 'center' | 'bottom',
	axisExtentPx: number
): number => {
	if (anchor === 'center') {
		return axisExtentPx * 0.5;
	}
	if (anchor === 'right' || anchor === 'bottom') {
		return axisExtentPx;
	}
	return 0;
};

export const anchorOffsetPx = (
	anchor: 'left' | 'center' | 'right' | 'top' | 'center' | 'bottom',
	sizePx: number
): number => {
	if (anchor === 'center') {
		return sizePx * 0.5;
	}
	if (anchor === 'right' || anchor === 'bottom') {
		return sizePx;
	}
	return 0;
};

export const anchorBaseExpression = (
	anchor: 'left' | 'center' | 'right' | 'top' | 'center' | 'bottom'
): string => {
	if (anchor === 'center') {
		return '50%';
	}
	if (anchor === 'right' || anchor === 'bottom') {
		return '100%';
	}
	return '0px';
};

export const freeLayoutRectFromPlacement = (
	placementLike: {
		position: { x: number; y: number };
		size: { width: CssValueData; height: CssValueData };
		anchor: DashboardAnchor;
	},
	interaction: Pick<
		FreeLayoutInteraction,
		| 'surfaceWidthPx'
		| 'surfaceHeightPx'
		| 'rootRemPx'
		| 'emPx'
		| 'viewportWidthPx'
		| 'viewportHeightPx'
	>
): FreeLayoutRect => {
	const xContext = createCssUnitContext(interaction, 'x');
	const yContext = createCssUnitContext(interaction, 'y');
	const widthPx = Math.max(0, cssValueToPx(placementLike.size.width, 'x', xContext));
	const heightPx = Math.max(0, cssValueToPx(placementLike.size.height, 'y', yContext));
	const anchorX = anchorXFromAnchor(placementLike.anchor);
	const anchorY = anchorYFromAnchor(placementLike.anchor);
	const anchorPointXPx =
		anchorBasePx(anchorX, interaction.surfaceWidthPx) + placementLike.position.x;
	const anchorPointYPx =
		anchorBasePx(anchorY, interaction.surfaceHeightPx) + placementLike.position.y;
	return {
		left: anchorPointXPx - anchorOffsetPx(anchorX, widthPx),
		top: anchorPointYPx - anchorOffsetPx(anchorY, heightPx),
		width: widthPx,
		height: heightPx
	};
};

export const freeLayoutPreviewFromRect = (
	rect: FreeLayoutRect,
	anchor: DashboardAnchor,
	interaction: Pick<
		FreeLayoutInteraction,
		| 'surfaceWidthPx'
		| 'surfaceHeightPx'
		| 'rootRemPx'
		| 'emPx'
		| 'viewportWidthPx'
		| 'viewportHeightPx'
	>,
	positionTemplate: { x: number; y: number },
	sizeTemplate: { width: CssValueData; height: CssValueData }
): FreeLayoutPreview => {
	const xContext = createCssUnitContext(interaction, 'x');
	const yContext = createCssUnitContext(interaction, 'y');
	const anchorX = anchorXFromAnchor(anchor);
	const anchorY = anchorYFromAnchor(anchor);
	const anchorPointXPx = rect.left + anchorOffsetPx(anchorX, rect.width);
	const anchorPointYPx = rect.top + anchorOffsetPx(anchorY, rect.height);
	return {
		position: {
			x: anchorPointXPx - anchorBasePx(anchorX, interaction.surfaceWidthPx),
			y: anchorPointYPx - anchorBasePx(anchorY, interaction.surfaceHeightPx)
		},
		size: {
			width: pxToCssValue(rect.width, sizeTemplate.width, 'x', xContext),
			height: pxToCssValue(rect.height, sizeTemplate.height, 'y', yContext)
		}
	};
};

export const buildSelectionRect = (targets: FreeLayoutTarget[]): FreeLayoutRect | null => {
	if (targets.length === 0) {
		return null;
	}
	let left = Number.POSITIVE_INFINITY;
	let top = Number.POSITIVE_INFINITY;
	let right = Number.NEGATIVE_INFINITY;
	let bottom = Number.NEGATIVE_INFINITY;
	for (const target of targets) {
		left = Math.min(left, target.startRect.left);
		top = Math.min(top, target.startRect.top);
		right = Math.max(right, target.startRect.left + target.startRect.width);
		bottom = Math.max(bottom, target.startRect.top + target.startRect.height);
	}
	return {
		left,
		top,
		width: Math.max(0, right - left),
		height: Math.max(0, bottom - top)
	};
};

export const buildFreeLayoutMovePreview = (
	interaction: FreeLayoutInteraction,
	deltaXPx: number,
	deltaYPx: number,
	snapPx: number
): FreeLayoutPreviewSet => {
	const primaryTarget =
		interaction.targets.find((target) => target.nodeId === interaction.primaryNodeId) ??
		interaction.targets[0];
	let appliedDeltaX = deltaXPx;
	let appliedDeltaY = deltaYPx;
	if (snapPx > 0) {
		appliedDeltaX =
			snapToGridPx(primaryTarget.startRect.left + appliedDeltaX, snapPx) -
			primaryTarget.startRect.left;
		appliedDeltaY =
			snapToGridPx(primaryTarget.startRect.top + appliedDeltaY, snapPx) -
			primaryTarget.startRect.top;
	}
	return {
		primaryNodeId: interaction.primaryNodeId,
		previews: interaction.targets.map((target) => ({
			target,
			preview: freeLayoutPreviewFromRect(
				{
					...target.startRect,
					left: target.startRect.left + appliedDeltaX,
					top: target.startRect.top + appliedDeltaY
				},
				target.anchor,
				interaction,
				target.startPreview.position,
				target.startPreview.size
			)
		}))
	};
};

export const buildFreeLayoutResizePreview = (
	interaction: FreeLayoutInteraction,
	deltaXPx: number,
	deltaYPx: number,
	snapPx: number
): FreeLayoutPreviewSet | null => {
	const resizeEdges = interaction.resizeEdges;
	if (!resizeEdges) {
		return null;
	}
	const minWidthPx = minFreeWidgetWidthRem * interaction.rootRemPx;
	const minHeightPx = minFreeWidgetHeightRem * interaction.rootRemPx;
	const startRect = interaction.selectionRect;
	let requiredScaleX = 0;
	let requiredScaleY = 0;
	for (const target of interaction.targets) {
		requiredScaleX = Math.max(
			requiredScaleX,
			target.startRect.width > 1e-6 ? minWidthPx / target.startRect.width : 1
		);
		requiredScaleY = Math.max(
			requiredScaleY,
			target.startRect.height > 1e-6 ? minHeightPx / target.startRect.height : 1
		);
	}
	const groupMinWidthPx = Math.max(minWidthPx, startRect.width * Math.max(requiredScaleX, 1e-6));
	const groupMinHeightPx = Math.max(minHeightPx, startRect.height * Math.max(requiredScaleY, 1e-6));
	let nextLeftPx = startRect.left;
	let nextTopPx = startRect.top;
	let nextWidthPx = startRect.width;
	let nextHeightPx = startRect.height;

	if (resizeEdges.left) {
		const appliedDeltaX = Math.min(deltaXPx, nextWidthPx - groupMinWidthPx);
		nextLeftPx += appliedDeltaX;
		nextWidthPx -= appliedDeltaX;
	}
	if (resizeEdges.right) {
		nextWidthPx = Math.max(groupMinWidthPx, nextWidthPx + deltaXPx);
	}
	if (resizeEdges.top) {
		const appliedDeltaY = Math.min(deltaYPx, nextHeightPx - groupMinHeightPx);
		nextTopPx += appliedDeltaY;
		nextHeightPx -= appliedDeltaY;
	}
	if (resizeEdges.bottom) {
		nextHeightPx = Math.max(groupMinHeightPx, nextHeightPx + deltaYPx);
	}
	if (snapPx > 0) {
		// Snap the edited edge and retain the opposite authored edge, including off-grid origins.
		if (resizeEdges.left) {
			const right = nextLeftPx + nextWidthPx;
			nextLeftPx = Math.min(snapToGridPx(nextLeftPx, snapPx), right - groupMinWidthPx);
			nextWidthPx = right - nextLeftPx;
		} else if (resizeEdges.right)
			nextWidthPx = Math.max(
				groupMinWidthPx,
				snapToGridPx(nextLeftPx + nextWidthPx, snapPx) - nextLeftPx
			);
		if (resizeEdges.top) {
			const bottom = nextTopPx + nextHeightPx;
			nextTopPx = Math.min(snapToGridPx(nextTopPx, snapPx), bottom - groupMinHeightPx);
			nextHeightPx = bottom - nextTopPx;
		} else if (resizeEdges.bottom)
			nextHeightPx = Math.max(
				groupMinHeightPx,
				snapToGridPx(nextTopPx + nextHeightPx, snapPx) - nextTopPx
			);
	}
	const scaleX = startRect.width > 1e-6 ? nextWidthPx / startRect.width : 1;
	const scaleY = startRect.height > 1e-6 ? nextHeightPx / startRect.height : 1;
	return {
		primaryNodeId: interaction.primaryNodeId,
		previews: interaction.targets.map((target) => {
			const nextRect: FreeLayoutRect = {
				left: nextLeftPx + (target.startRect.left - startRect.left) * scaleX,
				top: nextTopPx + (target.startRect.top - startRect.top) * scaleY,
				width: Math.max(minWidthPx, target.startRect.width * scaleX),
				height: Math.max(minHeightPx, target.startRect.height * scaleY)
			};
			return {
				target,
				preview: freeLayoutPreviewFromRect(
					nextRect,
					target.anchor,
					interaction,
					target.startPreview.position,
					target.startPreview.size
				)
			};
		})
	};
};

export const sumCssLengthExpressions = (expressions: string[]): string => {
	if (expressions.length === 0) {
		return '0px';
	}
	if (expressions.length === 1) {
		return expressions[0];
	}
	return expressions.map((expression) => `(${expression})`).join(' + ');
};

export const placementHasExplicitSize = (
	axis: 'horizontal' | 'vertical',
	placementLike: DashboardPlacement | DashboardWidgetCreationPlacement
): boolean => {
	if (!('sizeEnabled' in placementLike) || placementLike.sizeEnabled === undefined) {
		return false;
	}
	return axis === 'horizontal' ? placementLike.sizeEnabled.width : placementLike.sizeEnabled.height;
};
