import type { GraphNodePosition } from './types';

export interface RoutingObstacle {
	left: number;
	top: number;
	right: number;
	bottom: number;
}

interface RoutingState {
	x: number;
	y: number;
	direction: number;
	cost: number;
	estimate: number;
	key: string;
}

export const ROUTING_GRID_REM = 1;
const ROUTING_MARGIN_REM = 16;
const ROUTING_BUCKET_REM = 16;
const ROUTING_MAX_VISITS = 24_000;
const ROUTING_TURN_COST = 0.35;

export const routingObstacleBuckets = (
	obstacles: RoutingObstacle[],
	remPx: number
): Map<string, RoutingObstacle[]> => {
	const buckets = new Map<string, RoutingObstacle[]>();
	const bucketSize = ROUTING_BUCKET_REM * remPx;
	for (const obstacle of obstacles) {
		const firstX = Math.floor(obstacle.left / bucketSize);
		const lastX = Math.floor(obstacle.right / bucketSize);
		const firstY = Math.floor(obstacle.top / bucketSize);
		const lastY = Math.floor(obstacle.bottom / bucketSize);
		for (let bucketX = firstX; bucketX <= lastX; bucketX += 1) {
			for (let bucketY = firstY; bucketY <= lastY; bucketY += 1) {
				const key = `${bucketX}:${bucketY}`;
				const entries = buckets.get(key);
				if (entries) {
					entries.push(obstacle);
				} else {
					buckets.set(key, [obstacle]);
				}
			}
		}
	}
	return buckets;
};

const routingPointBlocked = (
	x: number,
	y: number,
	buckets: Map<string, RoutingObstacle[]>,
	remPx: number
): boolean => {
	const bucketSize = ROUTING_BUCKET_REM * remPx;
	const obstacles =
		buckets.get(`${Math.floor(x / bucketSize)}:${Math.floor(y / bucketSize)}`) ?? [];
	return obstacles.some(
		(obstacle) =>
			x >= obstacle.left && x <= obstacle.right && y >= obstacle.top && y <= obstacle.bottom
	);
};

const routingStateKey = (x: number, y: number, direction: number): string =>
	`${x}:${y}:${direction}`;

const heapPush = (heap: RoutingState[], state: RoutingState): void => {
	heap.push(state);
	let index = heap.length - 1;
	while (index > 0) {
		const parent = Math.floor((index - 1) * 0.5);
		if (heap[parent].estimate <= state.estimate) {
			break;
		}
		heap[index] = heap[parent];
		index = parent;
	}
	heap[index] = state;
};

const heapPop = (heap: RoutingState[]): RoutingState | null => {
	const first = heap[0];
	const last = heap.pop();
	if (!first || !last || heap.length === 0) {
		return first ?? null;
	}
	let index = 0;
	while (true) {
		const left = index * 2 + 1;
		const right = left + 1;
		if (left >= heap.length) {
			break;
		}
		const child = right < heap.length && heap[right].estimate < heap[left].estimate ? right : left;
		if (heap[child].estimate >= last.estimate) {
			break;
		}
		heap[index] = heap[child];
		index = child;
	}
	heap[index] = last;
	return first;
};

const simplifyRoute = (points: GraphNodePosition[]): GraphNodePosition[] => {
	const unique = points.filter(
		(point, index) =>
			index === 0 || point.x !== points[index - 1].x || point.y !== points[index - 1].y
	);
	if (unique.length < 3) {
		return unique;
	}
	const simplified = [unique[0]];
	for (let index = 1; index < unique.length - 1; index += 1) {
		const previous = simplified[simplified.length - 1];
		const current = unique[index];
		const next = unique[index + 1];
		if (
			(previous.x === current.x && current.x === next.x) ||
			(previous.y === current.y && current.y === next.y)
		) {
			continue;
		}
		simplified.push(current);
	}
	simplified.push(unique.at(-1) ?? unique[0]);
	return simplified;
};

const pointInRect = (
	x: number,
	y: number,
	left: number,
	top: number,
	right: number,
	bottom: number
) => x > left && x < right && y > top && y < bottom;

const lineIntersectsAnyObstacle = (
	x0: number,
	y0: number,
	x1: number,
	y1: number,
	obstacles: RoutingObstacle[]
): boolean => {
	const minX = Math.min(x0, x1);
	const maxX = Math.max(x0, x1);
	const minY = Math.min(y0, y1);
	const maxY = Math.max(y0, y1);
	for (const obstacle of obstacles) {
		if (
			maxX <= obstacle.left ||
			minX >= obstacle.right ||
			maxY <= obstacle.top ||
			minY >= obstacle.bottom
		) {
			continue;
		}
		if (
			pointInRect(x0, y0, obstacle.left, obstacle.top, obstacle.right, obstacle.bottom) ||
			pointInRect(x1, y1, obstacle.left, obstacle.top, obstacle.right, obstacle.bottom)
		) {
			return true;
		}
		const dx = x1 - x0;
		const dy = y1 - y0;
		if (dx === 0 || dy === 0) {
			if (
				dy === 0 &&
				y0 > obstacle.top &&
				y0 < obstacle.bottom &&
				minX < obstacle.right &&
				maxX > obstacle.left
			) {
				return true;
			}
			if (
				dx === 0 &&
				x0 > obstacle.left &&
				x0 < obstacle.right &&
				minY < obstacle.bottom &&
				maxY > obstacle.top
			) {
				return true;
			}
			continue;
		}
		const checkLine = (x3: number, y3: number, x4: number, y4: number) => {
			const den = (x0 - x1) * (y3 - y4) - (y0 - y1) * (x3 - x4);
			if (den === 0) {
				return false;
			}
			const t = ((x0 - x3) * (y3 - y4) - (y0 - y3) * (x3 - x4)) / den;
			const u = -((x0 - x1) * (y0 - y3) - (y0 - y1) * (x0 - x3)) / den;
			return t > 0 && t < 1 && u > 0 && u < 1;
		};
		if (
			checkLine(obstacle.left, obstacle.top, obstacle.right, obstacle.top) ||
			checkLine(obstacle.right, obstacle.top, obstacle.right, obstacle.bottom) ||
			checkLine(obstacle.right, obstacle.bottom, obstacle.left, obstacle.bottom) ||
			checkLine(obstacle.left, obstacle.bottom, obstacle.left, obstacle.top)
		) {
			return true;
		}
	}
	return false;
};

const smoothRoute = (
	points: GraphNodePosition[],
	obstacles: RoutingObstacle[]
): GraphNodePosition[] => {
	if (points.length <= 2) {
		return points;
	}
	const smoothed: GraphNodePosition[] = [points[0]];
	let currentIndex = 0;
	while (currentIndex < points.length - 1) {
		let furthestVisibleIndex = currentIndex + 1;
		for (let i = currentIndex + 2; i < points.length; i++) {
			if (
				!lineIntersectsAnyObstacle(
					points[currentIndex].x,
					points[currentIndex].y,
					points[i].x,
					points[i].y,
					obstacles
				)
			) {
				furthestVisibleIndex = i;
			}
		}
		smoothed.push(points[furthestVisibleIndex]);
		currentIndex = furthestVisibleIndex;
	}
	return smoothed;
};

const roundedPath = (points: GraphNodePosition[], radius: number): string => {
	if (points.length < 3) {
		return points.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(' ');
	}
	let path = `M ${points[0].x} ${points[0].y}`;
	for (let i = 1; i < points.length - 1; i++) {
		const prev = points[i - 1];
		const curr = points[i];
		const next = points[i + 1];
		const dPrev = Math.hypot(curr.x - prev.x, curr.y - prev.y);
		const dNext = Math.hypot(next.x - curr.x, next.y - curr.y);
		if (dPrev < 0.1 || dNext < 0.1) {
			path += ` L ${curr.x} ${curr.y}`;
			continue;
		}
		const r = Math.min(radius, dPrev / 2, dNext / 2);
		if (r <= 0.1) {
			path += ` L ${curr.x} ${curr.y}`;
			continue;
		}
		const startX = curr.x - (curr.x - prev.x) * (r / dPrev);
		const startY = curr.y - (curr.y - prev.y) * (r / dPrev);
		const endX = curr.x + (next.x - curr.x) * (r / dNext);
		const endY = curr.y + (next.y - curr.y) * (r / dNext);
		path += ` L ${startX} ${startY} Q ${curr.x} ${curr.y} ${endX} ${endY}`;
	}
	path += ` L ${points[points.length - 1].x} ${points[points.length - 1].y}`;
	return path;
};

const findOrthogonalRoute = (
	start: GraphNodePosition,
	end: GraphNodePosition,
	buckets: Map<string, RoutingObstacle[]>,
	remPx: number
): GraphNodePosition[] | null => {
	const grid = ROUTING_GRID_REM * remPx;
	const margin = ROUTING_MARGIN_REM * remPx;
	const sourceX = Math.ceil(start.x / grid);
	const sourceY = Math.round(start.y / grid);
	const targetX = Math.floor(end.x / grid);
	const targetY = Math.round(end.y / grid);
	const minX = Math.floor((Math.min(start.x, end.x) - margin) / grid);
	const maxX = Math.ceil((Math.max(start.x, end.x) + margin) / grid);
	const minY = Math.floor((Math.min(start.y, end.y) - margin) / grid);
	const maxY = Math.ceil((Math.max(start.y, end.y) + margin) / grid);
	const directions = [
		{ x: 1, y: 0 },
		{ x: 0, y: 1 },
		{ x: -1, y: 0 },
		{ x: 0, y: -1 }
	];
	const open: RoutingState[] = [];
	const costs = new Map<string, number>();
	const parents = new Map<string, string>();
	const states = new Map<string, RoutingState>();
	const startState: RoutingState = {
		x: sourceX,
		y: sourceY,
		direction: -1,
		cost: 0,
		estimate: Math.abs(targetX - sourceX) + Math.abs(targetY - sourceY),
		key: routingStateKey(sourceX, sourceY, -1)
	};
	costs.set(startState.key, 0);
	states.set(startState.key, startState);
	heapPush(open, startState);
	let visits = 0;
	let goal: RoutingState | null = null;
	while (open.length > 0 && visits < ROUTING_MAX_VISITS) {
		const current = heapPop(open);
		if (!current || current.cost !== costs.get(current.key)) {
			continue;
		}
		visits += 1;
		if (current.x === targetX && current.y === targetY) {
			goal = current;
			break;
		}
		for (let direction = 0; direction < directions.length; direction += 1) {
			const nextX = current.x + directions[direction].x;
			const nextY = current.y + directions[direction].y;
			if (nextX < minX || nextX > maxX || nextY < minY || nextY > maxY) {
				continue;
			}
			if (routingPointBlocked(nextX * grid, nextY * grid, buckets, remPx)) {
				continue;
			}
			const turnCost =
				current.direction >= 0 && current.direction !== direction ? ROUTING_TURN_COST : 0;
			const nextCost = current.cost + 1 + turnCost;
			const key = routingStateKey(nextX, nextY, direction);
			if (nextCost >= (costs.get(key) ?? Number.POSITIVE_INFINITY)) {
				continue;
			}
			const next: RoutingState = {
				x: nextX,
				y: nextY,
				direction,
				cost: nextCost,
				estimate: nextCost + Math.abs(targetX - nextX) + Math.abs(targetY - nextY),
				key
			};
			costs.set(key, nextCost);
			parents.set(key, current.key);
			states.set(key, next);
			heapPush(open, next);
		}
	}
	if (!goal) {
		return null;
	}
	const points: GraphNodePosition[] = [];
	let key: string | undefined = goal.key;
	while (key) {
		const state = states.get(key);
		if (!state) {
			break;
		}
		points.push({ x: state.x * grid, y: state.y * grid });
		key = parents.get(key);
	}
	points.reverse();
	return simplifyRoute(points);
};

export const routeEdgeAroundObstacles = (
	start: GraphNodePosition,
	end: GraphNodePosition,
	routeStart: GraphNodePosition,
	routeEnd: GraphNodePosition,
	obstacles: RoutingObstacle[],
	buckets: Map<string, RoutingObstacle[]>,
	remPx: number
): string | null => {
	const grid = ROUTING_GRID_REM * remPx;
	const gridStart = {
		x: routeStart.x,
		y: Math.round(routeStart.y / grid) * grid
	};
	const gridEnd = { x: routeEnd.x, y: Math.round(routeEnd.y / grid) * grid };
	const middle = findOrthogonalRoute(gridStart, gridEnd, buckets, remPx);
	if (!middle) {
		return null;
	}
	let points = simplifyRoute([routeStart, gridStart, ...middle, gridEnd, routeEnd]);
	points = smoothRoute(points, obstacles);
	points = simplifyRoute([start, ...points, end]);
	return roundedPath(points, remPx * 1.5);
};
