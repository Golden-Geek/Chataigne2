<script lang="ts">
	import { appState } from 'golden_ui';
	import { untrack } from 'svelte';
	import { useBrowserPreviews } from './browser-preview.svelte';

	let { source, label }: { source: string; label: string } = $props();
	let canvas: HTMLCanvasElement | undefined = $state();
	let intersecting = $state(false);
	let pageVisible = $state(false);
	let hasFrame = $state(false);
	const previews = useBrowserPreviews();
	const enabled = $derived(previews?.mode !== 'disabled');

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
		if (!target || !request || !previews || !enabled || !intersecting || !pageVisible) return;
		const context = target.getContext('2d', { alpha: false });
		if (!context) return;
		const withdraw = untrack(() =>
			previews.register(key, request, (image) => {
				if (target.width !== image.width || target.height !== image.height) {
					target.width = image.width;
					target.height = image.height;
				}
				context.drawImage(image, 0, 0);
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

<div role="img" aria-label={label}>
	<canvas
		bind:this={canvas}
		data-preview-state={!enabled ? 'disabled' : hasFrame ? 'available' : 'waiting'}
	></canvas>
</div>

<style>
	div,
	canvas {
		display: block;
		width: 100%;
		height: 100%;
		object-fit: contain;
		background: black;
	}
</style>
