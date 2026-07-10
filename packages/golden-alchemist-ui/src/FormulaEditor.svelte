<script lang="ts">
  import GraphCanvas from "@golden/graph-ui/GraphCanvas.svelte";
  import type { GraphEditorModel, GraphUiDomain, Rect } from "@golden/graph-ui";

  import type { FormulaStore } from "./formula-store";
  import type { AlchemistUiNode } from "./index";

  interface Props {
    store: FormulaStore;
    model: GraphEditorModel<AlchemistUiNode, string>;
    domain: GraphUiDomain<AlchemistUiNode, string>;
    viewport: Rect;
  }

  let { store, model, domain, viewport }: Props = $props();
  let inputs = $derived([...store.inputs.values()]);
  let outputs = $derived([...store.outputs.values()]);

  function display(value: unknown): string {
    return typeof value === "string" ? value : JSON.stringify(value);
  }
</script>

<section class="formula-editor" aria-label="Formula editor">
  <aside class="surface-panel" aria-label="Formula inputs">
    <h2>Inputs</h2>
    {#each inputs as input (input.id)}
      <div class="surface-item"><span>{input.label}</span><small>{input.valueType}</small></div>
    {/each}
  </aside>
  <div class="graph-region">
    <GraphCanvas {model} {domain} {viewport} />
  </div>
  <aside class="surface-panel" aria-label="Formula outputs">
    <h2>Outputs</h2>
    {#each outputs as output (output.id)}
      <div class="surface-item">
        <span>{output.label}</span>
        <output>{display(store.runtimeOutputs.get(output.id))}</output>
      </div>
    {/each}
  </aside>
</section>

<style>
  .formula-editor { display: grid; grid-template-columns: minmax(10rem, 18%) 1fr minmax(10rem, 18%); height: 100%; }
  .graph-region { min-width: 0; }
  .surface-panel { padding: 0.75em; overflow: auto; border-inline: 0.0625rem solid currentColor; }
  .surface-panel h2 { margin-block: 0 0.75em; font-size: 0.9em; }
  .surface-item { display: flex; justify-content: space-between; gap: 0.75em; padding-block: 0.45em; }
  .surface-item small, .surface-item output { opacity: 0.75; }
</style>
