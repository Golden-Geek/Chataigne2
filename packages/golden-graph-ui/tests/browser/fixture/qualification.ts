import { tick } from "svelte";
import type GraphCanvas from "../../../components/GraphCanvas.svelte";
import type {
  GraphCamera,
  GraphEdge,
  GraphNode,
  GraphPresentationDocument,
} from "../../../types";
import { qualificationScene } from "./qualification-scenes";

type Canvas = ReturnType<typeof GraphCanvas>;
export interface QualificationContext {
  canvas(): Canvas;
  graph(): GraphPresentationDocument;
  publish(nodes: readonly GraphNode[], edges: readonly GraphEdge[]): void;
  select(ids: string[]): void;
  size(width: number, height: number): void;
}
const frame = () => new Promise<number>(requestAnimationFrame);
function ensure(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}
const rounded = (value: number) => Math.round(value * 1000) / 1000;
const percentile = (values: number[], q: number) =>
  [...values].sort((a, b) => a - b)[Math.ceil(values.length * q) - 1] ?? 0;
const cameraEqual = (a: GraphCamera, b: GraphCamera) =>
  Math.abs(a.x - b.x) < 0.001 &&
  Math.abs(a.y - b.y) < 0.001 &&
  Math.abs(a.zoom - b.zoom) < 0.000001;

export const qualificationActions = [
  "selection",
  "drag-single",
  "drag-multi",
  "pan",
  "zoom",
  "frame-selection",
  "home",
  "activity",
  "payload",
  "loop",
  "obstacle",
] as const;
type Action = (typeof qualificationActions)[number];

export function createQualification(context: QualificationContext) {
  let count = 0;
  const canvas = context.canvas;
  const graph = context.graph;
  const root = () => document.querySelector<HTMLElement>(".graph-canvas")!;
  const element = (id: string) =>
    document.querySelector<HTMLElement>(`article.node[data-node-id="${id}"]`);
  const wire = (id: string) =>
    document.querySelector<SVGGeometryElement>(
      `.wires path.edge[data-edge-id="${id}"]`,
    );
  const updateNode = (id: string, change: (node: GraphNode) => GraphNode) =>
    context.publish(
      graph().nodes.map((node) => (node.id === id ? change(node) : node)),
      graph().edges,
    );
  const counts = () => ({
    mountedNodes: root().querySelectorAll("article.node").length,
    mountedEdges: root().querySelectorAll(".wires path.edge").length,
    paintGroups: root().querySelectorAll("svg.wires").length - 1,
    overviewNodes: root().querySelectorAll(
      'article[data-presentation="overview"]',
    ).length,
  });

  const settled = async () => {
    const started = performance.now();
    for (;;) {
      await tick();
      if (
        !canvas().navigationState().animating &&
        canvas().routingWork().pendingRoutes === 0 &&
        canvas().mountingWork().nodes === 0 &&
        canvas().mountingWork().edges === 0
      )
        return;
      if (performance.now() - started > 10000)
        throw new Error("Operation did not settle within 10 seconds.");
      await frame();
    }
  };

  const assertNode = (id: string) => {
    const model = graph().nodes.find((node) => node.id === id);
    const rendered = element(id);
    ensure(model && rendered, `Expected rendered node ${id}.`);
    ensure(
      rendered.style.left === `${model.position.x}rem` &&
        rendered.style.top === `${model.position.y}rem`,
      `Source/rendered geometry differs for ${id}.`,
    );
    ensure(
      rendered.getBoundingClientRect().width > 0,
      `Empty node geometry for ${id}.`,
    );
  };

  async function prepare(action: Action) {
    canvas().clearNodePreviews();
    context.select(
      action === "drag-multi" ? ["n0", "n1", "n64", "n65"] : ["n0"],
    );
    const corridor = action === "loop" || action === "obstacle";
    const zoom = corridor ? 1 : count === 100000 ? 0.5 : 1;
    canvas().setCamera({ x: 0, y: corridor ? 960 : 0, zoom });
    await settled();
  }

  async function setup(nodes: number) {
    count = nodes;
    const started = performance.now();
    context.size(100, 60);
    const scene = qualificationScene(count);
    context.publish(scene.nodes, scene.edges);
    await frame();
    await tick();
    await prepare("selection");
    ensure(graph().nodes.length === count, "Authored fixture count differs.");
    return {
      nodes: count,
      edges: graph().edges.length,
      setupMs: rounded(performance.now() - started),
      ...counts(),
    };
  }

  async function sample(action: Action, index: number) {
    if (action === "home") {
      context.select(["n0"]);
      canvas().frameSelection();
      await settled();
    }
    const firstFrame = await frame();
    const intervals: number[] = [];
    let previous = firstFrame,
      active = true,
      handle = 0;
    const observe = (now: number) => {
      intervals.push(rounded(now - previous));
      previous = now;
      if (active) handle = requestAnimationFrame(observe);
    };
    handle = requestAnimationFrame(observe);
    const beforeCamera = canvas().navigationState().camera;
    const beforeWork = canvas().presentationWork();
    const started = performance.now();
    try {
      let afterFeedback: (() => void) | null = null;
      let expected = () => {};
      let observeCamera = false;
      let framedId = "";
      if (action === "selection") {
        const selected = index % 2 === 0;
        context.select(selected ? ["n0", "n1"] : []);
        expected = () =>
          ensure(
            element("n0")?.classList.contains("selected") === selected,
            "Selection did not render.",
          );
      } else if (action === "drag-single" || action === "drag-multi") {
        const ids =
          action === "drag-single" ? ["n0"] : ["n0", "n1", "n64", "n65"];
        const delta = index % 2 === 0 ? 1 : -1;
        const moves = ids.map((id) => {
          const node = graph().nodes.find((node) => node.id === id)!;
          return {
            nodeId: id,
            position: { x: node.position.x + delta, y: node.position.y },
          };
        });
        canvas().previewNodeMoves(moves);
        expected = () => {
          for (const move of moves)
            ensure(
              element(move.nodeId)?.style.left === `${move.position.x}rem`,
              "Move preview did not render.",
            );
        };
        afterFeedback = () => canvas().commitNodeMovePreview();
      } else if (action === "pan") {
        const delta = index % 2 === 0 ? 16 : -16;
        const beforeRect = element("n0")!.getBoundingClientRect();
        canvas().panBy(delta, 0);
        expected = () =>
          ensure(
            Math.abs(
              element("n0")!.getBoundingClientRect().x - beforeRect.x - delta,
            ) < 1,
            "Pan delta did not render.",
          );
      } else if (action === "zoom") {
        const bounds = root().getBoundingClientRect();
        const anchor = { x: bounds.width * 0.4, y: bounds.height * 0.4 };
        const before = canvas().clientToWorld(
          bounds.left + anchor.x,
          bounds.top + anchor.y,
        );
        const base = count === 100000 ? 0.5 : 1;
        canvas().zoomAt(
          index % 2 === 0 ? base * 0.9 : base,
          anchor.x,
          anchor.y,
        );
        expected = () => {
          const after = canvas().clientToWorld(
            bounds.left + anchor.x,
            bounds.top + anchor.y,
          );
          ensure(
            Math.abs(after.x - before.x) < 0.0001 &&
              Math.abs(after.y - before.y) < 0.0001,
            "Zoom anchor moved.",
          );
          ensure(
            !cameraEqual(canvas().navigationState().camera, beforeCamera),
            "Zoom did not change view.",
          );
        };
      } else if (action === "frame-selection" || action === "home") {
        if (action === "frame-selection") {
          framedId = index % 2 === 0 ? `n${Math.floor((count - 4) / 2)}` : "n0";
          context.select([framedId]);
        }
        if (action === "frame-selection") await tick();
        ensure(
          action === "home" ? canvas().home() : canvas().frameSelection(),
          "Framing request failed.",
        );
        observeCamera = true;
      } else if (action === "activity") {
        const value = index % 2 === 0;
        updateNode("n0", (node) => ({ ...node, active: value }));
        expected = () =>
          ensure(
            element("n0")?.classList.contains("active") === value,
            "Activity did not render.",
          );
      } else if (action === "payload") {
        const label = index % 2 === 0 ? "Node alpha" : "Node omega";
        updateNode("n0", (node) => ({ ...node, label }));
        expected = () =>
          ensure(
            element("n0")
              ?.querySelector(".node-title-label")
              ?.textContent?.trim() === label,
            "Payload did not render.",
          );
      } else if (action === "loop") {
        const x = index % 2 === 0 ? 20 : 21;
        updateNode("loop", (node) => ({ ...node, position: { x, y: -30 } }));
        expected = () => assertNode("loop");
      } else if (action === "obstacle") {
        const y = index % 2 === 0 ? -48 : -40;
        updateNode("obstacle", (node) => ({ ...node, position: { x: 40, y } }));
        expected = () => assertNode("obstacle");
      }
      await tick();
      if (observeCamera) {
        while (cameraEqual(canvas().navigationState().camera, beforeCamera)) {
          ensure(
            performance.now() - started < 1000,
            "Framing produced no feedback.",
          );
          await frame();
          await tick();
        }
        ensure(
          root().querySelector<HTMLElement>(".world-pan")!.style.transform !==
            "",
          "Camera transform was not rendered.",
        );
      } else expected();
      const feedbackMs = rounded(performance.now() - started);
      const atFeedback = counts();
      afterFeedback?.();
      await settled();
      if (action === "frame-selection") {
        assertNode(framedId);
        const nodeRect = element(framedId)!.getBoundingClientRect(),
          bounds = root().getBoundingClientRect();
        ensure(
          Math.abs(
            nodeRect.x + nodeRect.width / 2 - bounds.x - bounds.width / 2,
          ) < 2 &&
            Math.abs(
              nodeRect.y + nodeRect.height / 2 - bounds.y - bounds.height / 2,
            ) < 2,
          "Frame Selection did not center its node.",
        );
      }
      if (action === "home") {
        const center = canvas().viewportCenter();
        const expectedY = (-50 + Math.floor((count - 5) / 64) * 6 + 5) / 2;
        ensure(
          Math.abs(center.x - 498.5) < 0.2 &&
            Math.abs(center.y - expectedY) < 0.2,
          "Home did not center the document bounds.",
        );
      }
      if (action === "drag-single" || action === "drag-multi")
        for (const id of action === "drag-single"
          ? ["n0"]
          : ["n0", "n1", "n64", "n65"])
          assertNode(id);
      if (action === "obstacle") {
        const path = wire("crossing");
        ensure(path, "Crossing wire disappeared.");
        const blocked = index % 2 === 0;
        ensure(
          path.isPointInStroke({ x: 46 * 16, y: -47.195 * 16 }) !== blocked,
          "Obstacle did not update the crossing stroke.",
        );
      }
      if (action === "loop") {
        const path = wire("loop-edge");
        ensure(path, "Self-loop disappeared.");
        const bounds = path.getBBox();
        ensure(bounds.height > 0, "Self-loop became a flat wire.");
      }
      const completeMs = rounded(performance.now() - started);
      await frame();
      const work = canvas().presentationWork();
      ensure(
        graph().nodes.length === count,
        "Authored identities changed during an action.",
      );
      return {
        feedbackMs,
        completeMs,
        ...atFeedback,
        finalView: counts(),
        projectedNodes: work.projectedNodes - beforeWork.projectedNodes,
        intervalsMs: intervals,
      };
    } finally {
      active = false;
      cancelAnimationFrame(handle);
    }
  }

  async function run(action: Action) {
    await prepare(action);
    const warmups = [],
      samples = [];
    for (let index = 0; index < 34; index += 1) {
      const value = await sample(action, index);
      (index < 3 ? warmups : samples).push(value);
    }
    const intervals = samples.flatMap((sample) => sample.intervalsMs);
    const stress = count === 100000;
    const summary = {
      feedbackP95: percentile(
        samples.map((sample) => sample.feedbackMs),
        0.95,
      ),
      completionP95: percentile(
        samples.map((sample) => sample.completeMs),
        0.95,
      ),
      intervalP95: percentile(intervals, 0.95),
      intervalMax: intervals.reduce(
        (maximum, value) => Math.max(maximum, value),
        0,
      ),
    };
    const limits = {
      feedbackP95: stress ? 400 : 200,
      completionP95: stress ? 4000 : 2000,
      intervalP95: stress ? 66.8 : 33.4,
      intervalMax: stress ? 400 : 200,
    };
    return {
      action,
      nodes: count,
      warmups,
      samples,
      summary,
      limits,
      passed: Object.keys(summary).every(
        (key) =>
          summary[key as keyof typeof summary] <=
          limits[key as keyof typeof limits],
      ),
    };
  }
  return { setup, run };
}
