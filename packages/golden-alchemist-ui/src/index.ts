import type { GraphPortView, GraphUiDomain } from "@golden/graph-ui";

export interface AlchemistUiNode {
  operation: string;
  inputs: readonly GraphPortView<string>[];
  outputs: readonly GraphPortView<string>[];
}

export const alchemistGraphUiDomain: GraphUiDomain<AlchemistUiNode, string> = {
  nodeLabel: (node) => node.operation,
  nodeKind: () => "alchemist-operation",
  nodePorts: (node) => [...node.inputs, ...node.outputs],
};
