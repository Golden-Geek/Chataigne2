import { describe, expect, it } from "vitest";
import {
  routeEdgeAroundObstacles,
  type RoutingObstacle,
} from "../edge-routing";

const route = (obstacles: RoutingObstacle[]) =>
  routeEdgeAroundObstacles(
    { x: 0, y: 0 },
    { x: 12, y: 0 },
    { x: 2, y: 0 },
    { x: 10, y: 0 },
    obstacles,
    1,
  );

describe("generic graph edge routing", () => {
  it("keeps a clear edge straight", () => {
    expect(route([])?.path).toBe("M 0 0 L 12 0");
  });

  it("takes a deterministic detour around node bounds", () => {
    const obstacles = [{ left: 5, top: -1, right: 7, bottom: 1 }];
    const path = route(obstacles);
    expect(path).toBeTruthy();
    expect(path).not.toEqual(route([]));
    expect(path && (path.bounds.top < -1 || path.bounds.bottom > 1)).toBe(true);
    expect(route(obstacles)).toEqual(path);
  });

  it("does not cross thin obstacles between adjacent grid points", () => {
    const path = route([{ left: 5.1, right: 5.2, top: -1, bottom: 1 }]);
    expect(path && (path.bounds.top < -1 || path.bounds.bottom > 1)).toBe(true);
  });

  it("bounds oversized coordinates, obstacle counts and search/smoothing work", () => {
    const budget = { remainingVisits: 1, remainingChecks: 0 };
    expect(
      routeEdgeAroundObstacles(
        { x: 0, y: 0 },
        { x: 100, y: 0 },
        { x: 2, y: 0 },
        { x: 98, y: 0 },
        [],
        1,
        budget,
      ),
    ).toBeNull();
    expect(budget.remainingVisits).toBe(1);
    expect(
      routeEdgeAroundObstacles(
        { x: 1e300, y: 0 },
        { x: 0, y: 0 },
        { x: 1e300, y: 0 },
        { x: 2, y: 0 },
        [],
        1,
      ),
    ).toBeNull();
    expect(
      route(
        Array.from({ length: 129 }, () => ({
          left: 5,
          top: -1,
          right: 7,
          bottom: 1,
        })),
      ),
    ).toBeNull();
    const bounded = { remainingVisits: 2048, remainingChecks: 0 };
    expect(
      routeEdgeAroundObstacles(
        { x: 0, y: 0 },
        { x: 12, y: 0 },
        { x: 2, y: 0 },
        { x: 10, y: 0 },
        [{ left: 5, top: -1, right: 7, bottom: 1 }],
        1,
        bounded,
      ),
    ).toBeTruthy();
    expect(bounded.remainingVisits).toBeGreaterThanOrEqual(0);
    expect(bounded.remainingChecks).toBe(0);
  });

  it("returns no route when the search boundary is obstructed", () => {
    expect(route([{ left: 3, top: -20, right: 9, bottom: 20 }])).toBeNull();
  });
});
