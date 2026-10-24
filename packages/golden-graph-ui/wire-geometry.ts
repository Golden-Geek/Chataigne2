import type { SpatialBounds } from "./spatial-index";
import type { GraphNodePosition } from "./types";

export interface GraphWireGeometry {
  path: string;
  bounds: SpatialBounds;
}

export const pointsBounds = (
  points: readonly GraphNodePosition[],
): SpatialBounds => {
  let left = Infinity,
    top = Infinity,
    right = -Infinity,
    bottom = -Infinity;
  for (const point of points) {
    left = Math.min(left, point.x);
    right = Math.max(right, point.x);
    top = Math.min(top, point.y);
    bottom = Math.max(bottom, point.y);
  }
  return { left, top, right, bottom };
};

export const expandedBounds = (
  bounds: SpatialBounds,
  margin: number,
): SpatialBounds => ({
  left: bounds.left - margin,
  top: bounds.top - margin,
  right: bounds.right + margin,
  bottom: bounds.bottom + margin,
});
export const unionBounds = (
  a: SpatialBounds,
  b: SpatialBounds,
): SpatialBounds => ({
  left: Math.min(a.left, b.left),
  top: Math.min(a.top, b.top),
  right: Math.max(a.right, b.right),
  bottom: Math.max(a.bottom, b.bottom),
});
export const boundsIntersect = (a: SpatialBounds, b: SpatialBounds): boolean =>
  a.left <= b.right &&
  a.right >= b.left &&
  a.top <= b.bottom &&
  a.bottom >= b.top;

export const cubicWireGeometry = (
  start: GraphNodePosition,
  end: GraphNodePosition,
): GraphWireGeometry => {
  const control = Math.min(12, Math.max(2.5, Math.abs(end.x - start.x) * 0.5));
  const first = { x: start.x + control, y: start.y },
    second = { x: end.x - control, y: end.y };
  return {
    path: `M ${start.x} ${start.y} C ${first.x} ${first.y} ${second.x} ${second.y} ${end.x} ${end.y}`,
    bounds: pointsBounds([start, first, second, end]),
  };
};

export const roundedWireGeometry = (
  points: readonly GraphNodePosition[],
  radius: number,
  obstacles: readonly SpatialBounds[] = [],
): GraphWireGeometry => {
  let path = `M ${points[0].x} ${points[0].y}`;
  for (let index = 1; index < points.length - 1; index += 1) {
    const previous = points[index - 1],
      current = points[index],
      next = points[index + 1];
    const before = Math.hypot(current.x - previous.x, current.y - previous.y);
    const after = Math.hypot(next.x - current.x, next.y - current.y);
    let r = Math.min(radius, before / 2, after / 2);
    if (r > 0.1) {
      const corner = pointsBounds([
        current,
        {
          x: current.x - ((current.x - previous.x) * r) / before,
          y: current.y - ((current.y - previous.y) * r) / before,
        },
        {
          x: current.x + ((next.x - current.x) * r) / after,
          y: current.y + ((next.y - current.y) * r) / after,
        },
      ]);
      if (obstacles.some((obstacle) => boundsIntersect(corner, obstacle)))
        r = 0;
    }
    if (r <= 0.1) {
      path += ` L ${current.x} ${current.y}`;
      continue;
    }
    path += ` L ${current.x - ((current.x - previous.x) * r) / before} ${current.y - ((current.y - previous.y) * r) / before} Q ${current.x} ${current.y} ${current.x + ((next.x - current.x) * r) / after} ${current.y + ((next.y - current.y) * r) / after}`;
  }
  const end = points.at(-1)!;
  if (points.length > 1) path += ` L ${end.x} ${end.y}`;
  return { path, bounds: pointsBounds(points) };
};

/** Loops keep a visible exterior path even if a bounded obstacle search cannot succeed. */
export const loopWireGeometry = (
  start: GraphNodePosition,
  end: GraphNodePosition,
  node: SpatialBounds,
): GraphWireGeometry =>
  roundedWireGeometry(
    [
      start,
      { x: node.right + 2, y: start.y },
      { x: node.right + 2, y: node.top - 2 },
      { x: node.left - 2, y: node.top - 2 },
      { x: node.left - 2, y: end.y },
      end,
    ],
    1.5,
  );

export const scaledWirePath = (
  geometry: GraphWireGeometry,
  scale: number,
): string | null => {
  const tokens = geometry.path.split(" ");
  for (let index = 0; index < tokens.length; index += 1) {
    if (/^[MLQC]$/.test(tokens[index])) continue;
    const value = Number(tokens[index]) * scale;
    if (!Number.isFinite(value)) return null;
    tokens[index] = String(value);
  }
  return tokens.join(" ");
};
