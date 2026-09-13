import type { GraphCamera, GraphNode, GraphViewportInset, GraphWorldBounds } from './types';

export interface VisibleViewport {
	x: number;
	y: number;
	width: number;
	height: number;
}

const clamp = (value: number, minimum: number, maximum: number): number =>
	Math.min(maximum, Math.max(minimum, value));

const finiteNumber = (value: number | undefined, fallback: number): number =>
	typeof value === 'number' && Number.isFinite(value) ? value : fallback;

export const visibleViewport = (
	inset: GraphViewportInset,
	viewportWidth: number,
	viewportHeight: number
): VisibleViewport => {
	const left = Math.max(0, finiteNumber(inset.left, 0));
	const right = Math.max(0, finiteNumber(inset.right, 0));
	const top = Math.max(0, finiteNumber(inset.top, 0));
	const bottom = Math.max(0, finiteNumber(inset.bottom, 0));
	const horizontalScale = Math.min(1, viewportWidth / Math.max(1, left + right));
	const verticalScale = Math.min(1, viewportHeight / Math.max(1, top + bottom));
	const scaledLeft = left * horizontalScale;
	const scaledRight = right * horizontalScale;
	const scaledTop = top * verticalScale;
	const scaledBottom = bottom * verticalScale;
	return {
		x: scaledLeft,
		y: scaledTop,
		width: Math.max(1, viewportWidth - scaledLeft - scaledRight),
		height: Math.max(1, viewportHeight - scaledTop - scaledBottom)
	};
};

export const cameraAtZoom = (
	camera: GraphCamera,
	zoom: number,
	anchorX: number,
	anchorY: number,
	minimumZoom: number,
	maximumZoom: number
): GraphCamera => {
	const nextZoom = clamp(zoom, minimumZoom, maximumZoom);
	const worldX = (anchorX - camera.x) / camera.zoom;
	const worldY = (anchorY - camera.y) / camera.zoom;
	return {
		x: anchorX - worldX * nextZoom,
		y: anchorY - worldY * nextZoom,
		zoom: nextZoom
	};
};

export const graphBoundsForNodes = (
	nodes: readonly GraphNode[],
	nodeWidth: (node: GraphNode) => number,
	nodeHeight: (node: GraphNode) => number
): GraphWorldBounds | null => {
	if (nodes.length === 0) {
		return null;
	}
	let left = Number.POSITIVE_INFINITY;
	let top = Number.POSITIVE_INFINITY;
	let right = Number.NEGATIVE_INFINITY;
	let bottom = Number.NEGATIVE_INFINITY;
	for (const node of nodes) {
		left = Math.min(left, node.position.x);
		top = Math.min(top, node.position.y);
		right = Math.max(right, node.position.x + nodeWidth(node));
		bottom = Math.max(bottom, node.position.y + nodeHeight(node));
	}
	return { left, top, right, bottom };
};

export const cameraForBounds = (
	bounds: GraphWorldBounds,
	viewport: VisibleViewport,
	remPx: number,
	paddingRem: number,
	minimumZoom: number,
	maximumZoom: number
): GraphCamera | null => {
	const left = Math.min(bounds.left, bounds.right);
	const top = Math.min(bounds.top, bounds.bottom);
	const right = Math.max(bounds.left, bounds.right);
	const bottom = Math.max(bounds.top, bounds.bottom);
	if (![left, top, right, bottom].every(Number.isFinite)) {
		return null;
	}
	const widthPx = Math.max(remPx, (right - left) * remPx);
	const heightPx = Math.max(remPx, (bottom - top) * remPx);
	const paddingPx = paddingRem * remPx;
	const zoom = clamp(
		Math.min(
			(viewport.width - paddingPx * 2) / widthPx,
			(viewport.height - paddingPx * 2) / heightPx
		),
		minimumZoom,
		maximumZoom
	);
	const centerX = (left + right) * 0.5 * remPx;
	const centerY = (top + bottom) * 0.5 * remPx;
	return {
		x: viewport.x + viewport.width * 0.5 - centerX * zoom,
		y: viewport.y + viewport.height * 0.5 - centerY * zoom,
		zoom
	};
};
