import type { UiNodeDto } from 'golden_ui';
import type {
	GraphNodePosition,
	GraphWorldBounds,
	GraphWorldContentContext
} from 'golden_graph_ui';
import type {
	SpatialEndpoint,
	Point2,
	VoronoiCell,
	DeadzoneHole,
	DebugVoronoiComputation,
	DebugVoronoiGuide,
	DebugConnection,
	RelatedValue
} from './types';
import { clamp01, floatValue, relatedValueAmount } from './projection.svelte';
import {
	MODE_VORONOI,
	MODE_TARGET_RADIUS,
	MODE_SOURCE_RADIUS,
	MODE_OVERLAP,
	VORONOI_TIE_EPSILON,
	VORONOI_EDGE_EPSILON,
	DEBUG_WEIGHT_EPSILON,
	SPATIALIZER_UNIT_REM,
	DEFAULT_SPATIAL_BOUNDS,
	TARGET_ZONE_PADDING_UNITS,
	ENDPOINT_BOUNDS_PADDING_UNITS
} from './constants';

export const graphPositionToSpatialPosition = (position: GraphNodePosition): Point2 => ({
	x: position.x / SPATIALIZER_UNIT_REM,
	y: position.y / SPATIALIZER_UNIT_REM
});

interface Source {
	endpoints: SpatialEndpoint[];
	targets: SpatialEndpoint[];
	sources: SpatialEndpoint[];
	enabledTargets: SpatialEndpoint[];
	spatializerMode: string;
	debugView: boolean;
	selectedEndpoint: SpatialEndpoint | null;
	relatedValues: RelatedValue[];
	valueParameterForPair(target: SpatialEndpoint, source: SpatialEndpoint): UiNodeDto | null;
}
export function createSpatializerGeometry(getSource: () => Source) {
	let endpoints = $derived(getSource().endpoints);
	let targets = $derived(getSource().targets);
	let sources = $derived(getSource().sources);
	let enabledTargets = $derived(getSource().enabledTargets);
	let spatializerMode = $derived(getSource().spatializerMode);
	let debugView = $derived(getSource().debugView);
	let selectedEndpoint = $derived(getSource().selectedEndpoint);
	let relatedValues = $derived(getSource().relatedValues);
	const valueParameterForPair = (target: SpatialEndpoint, source: SpatialEndpoint) =>
		getSource().valueParameterForPair(target, source);
	const endpointDistance = (left: SpatialEndpoint, right: SpatialEndpoint): number =>
		Math.hypot(right.x - left.x, right.y - left.y);

	const pointBetween = (start: Point2, end: Point2, fraction: number): Point2 => ({
		x: start.x + (end.x - start.x) * fraction,
		y: start.y + (end.y - start.y) * fraction
	});

	const nearestTargetTo = (endpoint: SpatialEndpoint): SpatialEndpoint | null => {
		let best: SpatialEndpoint | null = null;
		let bestDistance = Infinity;
		for (const target of enabledTargets) {
			const distance = endpointDistance(endpoint, target);
			if (distance < bestDistance) {
				best = target;
				bestDistance = distance;
			}
		}
		return best;
	};

	let spatialWorldBounds = $derived.by((): GraphWorldBounds => {
		let left = DEFAULT_SPATIAL_BOUNDS.left;
		let top = DEFAULT_SPATIAL_BOUNDS.top;
		let right = DEFAULT_SPATIAL_BOUNDS.right;
		let bottom = DEFAULT_SPATIAL_BOUNDS.bottom;
		for (const endpoint of endpoints) {
			const radius = Math.max(endpoint.radius ?? 0, endpoint.freezeRadius ?? 0);
			left = Math.min(left, endpoint.x - radius);
			top = Math.min(top, endpoint.y - radius);
			right = Math.max(right, endpoint.x + radius);
			bottom = Math.max(bottom, endpoint.y + radius);
		}
		return {
			left: left - ENDPOINT_BOUNDS_PADDING_UNITS,
			top: top - ENDPOINT_BOUNDS_PADDING_UNITS,
			right: right + ENDPOINT_BOUNDS_PADDING_UNITS,
			bottom: bottom + ENDPOINT_BOUNDS_PADDING_UNITS
		};
	});

	const spatialBoundsToGraphBounds = (bounds: GraphWorldBounds): GraphWorldBounds => ({
		left: bounds.left * SPATIALIZER_UNIT_REM,
		top: bounds.top * SPATIALIZER_UNIT_REM,
		right: bounds.right * SPATIALIZER_UNIT_REM,
		bottom: bounds.bottom * SPATIALIZER_UNIT_REM
	});

	let graphWorldBounds = $derived(spatialBoundsToGraphBounds(spatialWorldBounds));

	const rectanglePolygon = (bounds: GraphWorldBounds): Point2[] => [
		{ x: bounds.left, y: bounds.top },
		{ x: bounds.right, y: bounds.top },
		{ x: bounds.right, y: bounds.bottom },
		{ x: bounds.left, y: bounds.bottom }
	];

	const closerHalfPlaneValue = (point: Point2, site: Point2, other: Point2): number => {
		const dx = other.x - site.x;
		const dy = other.y - site.y;
		return (
			2 * (dx * point.x + dy * point.y) -
			(other.x * other.x + other.y * other.y - site.x * site.x - site.y * site.y)
		);
	};

	const clipVoronoiCell = (polygon: Point2[], target: Point2, other: Point2): Point2[] => {
		const dx = other.x - target.x;
		const dy = other.y - target.y;
		if (Math.hypot(dx, dy) < VORONOI_EDGE_EPSILON) {
			return polygon;
		}
		const valueAt = (point: Point2): number => closerHalfPlaneValue(point, target, other);
		const inside = (point: Point2): boolean => valueAt(point) <= VORONOI_TIE_EPSILON;
		const intersect = (start: Point2, end: Point2): Point2 => {
			const startValue = valueAt(start);
			const endValue = valueAt(end);
			const denominator = startValue - endValue;
			if (Math.abs(denominator) <= VORONOI_TIE_EPSILON) {
				return end;
			}
			const t = Math.min(1, Math.max(0, startValue / denominator));
			return {
				x: start.x + (end.x - start.x) * t,
				y: start.y + (end.y - start.y) * t
			};
		};

		const result: Point2[] = [];
		for (let index = 0; index < polygon.length; index += 1) {
			const current = polygon[index];
			const previous = polygon[(index + polygon.length - 1) % polygon.length];
			const currentInside = inside(current);
			const previousInside = inside(previous);
			if (currentInside && !previousInside) {
				result.push(intersect(previous, current));
			}
			if (currentInside) {
				result.push(current);
			} else if (previousInside) {
				result.push(intersect(previous, current));
			}
		}
		return result;
	};

	const voronoiBoundsForSource = (source: SpatialEndpoint): GraphWorldBounds => {
		let left = source.x;
		let right = source.x;
		let top = source.y;
		let bottom = source.y;
		for (const target of enabledTargets) {
			left = Math.min(left, target.x);
			right = Math.max(right, target.x);
			top = Math.min(top, target.y);
			bottom = Math.max(bottom, target.y);
		}
		const padding = Math.max(Math.abs(right - left), Math.abs(bottom - top), 1) * 4;
		return {
			left: left - padding,
			top: top - padding,
			right: right + padding,
			bottom: bottom + padding
		};
	};

	const voronoiCellPolygon = (target: SpatialEndpoint, bounds: GraphWorldBounds): Point2[] => {
		let polygon = rectanglePolygon(bounds);
		for (const other of enabledTargets) {
			if (other.key === target.key) {
				continue;
			}
			polygon = clipVoronoiCell(polygon, target, other);
			if (polygon.length < 3) {
				break;
			}
		}
		return polygon;
	};

	const distancesTie = (left: number, right: number): boolean =>
		Number.isFinite(left) &&
		Number.isFinite(right) &&
		Math.abs(left - right) <= VORONOI_TIE_EPSILON;

	const targetWeightForSource = (target: SpatialEndpoint, source: SpatialEndpoint): number =>
		relatedValueAmount(valueParameterForPair(target, source));

	const sourceWeightTotal = (source: SpatialEndpoint): number =>
		targets.reduce((total, target) => total + targetWeightForSource(target, source), 0);

	const activeRadiusForTarget = (source: SpatialEndpoint, target: SpatialEndpoint): number => {
		if (spatializerMode === MODE_TARGET_RADIUS) {
			return Math.max(0, target.radius ?? 0);
		}
		if (spatializerMode === MODE_SOURCE_RADIUS) {
			return Math.max(0, source.radius ?? 0);
		}
		if (spatializerMode === MODE_OVERLAP) {
			return Math.max(0, source.radius ?? 0) + Math.max(0, target.radius ?? 0);
		}
		return 0;
	};

	const deadzoneHolesForSource = (source: SpatialEndpoint): DeadzoneHole[] =>
		enabledTargets.flatMap((target) => {
			const radius = activeRadiusForTarget(source, target);
			return radius > 0
				? [{ key: target.key, x: target.x, y: target.y, radius, color: target.color }]
				: [];
		});

	const selectedDeadzoneSource = (): SpatialEndpoint | null =>
		selectedEndpoint?.kind === 'source' &&
		selectedEndpoint.enabled &&
		spatializerMode !== MODE_VORONOI &&
		sourceWeightTotal(selectedEndpoint) <= DEBUG_WEIGHT_EPSILON
			? selectedEndpoint
			: null;

	const selectedVoronoiDeadzoneSource = (): SpatialEndpoint | null =>
		selectedEndpoint?.kind === 'source' &&
		selectedEndpoint.enabled &&
		spatializerMode === MODE_VORONOI &&
		frozenTargetsForSource(selectedEndpoint).length > 0
			? selectedEndpoint
			: null;

	const highlightedVoronoiCell = (cell: VoronoiCell): boolean => {
		if (!selectedEndpoint || spatializerMode !== MODE_VORONOI) {
			return false;
		}
		if (selectedEndpoint.kind === 'target') {
			return cell.key === selectedEndpoint.key;
		}
		return nearestTargetTo(selectedEndpoint)?.key === cell.key;
	};

	const frozenTargetsForSource = (source: SpatialEndpoint): SpatialEndpoint[] => {
		const frozenDistances = enabledTargets.flatMap((target) => {
			const freezeRadius = Math.max(0, target.freezeRadius ?? 0);
			const distance = endpointDistance(source, target);
			return distance <= freezeRadius + VORONOI_TIE_EPSILON ? [distance] : [];
		});
		const frozenDistance = frozenDistances.reduce(
			(best, distance) => Math.min(best, distance),
			Infinity
		);
		if (!Number.isFinite(frozenDistance)) {
			return [];
		}
		return enabledTargets.filter((target) =>
			distancesTie(endpointDistance(source, target), frozenDistance)
		);
	};

	const closestPointOnSegment = (point: Point2, start: Point2, end: Point2): Point2 => {
		const dx = end.x - start.x;
		const dy = end.y - start.y;
		const lengthSquared = dx * dx + dy * dy;
		if (lengthSquared <= VORONOI_TIE_EPSILON) {
			return start;
		}
		const t = Math.min(
			1,
			Math.max(0, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared)
		);
		return { x: start.x + dx * t, y: start.y + dy * t };
	};

	const closestCellEdgeForNeighbor = (
		source: SpatialEndpoint,
		current: SpatialEndpoint,
		neighbor: SpatialEndpoint,
		cell: Point2[]
	): Omit<DebugVoronoiGuide, 'key' | 'source' | 'current' | 'neighbor' | 'weight'> | null => {
		let best: {
			edgeStart: Point2;
			edgeEnd: Point2;
			closestPoint: Point2;
			neighborMeasurePoint: Point2;
			edgeDistance: number;
			neighborDistance: number;
		} | null = null;
		for (let edgeIndex = 0; edgeIndex < cell.length; edgeIndex += 1) {
			const edgeStart = cell[edgeIndex];
			const edgeEnd = cell[(edgeIndex + 1) % cell.length];
			const startValue = Math.abs(closerHalfPlaneValue(edgeStart, current, neighbor));
			const endValue = Math.abs(closerHalfPlaneValue(edgeEnd, current, neighbor));
			if (startValue > VORONOI_EDGE_EPSILON || endValue > VORONOI_EDGE_EPSILON) {
				continue;
			}
			const closestPoint = closestPointOnSegment(source, edgeStart, edgeEnd);
			const edgeDistance = Math.hypot(closestPoint.x - source.x, closestPoint.y - source.y);
			if (best && edgeDistance >= best.edgeDistance) {
				continue;
			}
			const rawNeighborDistance = Math.hypot(
				closestPoint.x - neighbor.x,
				closestPoint.y - neighbor.y
			);
			const neighborDistance = Math.max(
				0,
				rawNeighborDistance - Math.max(0, neighbor.freezeRadius ?? 0)
			);
			const neighborFraction =
				rawNeighborDistance <= VORONOI_TIE_EPSILON ? 0 : neighborDistance / rawNeighborDistance;
			const neighborMeasurePoint = pointBetween(closestPoint, neighbor, neighborFraction);
			best = {
				edgeStart,
				edgeEnd,
				closestPoint,
				neighborMeasurePoint,
				edgeDistance,
				neighborDistance
			};
		}
		return best;
	};

	const voronoiViewBounds = (world: GraphWorldContentContext): GraphWorldBounds => {
		const zoom = Math.max(0.000001, world.camera.zoom);
		const left = -world.camera.x / zoom / world.remPx / SPATIALIZER_UNIT_REM;
		const top = -world.camera.y / zoom / world.remPx / SPATIALIZER_UNIT_REM;
		const right =
			(world.viewportWidth - world.camera.x) / zoom / world.remPx / SPATIALIZER_UNIT_REM;
		const bottom =
			(world.viewportHeight - world.camera.y) / zoom / world.remPx / SPATIALIZER_UNIT_REM;
		return {
			left: left - TARGET_ZONE_PADDING_UNITS,
			top: top - TARGET_ZONE_PADDING_UNITS,
			right: right + TARGET_ZONE_PADDING_UNITS,
			bottom: bottom + TARGET_ZONE_PADDING_UNITS
		};
	};

	const deadzonePath = (
		bounds: GraphWorldBounds,
		holes: DeadzoneHole[],
		world: GraphWorldContentContext
	): string => {
		const left = pointPx(bounds.left, world);
		const top = pointPx(bounds.top, world);
		const right = pointPx(bounds.right, world);
		const bottom = pointPx(bounds.bottom, world);
		const rect = `M ${left} ${top} H ${right} V ${bottom} H ${left} Z`;
		const circles = holes
			.map((hole) => {
				const radius = pointPx(Math.max(0, hole.radius), world);
				const x = pointPx(hole.x, world);
				const y = pointPx(hole.y, world);
				return radius > 0
					? `M ${x - radius} ${y} a ${radius} ${radius} 0 1 0 ${radius * 2} 0 a ${radius} ${radius} 0 1 0 ${-radius * 2} 0`
					: '';
			})
			.filter(Boolean)
			.join(' ');
		return `${rect} ${circles}`;
	};

	const voronoiCellsForBounds = (
		bounds: GraphWorldBounds,
		candidates: readonly SpatialEndpoint[] = enabledTargets
	): VoronoiCell[] => {
		if (spatializerMode !== MODE_VORONOI || enabledTargets.length === 0) {
			return [];
		}
		return candidates.flatMap((target) => {
			let polygon = rectanglePolygon(bounds);
			for (const other of enabledTargets) {
				if (other.key === target.key) {
					continue;
				}
				polygon = clipVoronoiCell(polygon, target, other);
				if (polygon.length === 0) {
					break;
				}
			}
			return polygon.length > 0 ? [{ key: target.key, color: target.color, points: polygon }] : [];
		});
	};

	const debugSources = (): SpatialEndpoint[] => {
		if (!debugView || spatializerMode !== MODE_VORONOI || !selectedEndpoint) {
			return [];
		}
		if (selectedEndpoint.kind === 'source') {
			return selectedEndpoint.enabled ? [selectedEndpoint] : [];
		}
		return sources.filter((source) => source.enabled);
	};

	const debugVoronoiComputationForSource = (
		source: SpatialEndpoint
	): DebugVoronoiComputation | null => {
		const frozenTargets = frozenTargetsForSource(source);
		const current = nearestTargetTo(source);
		if (!current) {
			return null;
		}
		const bounds = voronoiBoundsForSource(source);
		const cellPoints = voronoiCellPolygon(current, bounds);
		const guides =
			cellPoints.length < 3
				? []
				: enabledTargets.flatMap((neighbor) => {
						if (neighbor.key === current.key) {
							return [];
						}
						const edge = closestCellEdgeForNeighbor(source, current, neighbor, cellPoints);
						if (!edge) {
							return [];
						}
						return [
							{
								key: `${source.key}:${current.key}:${neighbor.key}`,
								source,
								current,
								neighbor,
								weight: targetWeightForSource(neighbor, source),
								...edge
							}
						];
					});
		return {
			key: source.key,
			source,
			current,
			frozenTargets,
			cellPoints,
			guides
		};
	};

	const computationIncludesTarget = (
		computation: DebugVoronoiComputation,
		target: SpatialEndpoint
	): boolean =>
		computation.current?.key === target.key ||
		computation.frozenTargets.some((frozenTarget) => frozenTarget.key === target.key) ||
		computation.guides.some((guide) => guide.neighbor.key === target.key);

	let debugVoronoiComputations = $derived.by((): DebugVoronoiComputation[] =>
		debugSources().flatMap((source) => {
			const computation = debugVoronoiComputationForSource(source);
			if (!computation) {
				return [];
			}
			if (
				selectedEndpoint?.kind === 'target' &&
				!computationIncludesTarget(computation, selectedEndpoint)
			) {
				return [];
			}
			return [computation];
		})
	);

	let debugVoronoiGuides = $derived.by((): DebugVoronoiGuide[] =>
		debugVoronoiComputations.flatMap((computation) => computation.guides)
	);

	let debugConnections = $derived.by((): DebugConnection[] => {
		if (!debugView || !selectedEndpoint) {
			return [];
		}
		if (spatializerMode === MODE_VORONOI) {
			return debugVoronoiComputations.flatMap((computation): DebugConnection[] => {
				if (computation.frozenTargets.length > 0) {
					return computation.frozenTargets.map((target): DebugConnection => ({
						key: `${computation.source.key}:${target.key}:frozen`,
						source: computation.source,
						target,
						weight: targetWeightForSource(target, computation.source),
						role: 'frozen' as const
					}));
				}
				const currentConnections: DebugConnection[] = computation.current
					? [
							{
								key: `${computation.source.key}:${computation.current.key}:current`,
								source: computation.source,
								target: computation.current,
								weight: targetWeightForSource(computation.current, computation.source),
								role: 'current' as const
							}
						]
					: [];
				const neighborConnections: DebugConnection[] = computation.guides.flatMap(
					(guide): DebugConnection[] => [
						{
							key: `${guide.source.key}:${guide.neighbor.key}:neighbor`,
							source: guide.source,
							target: guide.neighbor,
							weight: guide.weight,
							role: 'neighbor' as const
						}
					]
				);
				return [...currentConnections, ...neighborConnections];
			});
		}
		if (selectedEndpoint.kind === 'source') {
			return relatedValues.map((item) => ({
				key: item.key,
				source: selectedEndpoint,
				target: item.endpoint,
				weight: relatedValueAmount(item.valueParam),
				role: 'direct' as const
			}));
		}
		return relatedValues.map((item) => ({
			key: item.key,
			source: item.endpoint,
			target: selectedEndpoint,
			weight: relatedValueAmount(item.valueParam),
			role: 'direct' as const
		}));
	});

	const debugLineOpacity = (weight: number): number =>
		weight <= DEBUG_WEIGHT_EPSILON ? 0.055 : 0.16 + clamp01(weight) * 0.74;

	const screenPx = (value: number, world: GraphWorldContentContext): number =>
		(value * world.remPx) / Math.max(0.000001, world.camera.zoom);

	const dashArray = (world: GraphWorldContentContext, ...values: number[]): string =>
		values.map((value) => `${screenPx(value, world)}`).join(' ');

	const debugConnectionDashArray = (
		role: DebugConnection['role'],
		world: GraphWorldContentContext
	): string | undefined => {
		if (role === 'neighbor') {
			return dashArray(world, 0.36, 0.2);
		}
		if (role === 'frozen') {
			return dashArray(world, 0.12, 0.14);
		}
		return undefined;
	};

	const debugLineWidth = (weight: number, world: GraphWorldContentContext): number =>
		screenPx(0.035 + clamp01(weight) * 0.17, world);

	const debugWeightLabel = (weight: number): string => `${Math.round(clamp01(weight) * 100)}%`;

	const debugDistanceLabel = (distance: number): string =>
		Number.isFinite(distance)
			? `d ${distance < 10 ? distance.toFixed(2) : distance.toFixed(1)}`
			: '';

	const pointPx = (value: number, world: GraphWorldContentContext): number =>
		value * SPATIALIZER_UNIT_REM * world.remPx;

	const polygonPoints = (points: Point2[], world: GraphWorldContentContext): string =>
		points.map((point) => `${pointPx(point.x, world)},${pointPx(point.y, world)}`).join(' ');

	return {
		voronoiCellsForBounds,
		voronoiViewBounds,
		highlightedVoronoiCell,
		polygonPoints,
		screenPx,
		selectedDeadzoneSource,
		deadzoneHolesForSource,
		deadzonePath,
		selectedVoronoiDeadzoneSource,
		frozenTargetsForSource,
		pointPx,
		dashArray,
		pointBetween,
		debugDistanceLabel,
		debugWeightLabel,
		debugLineOpacity,
		debugLineWidth,
		debugConnectionDashArray,
		get graphWorldBounds() {
			return graphWorldBounds;
		},
		get debugVoronoiComputations() {
			return debugVoronoiComputations;
		},
		get debugVoronoiGuides() {
			return debugVoronoiGuides;
		},
		get debugConnections() {
			return debugConnections;
		}
	};
}
export type SpatializerGeometry = ReturnType<typeof createSpatializerGeometry>;
