import { describe, expect, it } from "vitest";
import {
  cubicWireGeometry,
  loopWireGeometry,
  roundedWireGeometry,
  scaledWirePath,
} from "../wire-geometry";

describe("wire path bounds and serialization", () => {
  it("keeps control hulls and loops inside their conservative bounds at any finite scale", () => {
    const wire = cubicWireGeometry({ x: 30, y: 0 }, { x: -30, y: 10 });
    expect(wire.bounds).toEqual({ left: -42, top: 0, right: 42, bottom: 10 });
    expect(scaledWirePath(wire, 16)).toContain("M 480 0");
    const loop = loopWireGeometry(
      { x: 2, y: 1 },
      { x: 0, y: 1 },
      { left: 0, top: 0, right: 2, bottom: 2 },
    );
    expect(loop.bounds).toEqual({ left: -2, top: -2, right: 4, bottom: 1 });
    expect(loop.path).toContain("Q");
    expect(
      scaledWirePath(
        cubicWireGeometry({ x: 1e300, y: 0 }, { x: 0, y: 0 }),
        1e300,
      ),
    ).toBeNull();
  });

  it("keeps corners sharp when rounding would cut through an obstacle", () => {
    const points = [
      { x: 0, y: 0 },
      { x: 2, y: 0 },
      { x: 2, y: 2 },
    ];
    expect(roundedWireGeometry(points, 1).path).toContain("Q");
    expect(
      roundedWireGeometry(points, 1, [
        { left: 1.4, top: 0.1, right: 2.1, bottom: 0.6 },
      ]).path,
    ).not.toContain("Q");
  });
});
