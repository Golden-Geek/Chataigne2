<script lang="ts">
	import { appState, type UiBrowserPreviewExtent } from 'golden_ui';
	import { untrack, type Snippet } from 'svelte';
	import { useBrowserPreviews } from './browser-preview.svelte';

	let {
		source,
		label,
		visible = true,
		lifetime = 0,
		fill = false,
		onextent,
		children
	}: {
		source: string;
		label: string;
		visible?: boolean;
		lifetime?: string | number;
		fill?: boolean;
		onextent?(extent: UiBrowserPreviewExtent): void;
		children?: Snippet;
	} = $props();
	let canvas: HTMLCanvasElement | undefined = $state();
	let intersecting = $state(false);
	let pageVisible = $state(false);
	let hasFrame = $state(false);
	const previews = useBrowserPreviews();
	const enabled = $derived(previews?.mode !== 'disabled');
	const viewLifetime = $derived(lifetime);

	$effect(() => {
		const target = canvas;
		if (!target) return;
		const updatePageVisibility = () => (pageVisible = !document.hidden);
		updatePageVisibility();
		document.addEventListener('visibilitychange', updatePageVisibility);
		const observer = new IntersectionObserver(([entry]) => {
			intersecting = !!entry?.isIntersecting && entry.intersectionRatio > 0;
		});
		observer.observe(target);
		return () => {
			observer.disconnect();
			document.removeEventListener('visibilitychange', updatePageVisibility);
		};
	});

	$effect(() => {
		const target = canvas;
		const request = appState.session?.client.browserPreview;
		const key = source;
		viewLifetime;
		if (!target || !request || !previews || !enabled || !visible || !intersecting || !pageVisible)
			return;
		const context = target.getContext('2d', { alpha: false });
		if (!context) return;
		let previous: UiBrowserPreviewExtent | undefined;
		const withdraw = untrack(() =>
			previews.register(key, request, (image, extent) => {
				if (target.width !== image.width || target.height !== image.height) {
					target.width = image.width;
					target.height = image.height;
				}
				context.drawImage(image, 0, 0);
				if (previous?.width !== extent.width || previous.height !== extent.height) {
					previous = extent;
					onextent?.(extent);
				}
				hasFrame = true;
			})
		);
		return () => {
			withdraw();
			context.clearRect(0, 0, target.width, target.height);
			hasFrame = false;
		};
	});
</script>

<div class="browser-preview">
	<div class="image" role="img" aria-label={label}>
		<canvas
			bind:this={canvas}
			class:fill
			data-preview-state={!enabled ? 'disabled' : hasFrame ? 'available' : 'waiting'}
		></canvas>
	</div>
	{#if children}<div class="overlay">{@render children()}</div>{/if}
</div>

<style>
	.browser-preview {
		position: relative;
	}
	.browser-preview,
	.image,
	canvas {
		display: block;
		width: 100%;
		height: 100%;
		object-fit: contain;
		background: black;
	}
	canvas.fill {
		object-fit: fill;
	}
	.overlay {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
</style>
