import type { Component } from "svelte";

import type { GraphEditorModel } from "./model";
import type { GraphUiDomain, Rect } from "./types";

declare const GraphCanvas: Component<{
  model: GraphEditorModel<any, any>;
  domain: GraphUiDomain<any, any>;
  viewport: Rect;
}>;

export default GraphCanvas;
