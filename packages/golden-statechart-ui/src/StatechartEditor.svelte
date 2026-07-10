<script lang="ts">
  import GraphCanvas from "@golden/graph-ui/GraphCanvas.svelte";
  import type { GraphEditorModel, Rect } from "@golden/graph-ui";

  import { statechartGraphUiDomain, type StatechartUiNode } from "./index";
  import type { StatechartStore } from "./statechart-store";

  interface Props {
    store: StatechartStore;
    model: GraphEditorModel<StatechartUiNode, string>;
    viewport: Rect;
  }

  let { store, model, viewport }: Props = $props();
  let active = $derived([...store.activeStates]);
</script>

<section class="statechart-editor" aria-label="Statechart editor">
  <div class="graph-region">
    <GraphCanvas {model} domain={statechartGraphUiDomain} {viewport} />
  </div>
  <aside aria-label="Active state configuration">
    <h2>Active states</h2>
    {#each active as state (state)}
      <code>{state}</code>
    {/each}
  </aside>
</section>

<style>
  .statechart-editor { display: grid; grid-template-columns: 1fr minmax(12rem, 20%); height: 100%; }
  .graph-region { min-width: 0; }
  aside { padding: 0.75em; border-inline-start: 0.0625rem solid currentColor; overflow: auto; }
  aside h2 { margin-block: 0 0.75em; font-size: 0.9em; }
  aside code { display: block; padding-block: 0.35em; overflow-wrap: anywhere; }
</style>
