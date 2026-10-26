import { tick } from "svelte";
import type { QualificationContext } from "./qualification";
import type { GraphNode } from "../../../types";

const frame = () => new Promise<number>(requestAnimationFrame);
function ensure(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}
const p95 = (values: number[]) =>
  [...values].sort((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1] ?? 0;

export function createNavigationQualification(context: QualificationContext) {
  let count = 0;
  const canvas = context.canvas;
  const finder = () => canvas().nodeFinderState();
  const root = () => document.querySelector<HTMLElement>(".graph-canvas")!;
  const settle = async () => {
    const started = performance.now();
    do {
      await tick();
      if (
        !canvas().mountingWork().nodes &&
        !canvas().mountingWork().edges &&
        !canvas().routingWork().pendingRoutes
      )
        return;
      await frame();
    } while (performance.now() - started < 10000);
    throw new Error("Discovery scene did not settle");
  };
  const bounds = () => {
    const rows = root().querySelectorAll("[data-graph-result]").length;
    const nodes = root().querySelectorAll("article.node").length;
    ensure(rows <= 32, `Finder mounted ${rows} results`);
    ensure(nodes <= 128, `Canvas mounted ${nodes} nodes`);
    return { rows, nodes };
  };
  const verifyPage = (first: string, length = 32) => {
    ensure(
      finder().rows[0]?.id === first && finder().rows.length === length,
      `Unexpected logical page: ${JSON.stringify(finder())}`,
    );
    const firstElement = root().querySelector<HTMLElement>(
      "[data-graph-result]",
    );
    ensure(
      firstElement?.dataset.graphResult === first &&
        firstElement.textContent?.includes(`Node ${first.slice(1)}`),
      "Result text/identity drift",
    );
    bounds();
    const panel = root().querySelector<HTMLElement>(".node-navigator")!;
    const panelRect = panel.getBoundingClientRect();
    const viewportRect = root().getBoundingClientRect();
    ensure(
      panelRect.left >= viewportRect.left &&
        panelRect.right <= viewportRect.right &&
        panelRect.top >= viewportRect.top &&
        panelRect.bottom <= viewportRect.bottom,
      "Finder controls are clipped outside the canvas",
    );
  };
  const prepare = async (action: string) => {
    if (action === "reveal") {
      canvas().setCamera({ x: 0, y: 0, zoom: 0.25 });
      await settle();
    }
    await canvas().showNodeFinder();
    if (action === "clear" || action === "reveal")
      await canvas().searchNodes(`Node ${count - 1}`);
    await tick();
  };
  const action = async (name: string, feedback: () => Promise<void>) => {
    if (name === "search") {
      await canvas().searchNodes(`Node ${count - 1}`);
      await tick();
      verifyPage(`n${count - 1}`, 1);
      await feedback();
    } else if (name === "clear") {
      await canvas().searchNodes("");
      await tick();
      verifyPage("n0");
      await feedback();
    } else if (name === "page") {
      await canvas().pageNodes("next");
      await tick();
      verifyPage("n32");
      await feedback();
      await canvas().pageNodes("previous");
      await tick();
      verifyPage("n0");
    } else if (name === "reveal") {
      const id = `n${count - 1}`;
      ensure(await canvas().revealNode(id), "Far node was not focused");
      const focused = document.activeElement as HTMLElement;
      ensure(
        focused.closest("article")?.dataset.nodeId === id,
        "Wrong focused node",
      );
      const rect = focused.getBoundingClientRect();
      const viewport = root().getBoundingClientRect();
      ensure(
        rect.right > viewport.left &&
          rect.left < viewport.right &&
          rect.bottom > viewport.top &&
          rect.top < viewport.bottom,
        "Focused title is offscreen",
      );
      ensure(
        focused.getAttribute("aria-pressed") === "true",
        "Revealed node is not selected",
      );
      bounds();
      await feedback();
    } else throw new Error(`Unknown discovery action ${name}`);
    await settle();
    await frame();
    bounds();
  };
  return {
    async setup(size: number) {
      const started = performance.now();
      count = size;
      context.size(70, 36);
      context.select([]);
      const nodes: GraphNode[] = Array.from({ length: count }, (_, index) => ({
        id: `n${index}`,
        label: `Node ${index}`,
        position: { x: (index % 256) * 24, y: Math.floor(index / 256) * 24 },
        inputs: [{ id: "in", label: "Input" }],
        outputs: [{ id: "out", label: "Output" }],
      }));
      context.publish(
        nodes,
        nodes.slice(1).map((node, index) => ({
          id: `e${index}`,
          from: { nodeId: `n${index}`, socketId: "out" },
          to: { nodeId: node.id, socketId: "in" },
        })),
      );
      await tick();
      canvas().setCamera({ x: 0, y: 0, zoom: 0.25 });
      await settle();
      return { count, setupMs: performance.now() - started, mounted: bounds() };
    },
    async run(name: string) {
      const samples: { feedback: number; completion: number; stall: number }[] =
        [];
      for (let index = -3; index < 31; index++) {
        await prepare(name);
        const started = performance.now();
        let feedback = 0;
        let last = started;
        let stall = 0;
        let raf = 0;
        const observe = (now: number) => {
          stall = Math.max(stall, now - last);
          last = now;
          raf = requestAnimationFrame(observe);
        };
        raf = requestAnimationFrame(observe);
        try {
          await action(name, async () => {
            await frame();
            feedback = performance.now() - started;
          });
          stall = Math.max(stall, performance.now() - last);
          if (index >= 0)
            samples.push({
              feedback,
              completion: performance.now() - started,
              stall,
            });
        } finally {
          cancelAnimationFrame(raf);
        }
      }
      const feedbackP95 = p95(samples.map((sample) => sample.feedback));
      const completionP95 = p95(samples.map((sample) => sample.completion));
      const maximumStall = Math.max(...samples.map((sample) => sample.stall));
      return {
        count,
        action: name,
        samples,
        feedbackP95,
        completionP95,
        maximumStall,
        passed:
          feedbackP95 <= (count === 100000 ? 400 : 200) &&
          maximumStall <= (count === 100000 ? 400 : 200) &&
          completionP95 <= (count === 100000 ? 4000 : 2000),
      };
    },
    async correctness() {
      canvas().setCamera({ x: 0, y: 0, zoom: 0.25 });
      context.select([]);
      await settle();
      const overviewTitle = root().querySelector<HTMLButtonElement>(
        'article[data-presentation="overview"] .node-title-label',
      )!;
      ensure(overviewTitle, "Overview has no focusable title");
      const overviewId =
        overviewTitle.closest<HTMLElement>("article")!.dataset.nodeId!;
      overviewTitle.focus();
      canvas().selectNodeWithKeyboard(overviewId);
      await settle();
      ensure(
        document.activeElement?.closest("article")?.dataset.nodeId ===
          overviewId &&
          document.activeElement.getAttribute("aria-pressed") === "true",
        "Overview activation lost focus or selection",
      );
      await canvas().showNodeFinder();
      const obsolete = canvas().searchNodes("never matches");
      await canvas().searchNodes(`Node ${count - 1}`);
      await obsolete;
      await tick();
      verifyPage(`n${count - 1}`, 1);
      const row = root().querySelector<HTMLButtonElement>(
        "[data-graph-result]",
      )!;
      row.focus();
      await canvas().searchNodes("Node 0");
      await tick();
      ensure(
        document.activeElement?.getAttribute("type") === "search",
        "Lost filter focus fallback",
      );
      ensure(
        await canvas().revealNode(`n${count - 1}`),
        "Cannot reveal far node",
      );
      canvas().panBy(100000, 0);
      await settle();
      ensure(
        document.activeElement?.closest("article")?.dataset.nodeId ===
          `n${count - 1}`,
        "Panning unmounted the focused node",
      );
      const outside = document.createElement("button");
      root().parentElement!.append(outside);
      outside.focus();
      await settle();
      ensure(
        !root().querySelector(`article[data-node-id="n${count - 1}"]`),
        "Leaving the canvas retained its offscreen focus pin",
      );
      outside.remove();
      ensure(
        await canvas().revealNode(`n${count - 1}`),
        "Cannot refocus after leaving canvas",
      );
      const camera = canvas().navigationState().camera;
      canvas().setCamera({ ...camera, zoom: 0.25 });
      canvas().selectNodeWithKeyboard("n0");
      await settle();
      ensure(
        document.activeElement?.closest("article")?.dataset.nodeId ===
          `n${count - 1}`,
        "Overview replacement lost focus",
      );
      ensure(
        await canvas().revealNode(`n${count - 1}`),
        "Cannot leave overview",
      );
      await canvas().showNodeFinder();
      await canvas().searchNodes(`Node ${count - 1}`);
      const old = context.graph().nodes;
      context.publish(old.slice(0, -1), []);
      await tick();
      while (finder().busy) await frame();
      ensure(finder().rows.length === 0, "Deleted node remains discoverable");
      ensure(
        !(await canvas().revealNode(`n${count - 1}`)),
        "Deleted node can be revealed",
      );
      context.publish(
        old.map((node, index) =>
          index === old.length - 1
            ? { ...node, label: "Renamed target" }
            : node,
        ),
        [],
      );
      await tick();
      await canvas().searchNodes("Renamed target");
      await tick();
      ensure(
        finder().rows[0]?.id === `n${count - 1}`,
        "Rename is absent from discovery",
      );
      return { count, passed: true, mounted: bounds() };
    },
  };
}
