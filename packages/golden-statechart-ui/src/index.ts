import type { GraphPortView, GraphUiDomain } from "@golden/graph-ui";

export interface StatechartUiNode {
  label: string;
  kind: "initial" | "atomic" | "compound" | "final";
  ports: readonly GraphPortView<"transition">[];
}

export const statechartGraphUiDomain: GraphUiDomain<StatechartUiNode, "transition"> = {
  nodeLabel: (node) => node.label,
  nodeKind: (node) => `statechart-${node.kind}`,
  nodePorts: (node) => node.ports,
};
