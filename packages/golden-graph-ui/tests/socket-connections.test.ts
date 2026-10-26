import { describe, expect, it } from "vitest";
import { GraphSocketConnections } from "../socket-connections";

describe("socket endpoint occupancy", () => {
  it("distinguishes the same input/output ID on the same node", () => {
    const connections = new GraphSocketConnections([
      {
        from: { nodeId: "source", socketId: "value" },
        to: { nodeId: "target", socketId: "value" },
      },
    ]);
    expect(connections.has("source", "value", "output")).toBe(true);
    expect(connections.has("source", "value", "input")).toBe(false);
    expect(connections.has("target", "value", "input")).toBe(true);
    expect(connections.has("target", "value", "output")).toBe(false);
  });

  it("does not alias node/socket IDs containing separators or empty strings", () => {
    const connections = new GraphSocketConnections([
      {
        from: { nodeId: "a:b", socketId: "c" },
        to: { nodeId: "", socketId: "x:y" },
      },
    ]);
    expect(connections.has("a:b", "c", "output")).toBe(true);
    expect(connections.has("a", "b:c", "output")).toBe(false);
    expect(connections.has("", "x:y", "input")).toBe(true);
    expect(connections.has("x", ":y", "input")).toBe(false);
  });
});
