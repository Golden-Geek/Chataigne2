<script>
  let { model, domain, viewport } = $props();
  let visibleNodes = $derived(model.visibleNodes(viewport));

  function nodeStyle(geometry) {
    return `transform: translate(${geometry.x}px, ${geometry.y}px); width: ${geometry.width}px; height: ${geometry.height}px`;
  }
</script>

<div class="graph-canvas" role="application" aria-label="Graph editor">
  {#each visibleNodes as visible (visible.node.id)}
    <button
      type="button"
      class="graph-node"
      data-node-kind={domain.nodeKind(visible.node.data)}
      style={nodeStyle(visible.geometry)}
    >
      {domain.nodeLabel(visible.node.data)}
    </button>
  {/each}
</div>

<style>
  .graph-canvas {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .graph-node {
    position: absolute;
    box-sizing: border-box;
    min-width: 8rem;
    min-height: 3rem;
    padding: 0.5em;
    border: 0.0625rem solid currentColor;
    border-radius: 0.35em;
    font: inherit;
    text-align: start;
  }
</style>
