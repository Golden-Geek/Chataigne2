import type { GraphNodePosition } from "./types";
import { roundedWireGeometry, type GraphWireGeometry } from "./wire-geometry";

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
export const ROUTING_MARGIN_REM = 16;
export const ROUTING_CLEARANCE_REM = 0.7;
export const MAX_ROUTING_OBSTACLES = 128;
export const MAX_ROUTING_VISITS = 2048;
const ROUTING_TURN_COST = 0.35;

export interface RoutingBudget {
  remainingVisits: number;
  remainingChecks: number;
}

export const routingSearchWithinBudget = (
  start: GraphNodePosition,
  end: GraphNodePosition,
  remPx: number,
  maximumVisits: number,
): boolean => {
  const grid = ROUTING_GRID_REM * remPx;
  const coordinates = [
    Math.ceil(start.x / grid),
    Math.round(start.y / grid),
    Math.floor(end.x / grid),
    Math.round(end.y / grid),
  ];
  return (
    Number.isFinite(remPx) &&
    remPx > 0 &&
    coordinates.every(
      (value) =>
        Number.isSafeInteger(value) &&
        Number.isSafeInteger(value - ROUTING_MARGIN_REM) &&
        Number.isSafeInteger(value + ROUTING_MARGIN_REM),
    ) &&
    Math.abs(coordinates[0] - coordinates[2]) +
      Math.abs(coordinates[1] - coordinates[3]) +
      1 <=
      maximumVisits
  );
};

const routingStepBlocked = (
  x: number,
  y: number,
  nextX: number,
  nextY: number,
  obstacles: readonly RoutingObstacle[],
): boolean =>
  obstacles.some((obstacle) =>
    x === nextX
      ? x >= obstacle.left &&
        x <= obstacle.right &&
        Math.max(y, nextY) >= obstacle.top &&
        Math.min(y, nextY) <= obstacle.bottom
      : y >= obstacle.top &&
        y <= obstacle.bottom &&
        Math.max(x, nextX) >= obstacle.left &&
        Math.min(x, nextX) <= obstacle.right,
  );

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
    const child =
      right < heap.length && heap[right].estimate < heap[left].estimate
        ? right
        : left;
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
      index === 0 ||
      point.x !== points[index - 1].x ||
      point.y !== points[index - 1].y,
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
  bottom: number,
) => x > left && x < right && y > top && y < bottom;

const lineIntersectsAnyObstacle = (
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  obstacles: readonly RoutingObstacle[],
  budget: RoutingBudget,
): boolean => {
  const minX = Math.min(x0, x1);
  const maxX = Math.max(x0, x1);
  const minY = Math.min(y0, y1);
  const maxY = Math.max(y0, y1);
  for (const obstacle of obstacles) {
    if (budget.remainingChecks <= 0) return true;
    budget.remainingChecks -= 1;
    if (
      maxX <= obstacle.left ||
      minX >= obstacle.right ||
      maxY <= obstacle.top ||
      minY >= obstacle.bottom
    ) {
      continue;
    }
    if (
      pointInRect(
        x0,
        y0,
        obstacle.left,
        obstacle.top,
        obstacle.right,
        obstacle.bottom,
      ) ||
      pointInRect(
        x1,
        y1,
        obstacle.left,
        obstacle.top,
        obstacle.right,
        obstacle.bottom,
      )
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
      checkLine(
        obstacle.right,
        obstacle.top,
        obstacle.right,
        obstacle.bottom,
      ) ||
      checkLine(
        obstacle.right,
        obstacle.bottom,
        obstacle.left,
        obstacle.bottom,
      ) ||
      checkLine(obstacle.left, obstacle.bottom, obstacle.left, obstacle.top)
    ) {
      return true;
    }
  }
  return false;
};

const smoothRoute = (
  points: GraphNodePosition[],
  obstacles: readonly RoutingObstacle[],
  budget: RoutingBudget,
): GraphNodePosition[] => {
  if (points.length <= 2) {
    return points;
  }
  const smoothed: GraphNodePosition[] = [points[0]];
  let currentIndex = 0;
  while (currentIndex < points.length - 1) {
    if (budget.remainingChecks <= 0) {
      smoothed.push(...points.slice(currentIndex + 1));
      return smoothed;
    }
    let furthestVisibleIndex = currentIndex + 1;
    for (let i = currentIndex + 2; i < points.length; i++) {
      if (budget.remainingChecks <= 0) break;
      if (
        !lineIntersectsAnyObstacle(
          points[currentIndex].x,
          points[currentIndex].y,
          points[i].x,
          points[i].y,
          obstacles,
          budget,
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

const findOrthogonalRoute = (
  start: GraphNodePosition,
  end: GraphNodePosition,
  obstacles: readonly RoutingObstacle[],
  remPx: number,
  budget: RoutingBudget,
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
  if (
    ![sourceX, sourceY, targetX, targetY, minX, maxX, minY, maxY].every(
      Number.isSafeInteger,
    ) ||
    Math.abs(targetX - sourceX) + Math.abs(targetY - sourceY) + 1 >
      budget.remainingVisits
  )
    return null;
  if (routingStepBlocked(start.x, start.y, start.x, start.y, obstacles))
    return null;
  const directions = [
    { x: 1, y: 0 },
    { x: 0, y: 1 },
    { x: -1, y: 0 },
    { x: 0, y: -1 },
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
    key: routingStateKey(sourceX, sourceY, -1),
  };
  costs.set(startState.key, 0);
  states.set(startState.key, startState);
  heapPush(open, startState);
  let goal: RoutingState | null = null;
  while (open.length > 0 && budget.remainingVisits > 0) {
    const current = heapPop(open);
    if (!current || current.cost !== costs.get(current.key)) {
      continue;
    }
    budget.remainingVisits -= 1;
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
      if (
        routingStepBlocked(
          current.x * grid,
          current.y * grid,
          nextX * grid,
          nextY * grid,
          obstacles,
        )
      ) {
        continue;
      }
      const turnCost =
        current.direction >= 0 && current.direction !== direction
          ? ROUTING_TURN_COST
          : 0;
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
        estimate:
          nextCost + Math.abs(targetX - nextX) + Math.abs(targetY - nextY),
        key,
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
  obstacles: readonly RoutingObstacle[],
  remPx: number,
  budget: RoutingBudget = {
    remainingVisits: MAX_ROUTING_VISITS,
    remainingChecks: 512,
  },
): GraphWireGeometry | null => {
  if (
    !Number.isFinite(remPx) ||
    remPx <= 0 ||
    obstacles.length > MAX_ROUTING_OBSTACLES ||
    ![
      start.x,
      start.y,
      end.x,
      end.y,
      routeStart.x,
      routeStart.y,
      routeEnd.x,
      routeEnd.y,
    ].every(Number.isFinite)
  )
    return null;
  if (
    !routingSearchWithinBudget(
      routeStart,
      routeEnd,
      remPx,
      budget.remainingVisits,
    )
  )
    return null;
  if (
    !lineIntersectsAnyObstacle(
      routeStart.x,
      routeStart.y,
      routeEnd.x,
      routeEnd.y,
      obstacles,
      budget,
    )
  ) {
    return roundedWireGeometry(
      simplifyRoute([start, routeStart, routeEnd, end]),
      remPx * 1.5,
      obstacles,
    );
  }
  const grid = ROUTING_GRID_REM * remPx;
  const gridStart = {
    x: routeStart.x,
    y: Math.round(routeStart.y / grid) * grid,
  };
  const gridEnd = { x: routeEnd.x, y: Math.round(routeEnd.y / grid) * grid };
  const middle = findOrthogonalRoute(
    gridStart,
    gridEnd,
    obstacles,
    remPx,
    budget,
  );
  if (!middle || middle.length > 256) {
    return null;
  }
  let points = simplifyRoute([
    routeStart,
    gridStart,
    ...middle,
    gridEnd,
    routeEnd,
  ]);
  points = smoothRoute(points, obstacles, budget);
  points = simplifyRoute([start, ...points, end]);
  if (points.length > 256) return null;
  return roundedWireGeometry(points, remPx * 1.5, obstacles);
};
