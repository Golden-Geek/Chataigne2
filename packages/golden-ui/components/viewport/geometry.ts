export interface ViewPoint {
	x: number;
	y: number;
}

/** Bounds expressed relative to the document's active area. */
export interface ViewBounds extends ViewPoint {
	width: number;
	height: number;
}

export interface ViewCamera {
	zoom: number;
	pan: ViewPoint;
}

export function fitSize(width: number, height: number, aspect: number) {
	if (!(width > 0 && height > 0 && Number.isFinite(aspect) && aspect > 0))
		return { width: 0, height: 0 };
	const fittedWidth = Math.min(width, height * aspect);
	return { width: fittedWidth, height: fittedWidth / aspect };
}

export function zoomAt(camera: ViewCamera, zoom: number, anchor: ViewPoint): ViewCamera {
	const next = cameraAtZoom(
		{ x: camera.pan.x, y: camera.pan.y, zoom: camera.zoom },
		zoom,
		anchor.x,
		anchor.y,
		0.05,
		64
	);
	return {
		zoom: next.zoom,
		pan: { x: next.x, y: next.y }
	};
}

export function frameBounds(
	bounds: ViewBounds,
	content: { width: number; height: number },
	viewport: { width: number; height: number },
	padding: number
): ViewCamera | null {
	if (
		![
			bounds.x,
			bounds.y,
			bounds.width,
			bounds.height,
			content.width,
			content.height,
			viewport.width,
			viewport.height,
			padding
		].every(Number.isFinite) ||
		Math.min(
			bounds.width,
			bounds.height,
			content.width,
			content.height,
			viewport.width,
			viewport.height
		) <= 0
	)
		return null;
	const next = cameraForBounds(
		{
			left: (bounds.x - 0.5) * content.width,
			top: (bounds.y - 0.5) * content.height,
			right: (bounds.x + bounds.width - 0.5) * content.width,
			bottom: (bounds.y + bounds.height - 0.5) * content.height
		},
		{ x: -viewport.width / 2, y: -viewport.height / 2, ...viewport },
		1,
		padding,
		0.05,
		64
	);
	return next ? { zoom: next.zoom, pan: { x: next.x, y: next.y } } : null;
}

export function pointBounds(points: readonly ViewPoint[]): ViewBounds | null {
	if (!points.length || points.some((point) => !Number.isFinite(point.x + point.y))) return null;
	const x = Math.min(...points.map((point) => point.x));
	const y = Math.min(...points.map((point) => point.y));
	return {
		x,
		y,
		width: Math.max(...points.map((point) => point.x)) - x,
		height: Math.max(...points.map((point) => point.y)) - y
	};
}
import { cameraAtZoom, cameraForBounds } from 'golden_graph_ui/camera-geometry';
