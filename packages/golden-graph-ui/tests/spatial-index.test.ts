import { describe, expect, it } from "vitest";
import { SpatialIndex, type SpatialBounds } from "../spatial-index";

const bounds = (x: number, y = 0): SpatialBounds => ({
  left: x,
  right: x + 2,
  top: y,
  bottom: y + 2,
});

describe("retained graph spatial index", () => {
  it("retains a coordinate snapshot without interpreting unrelated metadata", () => {
    const index = new SpatialIndex<string>();
    const rectangle = { ...bounds(0), label: "presentation metadata" };
    index.insert("node", rectangle, "original");
    rectangle.left = 100;
    rectangle.right = 102;
    expect(index.query(bounds(0))).toEqual(["original"]);
    expect(index.query(bounds(100))).toEqual([]);
  });

  it("updates cells while preserving document order", () => {
    const index = new SpatialIndex<string>(4);
    index.insert("first", bounds(0), "first");
    index.insert("second", bounds(100), "second");
    index.update("first", bounds(100), "moved");
    expect(index.query(bounds(0))).toEqual([]);
    expect(index.query(bounds(100))).toEqual(["moved", "second"]);
    expect(index.size).toBe(2);
    expect(index.remove("first")).toBe(true);
    expect(index.remove("first")).toBe(false);
    index.insert("first", bounds(100), "reinserted");
    expect(index.query(bounds(100))).toEqual(["second", "reinserted"]);
  });

  it("replaces payload without changing geometry or neighboring values", () => {
    const index = new SpatialIndex<object>();
    const untouched = { label: "untouched" };
    const updated = { label: "new" };
    index.insert("edited", bounds(0), {});
    index.insert("neighbor", bounds(0), untouched);
    index.setValue("edited", updated);
    expect(index.get("edited")).toBe(updated);
    expect(index.query(bounds(0))).toEqual([updated, untouched]);
    expect(index.get("neighbor")).toBe(untouched);
  });

  it("filters coarse neighbors but retains touching and reversed bounds", () => {
    const index = new SpatialIndex<string>(32);
    index.insert("outside", bounds(20), "outside");
    index.insert("touching", bounds(2), "touching");
    index.insert(
      "reversed",
      { left: 2, right: 0, top: 2, bottom: 0 },
      "reversed",
    );
    expect(index.query(bounds(0))).toEqual(["touching", "reversed"]);
  });

  it("bounds huge geometry and queries without expanding their grids", () => {
    const index = new SpatialIndex<string>();
    const enormous = { left: -1e300, top: -1e300, right: 1e300, bottom: 1e300 };
    index.insert("world", enormous, "world");
    index.insert("local", bounds(0), "local");
    expect(index.query(bounds(0))).toEqual(["world", "local"]);
    expect(index.query(enormous)).toEqual(["world", "local"]);
    index.update("world", bounds(1000), "localized");
    expect(index.query(bounds(0))).toEqual(["local"]);
    index.update("local", enormous, "expanded");
    expect(index.query(bounds(1000))).toEqual(["localized", "expanded"]);
    index.remove("local");
    expect(index.query(bounds(0))).toEqual([]);
  });

  it("rejects invalid changes before altering membership", () => {
    const index = new SpatialIndex<string>();
    index.insert("node", bounds(0), "original");
    expect(() =>
      index.update("node", { ...bounds(0), right: Infinity }, "invalid"),
    ).toThrow(RangeError);
    expect(index.query(bounds(0))).toEqual(["original"]);
    expect(() => index.insert("node", bounds(20), "duplicate")).toThrow();
    expect(() => index.update("missing", bounds(0), "missing")).toThrow();
    expect(() => index.setValue("missing", "missing")).toThrow();
    expect(() => index.query({ ...bounds(0), left: NaN })).toThrow(RangeError);
    expect(() => new SpatialIndex(0)).toThrow(RangeError);
  });
});
