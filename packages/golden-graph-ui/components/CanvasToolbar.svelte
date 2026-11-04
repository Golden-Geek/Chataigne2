<script lang="ts">
	import type { Snippet } from 'svelte';
	import { canvasNavigationCommand } from '../canvas-shortcuts';
	let {
		zoom,
		minZoom = 0.1,
		maxZoom = 8,
		frame,
		home,
		setZoom,
		canFrame = true,
		disabled = false,
		label = 'View navigation',
		frameLabel = 'Frame selection',
		homeLabel = 'Home view',
		navigation,
		actions
	}: {
		zoom?: number;
		minZoom?: number;
		maxZoom?: number;
		frame(): void;
		home(): void;
		setZoom?(value: number): void;
		canFrame?: boolean;
		disabled?: boolean;
		label?: string;
		frameLabel?: string;
		homeLabel?: string;
		navigation?: Snippet;
		actions?: Snippet;
	} = $props();
</script>

<div
	class="canvas-toolbar toolbar navigation"
	data-native-preview-input-exclude
	role="toolbar"
	tabindex="-1"
	aria-label={label}
	onpointerdown={(event) => event.stopPropagation()}
	onkeydown={(event) => {
		event.stopPropagation();
		if (disabled || event.target instanceof HTMLInputElement) return;
		const command = canvasNavigationCommand(event);
		if (command === 'home') home();
		else if (command === 'frame' && canFrame) frame();
		else if (command === 'zoom-in' && zoom != null && setZoom)
			setZoom(Math.min(maxZoom, zoom * 1.2));
		else if (command === 'zoom-out' && zoom != null && setZoom)
			setZoom(Math.max(minZoom, zoom / 1.2));
		else return;
		event.preventDefault();
	}}>
	<button
		type="button"
		aria-label={frameLabel}
		title="Frame selection (F)"
		disabled={disabled || !canFrame}
		onclick={frame}>Frame</button>
	<button type="button" aria-label={homeLabel} title="Home (H / Home)" {disabled} onclick={home}
		>Home</button>
	{#if navigation}{@render navigation()}{/if}
	{#if zoom != null && setZoom}<div class="zoom-control">
			<button
				type="button"
				aria-label="Zoom out"
				title="Zoom out (-)"
				{disabled}
				onclick={() => setZoom(Math.max(minZoom, zoom / 1.2))}>−</button>
			<input
				class="zoom-slider"
				type="range"
				min={minZoom * 100}
				max={maxZoom * 100}
				step="any"
				value={zoom * 100}
				aria-label="Zoom"
				title="Zoom"
				{disabled}
				oninput={(event) => setZoom(Number(event.currentTarget.value) / 100)} />
			<button
				type="button"
				class="zoom-reset"
				aria-label="Reset zoom to 100%"
				title="Reset zoom to 100%"
				{disabled}
				onclick={() => setZoom(1)}
				><output aria-label="View zoom" data-zoom={zoom}>{Math.round(zoom * 100)}%</output></button>
			<button
				type="button"
				aria-label="Zoom in"
				title="Zoom in (+)"
				{disabled}
				onclick={() => setZoom(Math.min(maxZoom, zoom * 1.2))}>+</button>
		</div>{/if}
	{#if actions}<span class="toolbar-divider" aria-hidden="true"></span>{@render actions()}{/if}
</div>

<style>
	.canvas-toolbar {
		--view-bg: var(--ga-bg, var(--gc-color-panel, #252525));
		--view-outline: var(--ga-outline, var(--gc-color-text, #ddd));
		--view-selection: var(--ga-selection, var(--gc-color-focus, #a3bfff));
		position: absolute;
		inset-block-start: 0.5rem;
		inset-inline-start: 50%;
		transform: translateX(-50%);
		z-index: 20;
		display: flex;
		align-items: center;
		gap: 0.3rem;
		inline-size: max-content;
		max-inline-size: calc(100% - 1rem);
		overflow-x: auto;
		padding: 0.28rem;
		border: solid 0.06rem color-mix(in srgb, var(--view-outline) 55%, transparent);
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--view-bg) 92%, transparent);
		color: var(--gc-color-text, inherit);
		backdrop-filter: blur(0.5rem);
		box-sizing: border-box;
	}
	.canvas-toolbar :global(button:not(.gc-icon-button)) {
		flex: none;
		min-block-size: 1.65rem;
		padding: 0.25rem 0.5rem;
		border: 0;
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--view-outline) 18%, transparent);
		color: inherit;
		font: inherit;
		font-size: 0.67rem;
		cursor: pointer;
	}
	.canvas-toolbar :global(button:not(.gc-icon-button):hover) {
		background: color-mix(in srgb, var(--view-selection) 28%, transparent);
	}
	.canvas-toolbar :global(button:disabled) {
		opacity: 0.4;
		cursor: default;
	}
	.zoom-control {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		flex: none;
	}
	.zoom-slider {
		inline-size: 4.8rem;
		block-size: 0.16rem;
		margin: 0;
		accent-color: var(--view-outline);
	}
	.zoom-reset {
		min-inline-size: 3.15rem;
		font-variant-numeric: tabular-nums;
	}
	.toolbar-divider {
		flex: none;
		inline-size: 0.06rem;
		block-size: 1rem;
		background: color-mix(in srgb, var(--view-outline) 55%, transparent);
	}
	@media (max-width: 34rem) {
		.zoom-slider {
			inline-size: 3rem;
		}
	}
</style>
