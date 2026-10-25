import { tick } from "svelte";
import type { GraphEdge, GraphNode } from "../../../types";
import type { QualificationContext } from "./qualification";

const frame = () => new Promise<number>(requestAnimationFrame);
const rounded = (value: number) => Math.round(value * 1000) / 1000;
const percentile = (values: number[], q: number) =>
  [...values].sort((a, b) => a - b)[Math.ceil(values.length * q) - 1] ?? 0;

function scene(varied: boolean) {
  const node = (id: string, x: number, y: number): GraphNode => ({
    id,
    label: id,
    position: { x, y },
    size: { width: 10, height: 4.5 },
    inputs: [
      { id: "a", label: "A" },
      { id: "b", label: "B" },
    ],
    outputs: [
      { id: "a", label: "A" },
      { id: "b", label: "B" },
    ],
  });
  const rows = varied ? 12 : 1;
  const nodes = Array.from({ length: rows }, (_, row) => [
    node(`left-${row}`, 2, row * 5),
    node(`right-${row}`, 80, row * 5),
  ]).flat();
  if (varied)
    for (let index = 0; index < 4; index += 1)
      nodes.push(node(`obstacle-${index}`, 40, index * 14));
  const edges: GraphEdge[] = Array.from({ length: 1100 }, (_, index) => ({
    id: `dense-${index}`,
    from: {
      nodeId: `left-${index % rows}`,
      socketId: varied && Math.floor(index / 144) % 2 ? "b" : "a",
    },
    to: {
      nodeId: `right-${Math.floor(index / rows) % rows}`,
      socketId: varied && Math.floor(index / 288) % 2 ? "b" : "a",
    },
  }));
  return { nodes, edges };
}

export async function qualifyDense(
  context: QualificationContext,
  varied: boolean,
  counts = { warmups: 3, samples: 31 },
) {
  const data = scene(varied);
  context.size(100, 60);
  context.select([]);
  context.canvas().setCamera({ x: 0, y: 0, zoom: 1 });
  const warmups = [],
    samples = [];
  for (let index = 0; index < counts.warmups + counts.samples; index += 1) {
    context.publish([], []);
    await tick();
    while (
      context.canvas().mountingWork().nodes ||
      context.canvas().mountingWork().edges
    ) {
      await frame();
      await tick();
    }
    let previous = await frame();
    const intervalsMs: number[] = [];
    let handle = 0,
      active = true;
    const observe = (now: number) => {
      intervalsMs.push(rounded(now - previous));
      previous = now;
      if (active) handle = requestAnimationFrame(observe);
    };
    handle = requestAnimationFrame(observe);
    const started = performance.now();
    const beforeWork = context.canvas().routingWork();
    try {
      context.publish(data.nodes, data.edges);
      await tick();
      const root = document.querySelector(".graph-canvas")!;
      let paths = root.querySelectorAll<SVGGeometryElement>(".wires path.edge");
      while (!paths.length && context.canvas().mountingWork().edges) {
        await frame();
        await tick();
        paths = root.querySelectorAll<SVGGeometryElement>(".wires path.edge");
      }
      if (!paths.length || !paths[0].getTotalLength())
        throw new Error("Dense wire fallback did not begin rendering.");
      const feedbackMs = rounded(performance.now() - started);
      while (
        context.canvas().routingWork().pendingRoutes ||
        context.canvas().mountingWork().nodes ||
        context.canvas().mountingWork().edges
      ) {
        if (performance.now() - started > 10000)
          throw new Error(
            `Dense routing did not settle within 10 seconds: ${JSON.stringify(context.canvas().routingWork())}`,
          );
        await frame();
        await tick();
      }
      const completeMs = rounded(performance.now() - started);
      await frame();
      if (
        context.graph().edges.length !== 1100 ||
        root.querySelectorAll(".wires path.edge").length !== 1100
      )
        throw new Error("Dense routing lost authored paths.");
      const work = context.canvas().routingWork();
      if (work.cachedRoutes !== 1024)
        throw new Error(
          "Dense route cache admission differs from its fixed cap.",
        );
      const value = {
        feedbackMs,
        completeMs,
        intervalsMs,
        mountedNodes: root.querySelectorAll("article.node").length,
        mountedEdges: root.querySelectorAll(".wires path.edge").length,
        paintGroups: root.querySelectorAll("svg.wires").length - 1,
        cachedRoutes: work.cachedRoutes,
        routing: {
          jobs: work.routeJobs - beforeWork.routeJobs,
          visits: work.searchVisits - beforeWork.searchVisits,
          checks: work.smoothingChecks - beforeWork.smoothingChecks,
          viewBudgetFallbacks:
            work.viewBudgetFallbacks - beforeWork.viewBudgetFallbacks,
        },
      };
      (index < counts.warmups ? warmups : samples).push(value);
    } finally {
      active = false;
      cancelAnimationFrame(handle);
    }
  }
  const intervals = samples.flatMap((sample) => sample.intervalsMs);
  const summary = {
    feedbackP95: percentile(
      samples.map((sample) => sample.feedbackMs),
      0.95,
    ),
    completionP95: percentile(
      samples.map((sample) => sample.completeMs),
      0.95,
    ),
    completionMax: Math.max(...samples.map((sample) => sample.completeMs)),
    intervalP95: percentile(intervals, 0.95),
    intervalMax: Math.max(...intervals),
  };
  return {
    scene: varied ? "varied" : "parallel",
    nodes: data.nodes.length,
    edges: 1100,
    warmups,
    samples,
    summary,
    limits: {
      feedbackP95: 200,
      completionMax: 2000,
      intervalP95: 33.4,
      intervalMax: 200,
    },
    passed:
      summary.completionMax <= 2000 &&
      summary.feedbackP95 <= 200 &&
      summary.intervalP95 <= 33.4 &&
      summary.intervalMax <= 200,
  };
}
