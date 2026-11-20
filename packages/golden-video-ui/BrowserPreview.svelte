<script lang="ts">
	import { appState } from 'golden_ui';

	let { source, label }: { source: string; label: string } = $props();
	let canvas: HTMLCanvasElement | undefined = $state();
	let intersecting = $state(false);
	let pageVisible = $state(false);
	let hasFrame = $state(false);

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
		if (!target || !request || !intersecting || !pageVisible) return;
		const context = target.getContext('2d', { alpha: false });
		if (!context) return;
		const controller = new AbortController();
		let timer: ReturnType<typeof setTimeout> | undefined;
		let previous: string | null = null;

		const pull = async () => {
			const began = performance.now();
			let interval = 67;
			try {
				const frame = await request(key, previous, controller.signal);
				if (frame && !controller.signal.aborted) {
					const image = await createImageBitmap(frame.image);
					try {
						if (controller.signal.aborted) return;
						if (target.width !== image.width || target.height !== image.height) {
							target.width = image.width;
							target.height = image.height;
						}
						context.drawImage(image, 0, 0);
						previous = frame.tag;
						hasFrame = true;
					} finally {
						image.close();
					}
				}
			} catch {
				interval = 500;
			} finally {
				if (!controller.signal.aborted) {
					timer = setTimeout(pull, Math.max(0, interval - (performance.now() - began)));
				}
			}
		};
		void pull();
		return () => {
			controller.abort();
			clearTimeout(timer);
			context.clearRect(0, 0, target.width, target.height);
			hasFrame = false;
		};
	});
</script>

<div role="img" aria-label={label}>
	<canvas bind:this={canvas} data-preview-state={hasFrame ? 'available' : 'waiting'}></canvas>
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
