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
	const next = Math.min(64, Math.max(0.05, zoom));
	const ratio = next / camera.zoom;
	return {
		zoom: next,
		pan: {
			x: anchor.x - (anchor.x - camera.pan.x) * ratio,
			y: anchor.y - (anchor.y - camera.pan.y) * ratio
		}
	};
}

export function frameBounds(bounds: ViewBounds, aspect: number): ViewCamera | null {
	if (
		![bounds.x, bounds.y, bounds.width, bounds.height, aspect].every(Number.isFinite) ||
		bounds.width <= 0 ||
		bounds.height <= 0 ||
		aspect <= 0
	)
		return null;
	// Pan is normalized to fitted content dimensions; the component converts it to CSS space.
	const zoom = Math.min(64, Math.max(0.05, 0.85 / Math.max(bounds.width, bounds.height)));
	return {
		zoom,
		pan: {
			x: (0.5 - bounds.x - bounds.width / 2) * zoom,
			y: (0.5 - bounds.y - bounds.height / 2) * zoom
		}
	};
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
