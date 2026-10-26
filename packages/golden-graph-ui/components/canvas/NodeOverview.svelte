<script lang="ts">
	import type { GraphNode } from '../../types';
	import type { CanvasGeometry } from './geometry.svelte';
	import type { CanvasEditing } from './editing.svelte';
	import type { CanvasKeyboard } from './keyboard.svelte';
	let {
		node,
		geometry,
		editing,
		keyboard
	}: {
		node: GraphNode;
		geometry: CanvasGeometry;
		editing: CanvasEditing;
		keyboard: CanvasKeyboard;
	} = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<article
	class="node node-header overview"
	class:active={node.active}
	class:disabled={node.enabled === false}
	class:invalid={node.invalid}
	class:collapsed={node.collapsed === true}
	data-node-id={node.id}
	data-presentation="overview"
	title={node.warning || node.description || node.subtitle || node.label}
	style:left={`${node.position.x}rem`}
	style:top={`${node.position.y}rem`}
	style:width={`${geometry.nodeWidth(node)}rem`}
	style:height={`${geometry.nodeHeight(node)}rem`}
	style:--node-accent={node.color ?? 'var(--ga-outline)'}
	onpointerdown={(event) => editing.handleNodeTitlePointerDown(event, node)}
	onkeydown={(event) => keyboard.handleNodeKeydown(event, node.id)}
	ondblclick={(event) => editing.beginNodeRename(event, node)}>
	<button
		type="button"
		class="node-title-label"
		aria-pressed="false"
		onclick={() => keyboard.selectNode(node.id)}>{node.label}</button>
	{#if node.warning}<span class="warning" aria-label={node.warning}>!</span>{/if}
	{#if node.inputs.length || node.headerInputs?.length}<span class="port input"></span>{/if}
	{#if node.outputs.length}<span class="port output"></span>{/if}
</article>

<style>
	.overview {
		position: absolute;
		box-sizing: border-box;
		border: solid 0.1rem var(--node-accent);
		border-radius: 0.4rem;
		background: var(--ga-node);
		color: var(--gc-color-text, #d8dfeb);
		font-size: 0.8rem;
		cursor: grab;
	}
	.node-title-label {
		display: block;
		overflow: hidden;
		padding: 0.4rem;
		text-align: center;
		text-overflow: ellipsis;
		white-space: nowrap;
		inline-size: 100%;
		border: 0;
		background: transparent;
		color: inherit;
		font: inherit;
	}
	.active {
		border-color: var(--ga-active);
	}
	.invalid {
		border-color: var(--ga-error);
	}
	.disabled {
		opacity: 0.55;
	}
	.port {
		position: absolute;
		inset-block-start: 50%;
		inline-size: 0.4rem;
		block-size: 0.4rem;
		border-radius: 50%;
		background: var(--ga-socket);
	}
	.input {
		inset-inline-start: -0.2rem;
	}
	.output {
		inset-inline-end: -0.2rem;
	}
	.warning {
		position: absolute;
		inset: 0.25rem 0.3rem auto auto;
		color: var(--ga-error);
	}
</style>
