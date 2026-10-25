import { describe, expect, it } from "vitest";
import { RetainedMounts } from "../retained-mounts";

describe("bounded canvas membership", () => {
  it("bounds removals/additions while retaining final authored order", () => {
    const mounts = new RetainedMounts<string>((value) => value);
    mounts.setTarget(["a", "b", "c"]);
    mounts.step(2);
    expect(mounts.values).toEqual(["a", "b"]);
    expect(mounts.pending).toBe(1);
    mounts.setTarget(["d", "b", "e"]);
    mounts.step(1);
    expect(mounts.values).toEqual(["b"]);
    expect(mounts.pending).toBe(2);
    mounts.step(2);
    expect(mounts.values).toEqual(["d", "b", "e"]);
    expect(mounts.pending).toBe(0);
  });
  it("updates existing payloads immediately and discards superseded targets", () => {
    const mounts = new RetainedMounts<{ id: string; value: number }>(
      (value) => value.id,
    );
    const a = { id: "a", value: 1 };
    mounts.setTarget([a, { id: "discarded", value: 0 }]);
    mounts.step(1);
    const changed = { id: "a", value: 2 };
    mounts.setTarget([changed]);
    mounts.step(0);
    expect(mounts.values).toEqual([changed]);
    expect(mounts.pending).toBe(0);
    const previous = mounts.values;
    mounts.setTarget([changed]);
    mounts.step(10);
    expect(mounts.values).toBe(previous);
    expect(() => mounts.step(-1)).toThrow(RangeError);
    expect(() => mounts.setTarget([a, a])).toThrow(/Duplicate/);
  });
});
