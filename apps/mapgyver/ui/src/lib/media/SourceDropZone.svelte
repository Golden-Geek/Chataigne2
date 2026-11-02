<script lang="ts">
	import { onDestroy, type Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { appState, readNodeDragPayload, resolveDragNode } from 'golden_ui';
	let {
		disabled = false,
		addLabel,
		replaceOnly = null,
		hitTest,
		onsource,
		children,
		...attributes
	}: Omit<HTMLAttributes<HTMLDivElement>, 'children' | 'ondragover' | 'ondragleave' | 'ondrop'> & {
		disabled?: boolean;
		addLabel: string;
		replaceOnly?: string | null;
		hitTest?(event: DragEvent): string | null;
		onsource(source: string, target: string | null, event: DragEvent): void;
		children: Snippet;
	} = $props();
	let element: HTMLDivElement;
	let hovering = $state(false),
		replacing = $state(false);
	let highlighted: Element | null = null;
	function source(event: DragEvent) {
		const session = appState.session;
		if (
			disabled ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession
		)
			return null;
		const node = resolveDragNode(session.graph.state, readNodeDragPayload(event), session);
		return node?.node_type === 'mapgyver.media' || node?.node_type === 'mapgyver.media.channel'
			? node
			: null;
	}
	function object(event: DragEvent) {
		const target =
			event.target instanceof Element ? event.target.closest('[data-source-drop-object]') : null;
		return target && element.contains(target) ? target : null;
	}
	function reset() {
		hovering = false;
		highlighted?.removeAttribute('data-source-drop-hover');
		highlighted = null;
	}
	function targetAt(event: DragEvent): string | null {
		if (replaceOnly !== null) return replaceOnly;
		const hovered = object(event);
		// Explicit editor handles stay selectable above the rendered scene. Body picking uses
		// the complete cached scene, independently of the bounded set of SVG affordances.
		if (hovered?.matches('button')) return hovered.getAttribute('data-source-drop-object');
		return hitTest ? hitTest(event) : (hovered?.getAttribute('data-source-drop-object') ?? null);
	}
	$effect(() => {
		if (disabled) reset();
	});
	onDestroy(reset);
</script>

<svelte:window ondragend={reset} />

<div
	{...attributes}
	bind:this={element}
	class="source-drop-zone {attributes.class ?? ''}"
	class:drop-active={hovering}
	ondragover={(event) => {
		if (!source(event)) {
			reset();
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
		const picked = targetAt(event);
		const hovered = object(event);
		const target = hovered?.getAttribute('data-source-drop-object') === picked ? hovered : null;
		if (highlighted !== target) {
			highlighted?.removeAttribute('data-source-drop-hover');
			highlighted = target;
			highlighted?.setAttribute('data-source-drop-hover', '');
		}
		replacing = picked !== null;
		hovering = true;
	}}
	ondragleave={(event) => {
		if (event.relatedTarget instanceof Node && element.contains(event.relatedTarget)) return;
		const rect = element.getBoundingClientRect();
		if (
			event.clientX <= rect.left ||
			event.clientX >= rect.right ||
			event.clientY <= rect.top ||
			event.clientY >= rect.bottom
		)
			reset();
	}}
	ondrop={(event) => {
		const node = source(event);
		if (!node) {
			reset();
			return;
		}
		const target = targetAt(event);
		reset();
		event.preventDefault();
		event.stopPropagation();
		onsource(node.uuid, target, event);
	}}>
	{@render children()}
	{#if hovering}<span class="drop-hint" role="status"
			>{replacing ? 'Replace source' : addLabel}</span
		>{/if}
</div>

<style>
	.source-drop-zone {
		position: relative;
		width: 100%;
		height: 100%;
		min-width: 0;
		min-height: 0;
	}
	.drop-active {
		outline: 0.12rem solid var(--gc-focus-color, #a3bfff);
		outline-offset: -0.12rem;
	}
	.drop-hint {
		position: absolute;
		inset: auto auto 0.5rem 0.5rem;
		z-index: 3;
		padding: 0.3em 0.6em;
		border-radius: 0.25rem;
		background: var(--gc-color-panel, #252525);
		color: var(--text-color, #ddd);
		font-size: 0.8rem;
		pointer-events: none;
	}
	:global(polygon[data-source-drop-hover]) {
		stroke: #b6ed89;
		fill: #b6ed8926;
	}
	:global(button[data-source-drop-hover]) {
		outline: 0.15rem solid #b6ed89;
	}
</style>
