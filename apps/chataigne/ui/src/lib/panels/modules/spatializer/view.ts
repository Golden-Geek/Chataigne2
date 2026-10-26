import type { GraphWorldBounds, GraphWorldContentContext } from 'golden_graph_ui';
import type { SpatialEndpoint } from './types';
import { SPATIALIZER_UNIT_REM } from './constants';

export const ENDPOINT_GLYPH_LIMIT = 128;
export const REGION_POLYGON_LIMIT = 256;
const OVERVIEW_GRID = 16;

export interface EndpointGroup extends GraphWorldBounds {
	key: number;
	sources: number;
	targets: number;
}

export function spatialViewport(world: GraphWorldContentContext): GraphWorldBounds {
	const scale = world.remPx * SPATIALIZER_UNIT_REM * Math.max(0.000001, world.camera.zoom);
	const padding = world.remPx / scale;
	return {
		left: -world.camera.x / scale - padding,
		top: -world.camera.y / scale - padding,
		right: (world.viewportWidth - world.camera.x) / scale + padding,
		bottom: (world.viewportHeight - world.camera.y) / scale + padding
	};
}

/** Complete viewport membership, or a count-preserving overview with explicit retained controls. */
export function endpointView(
	endpoints: readonly SpatialEndpoint[],
	bounds: GraphWorldBounds,
	retainedKeys: readonly string[]
) {
	const byKey = new Map(endpoints.map((endpoint) => [endpoint.key, endpoint]));
	const retained = new Map<string, SpatialEndpoint>();
	for (const key of retainedKeys) {
		const endpoint = byKey.get(key);
		if (endpoint && retained.size < ENDPOINT_GLYPH_LIMIT) retained.set(key, endpoint);
	}
	const visible = endpoints.filter((endpoint) => {
		const radius = Math.max(0, endpoint.radius ?? 0, endpoint.freezeRadius ?? 0);
		return (
			endpoint.x + radius >= bounds.left &&
			endpoint.x - radius <= bounds.right &&
			endpoint.y + radius >= bounds.top &&
			endpoint.y - radius <= bounds.bottom
		);
	});
	const membership = new Map(retained);
	for (const endpoint of visible) membership.set(endpoint.key, endpoint);
	const overview = membership.size > ENDPOINT_GLYPH_LIMIT;
	const groups = new Map<number, EndpointGroup>();
	if (overview) {
		const width = Math.max(0.000001, bounds.right - bounds.left) / OVERVIEW_GRID;
		const height = Math.max(0.000001, bounds.bottom - bounds.top) / OVERVIEW_GRID;
		for (const endpoint of visible) {
			const column = Math.max(
				0,
				Math.min(OVERVIEW_GRID - 1, Math.floor((endpoint.x - bounds.left) / width))
			);
			const row = Math.max(
				0,
				Math.min(OVERVIEW_GRID - 1, Math.floor((endpoint.y - bounds.top) / height))
			);
			const key = row * OVERVIEW_GRID + column;
			let group = groups.get(key);
			if (!group) {
				group = {
					key,
					left: bounds.left + column * width,
					top: bounds.top + row * height,
					right: bounds.left + (column + 1) * width,
					bottom: bounds.top + (row + 1) * height,
					sources: 0,
					targets: 0
				};
				groups.set(key, group);
			}
			if (endpoint.kind === 'source') group.sources++;
			else group.targets++;
		}
	}
	return {
		overview,
		visibleCount: visible.length,
		glyphs: [...(overview ? retained : membership).values()],
		groups: [...groups.values()]
	};
}

/** Conservative candidates: a site farther than an entire reference cell cannot own this view. */
export function voronoiCandidates(
	targets: readonly SpatialEndpoint[],
	bounds: GraphWorldBounds,
	limit = REGION_POLYGON_LIMIT
): SpatialEndpoint[] | null {
	if (!targets.length) return [];
	const x = (bounds.left + bounds.right) / 2,
		y = (bounds.top + bounds.bottom) / 2;
	let reference = targets[0],
		distance = Infinity;
	for (const target of targets) {
		const next = (target.x - x) ** 2 + (target.y - y) ** 2;
		if (next < distance) {
			reference = target;
			distance = next;
		}
	}
	const upper = Math.max(
		...[bounds.left, bounds.right].flatMap((x) =>
			[bounds.top, bounds.bottom].map((y) => (reference.x - x) ** 2 + (reference.y - y) ** 2)
		)
	);
	const tolerance = Math.max(1e-8, upper * 1e-12);
	const candidates: SpatialEndpoint[] = [];
	for (const target of targets) {
		const dx = Math.max(bounds.left - target.x, 0, target.x - bounds.right);
		const dy = Math.max(bounds.top - target.y, 0, target.y - bounds.bottom);
		if (dx * dx + dy * dy <= upper + tolerance) {
			if (candidates.length === limit) return null;
			candidates.push(target);
		}
	}
	return candidates;
}
