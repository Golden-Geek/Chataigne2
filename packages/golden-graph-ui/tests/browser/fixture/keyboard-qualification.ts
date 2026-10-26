import { tick } from "svelte";
import type { QualificationContext } from "./qualification";
import type { GraphNode, GraphSocket } from "../../../types";

const frame = () => new Promise<number>(requestAnimationFrame);
function ensure(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}
const p95 = (values: number[]) =>
  [...values].sort((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1] ?? 0;
const vector = (): GraphSocket => ({
  id: "vector",
  label: "Vector",
  children: [
    {
      id: "x",
      label: "X",
      parentId: "vector",
      children: [{ id: "leaf", label: "Leaf", parentId: "x" }],
    },
  ],
});

export function createKeyboardQualification(
  context: QualificationContext & { configure(): void },
) {
  let count = 0;
  const canvas = context.canvas;
  const root = () => document.querySelector<HTMLElement>(".graph-canvas")!;
  const sourceId = () => `n${count - 3}`;
  const targetId = () => `n${count - 1}`;
  const element = (id: string) =>
    root().querySelector<HTMLElement>(`article[data-node-id="${id}"]`);
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
    throw new Error("Keyboard scene did not settle");
  };
  const verifyBounds = () => {
    const nodes = root().querySelectorAll("article.node").length;
    ensure(nodes <= 128, `Mounted ${nodes} nodes during keyboard operation`);
    return nodes;
  };
  const prepare = async (action: string) => {
    const graph = context.graph();
    const nodes = graph.nodes.map((node, index) =>
      index < count - 3
        ? node
        : {
            ...node,
            collapsed: action === "collapsed",
            position: {
              x: (index % 256) * 24,
              y: Math.floor(index / 256) * 24,
            },
          },
    );
    // Restore the unchanged chain workload before each measured mutation.
    const edges = graph.edges.filter(
      (edge) =>
        edge.id !== "acknowledged-keyboard-connection" &&
        edge.id !== `e${count - 2}`,
    );
    edges.push({
      id: `e${count - 2}`,
      from: { nodeId: `n${count - 2}`, socketId: "out" },
      to: { nodeId: targetId(), socketId: "in" },
    });
    context.publish(nodes, edges);
    await tick();
    for (const [id, direction] of [
      [sourceId(), "output"],
      [targetId(), "input"],
    ] as const) {
      const expanded = element(id)?.querySelectorAll<HTMLElement>(
        `.socket-list.${direction}s .socket-expander[aria-expanded="true"]`,
      );
      for (const button of [...(expanded ?? [])].reverse()) {
        const socket = button
          .closest(".socket-row")
          ?.querySelector<HTMLElement>("[data-socket-id]")?.dataset.socketId;
        if (socket)
          ensure(
            canvas().toggleSocketExpanded(id, socket, direction),
            "Cannot reset expanded socket",
          );
      }
    }
    await tick();
    ensure(
      await canvas().revealNode(sourceId()),
      "Cannot focus the source node",
    );
    if (action === "multi-move")
      context.select([sourceId(), `n${count - 2}`, targetId()]);
    await settle();
    verifyBounds();
  };
  const activate = async (
    id: string,
    socket: string,
    direction: "input" | "output",
  ) => {
    ensure(
      canvas().activateSocketWithKeyboard(id, socket, direction),
      `Cannot activate ${id}/${socket}`,
    );
    await tick();
  };
  const assertPending = (id: string, socket: string, direction: string) => {
    const button = element(id)?.querySelector<HTMLButtonElement>(
      `[data-socket-id="${socket}"][data-socket-direction="${direction}"]`,
    );
    ensure(
      button?.getAttribute("aria-pressed") === "true",
      "Pending endpoint is not exposed",
    );
  };
  const action = async (name: string) => {
    const from = sourceId(),
      to = targetId();
    if (name === "single-move" || name === "multi-move") {
      const multi = name === "multi-move";
      ensure(
        canvas().moveSelectionWithKeyboard(
          multi ? "ArrowUp" : "ArrowRight",
          multi,
        ),
        "Movement was not accepted",
      );
      await tick();
      for (let index = count - 3; index < count; index++) {
        const expected = {
          x: (index % 256) * 24 + (!multi && index === count - 3 ? 0.25 : 0),
          y: Math.floor(index / 256) * 24 - (multi ? 2 : 0),
        };
        const node = context.graph().nodes[index];
        ensure(
          node.position.x === expected.x && node.position.y === expected.y,
          "Authoritative fixture movement differs",
        );
        const rendered = element(node.id);
        ensure(
          rendered &&
            Number.parseFloat(rendered.style.left) === expected.x &&
            Number.parseFloat(rendered.style.top) === expected.y,
          "Mounted movement differs",
        );
      }
      ensure(
        canvas().keyboardState().message.startsWith("Move requested"),
        "Move feedback missing",
      );
    } else {
      let socket = "out",
        target = "in";
      if (name === "compound") {
        for (const id of [from, to])
          for (const parent of ["vector", "x"])
            ensure(
              canvas().toggleSocketExpanded(
                id,
                parent,
                id === from ? "output" : "input",
              ),
              "Compound socket cannot expand",
            );
        socket = target = "leaf";
      } else if (name === "collapsed") {
        ensure(
          canvas().toggleNodeCollapsed(from) &&
            canvas().toggleNodeCollapsed(to),
          "Collapsed node cannot expand",
        );
        await tick();
        ensure(
          context.graph().nodes[count - 3].collapsed === false &&
            context.graph().nodes[count - 1].collapsed === false,
          "Collapse acknowledgement missing",
        );
      }
      if (name === "input-first") {
        await activate(to, target, "input");
        assertPending(to, target, "input");
        await activate(from, socket, "output");
      } else {
        await activate(from, socket, "output");
        assertPending(from, socket, "output");
        await activate(to, target, "input");
      }
      const edge = context
        .graph()
        .edges.find((edge) => edge.id === "acknowledged-keyboard-connection");
      ensure(
        edge?.from.nodeId === from &&
          edge.from.socketId === socket &&
          edge.to.nodeId === to &&
          edge.to.socketId === target,
        "Directed fixture acknowledgement differs",
      );
      ensure(
        canvas().keyboardState().endpoint === null &&
          canvas().keyboardState().message === "Connection requested.",
        "Connection feedback differs",
      );
    }
    const focused = document.activeElement?.closest("article");
    ensure(
      focused?.dataset.nodeId === from,
      "Keyboard mutation lost source focus",
    );
    verifyBounds();
  };
  return {
    async setup(size: number) {
      const started = performance.now();
      count = size;
      context.configure();
      context.size(70, 36);
      context.select([]);
      const nodes: GraphNode[] = Array.from({ length: count }, (_, index) => ({
        id: `n${index}`,
        label: `Node ${index}`,
        position: { x: (index % 256) * 24, y: Math.floor(index / 256) * 24 },
        inputs: [
          { id: "in", label: "Input" },
          ...(index >= count - 3 ? [vector()] : []),
        ],
        outputs: [
          { id: "out", label: "Output" },
          ...(index >= count - 3 ? [vector()] : []),
        ],
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
      canvas().setCamera({ x: 0, y: 0, zoom: 0.5 });
      await settle();
      return {
        count,
        setupMs: performance.now() - started,
        mounted: verifyBounds(),
      };
    },
    async run(name: string) {
      const samples: { feedback: number; completion: number; stall: number }[] =
        [];
      for (let index = -3; index < 31; index++) {
        await prepare(name);
        const started = performance.now();
        let last = started,
          stall = 0,
          raf = 0;
        const observe = (now: number) => {
          stall = Math.max(stall, now - last);
          last = now;
          raf = requestAnimationFrame(observe);
        };
        raf = requestAnimationFrame(observe);
        try {
          await action(name);
          await frame();
          const feedback = performance.now() - started;
          await settle();
          await frame();
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
  };
}
