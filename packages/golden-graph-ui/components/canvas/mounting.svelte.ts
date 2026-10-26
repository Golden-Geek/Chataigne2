import { RetainedMounts } from "../../retained-mounts";
import { wireViewKey, type GraphVisibleEdge } from "../../retained-edges";
import type { GraphNode } from "../../types";
import type { CanvasGeometry } from "./geometry.svelte";
import { onDestroy, untrack } from "svelte";
import { unionBounds } from "../../wire-geometry";
import type { SpatialBounds } from "../../spatial-index";

export function createCanvasMounting(
  geometry: CanvasGeometry,
  focusedId: () => string | null = () => null,
) {
  const nodes = new RetainedMounts<GraphNode>((node) => node.id);
  const edges = new RetainedMounts<GraphVisibleEdge>(wireViewKey);
  let view = $state.raw({ nodes: nodes.values, edges: edges.values });
  let pending = $state.raw({ nodes: 0, edges: 0 });
  let edgeGroups = $derived.by(() => {
    const groups = new Map<
      number,
      { key: number; edges: GraphVisibleEdge[]; bounds: SpatialBounds }
    >();
    for (const edge of view.edges) {
      const key = Math.floor(edge.index / 32);
      const group = groups.get(key);
      if (group) {
        group.edges.push(edge);
        group.bounds = unionBounds(group.bounds, edge.wire.bounds);
      } else
        groups.set(key, {
          key,
          edges: [edge],
          bounds: { ...edge.wire.bounds },
        });
    }
    return [...groups.values()];
  });
  let scheduled: number | null = null;
  const publish = () => {
    if (view.nodes !== nodes.values || view.edges !== edges.values)
      view = { nodes: nodes.values, edges: edges.values };
    pending = { nodes: nodes.pending, edges: edges.pending };
  };
  const schedule = () => {
    if (scheduled === null && (nodes.pending || edges.pending))
      scheduled = requestAnimationFrame(step);
  };
  const step = () => {
    scheduled = null;
    nodes.step(32);
    edges.step(64);
    publish();
    schedule();
  };
  $effect(() => {
    const visible = geometry.visibleNodes;
    const id = focusedId();
    const focused = id ? geometry.nodeById(id) : undefined;
    const targetNodes = focused
      ? [focused, ...visible.filter((node) => node.id !== focused.id)]
      : visible;
    const targetEdges = geometry.visibleEdges;
    // One owner drives membership; route and source publications only replace its latest target.
    untrack(() => {
      nodes.setTarget(targetNodes);
      edges.setTarget(targetEdges);
      nodes.step(0);
      edges.step(0);
      publish();
      schedule();
    });
  });
  onDestroy(() => {
    if (scheduled !== null) cancelAnimationFrame(scheduled);
  });
  return {
    get nodes() {
      return view.nodes;
    },
    get edges() {
      return view.edges;
    },
    get edgeGroups() {
      return edgeGroups;
    },
    work() {
      return { ...pending };
    },
  };
}
