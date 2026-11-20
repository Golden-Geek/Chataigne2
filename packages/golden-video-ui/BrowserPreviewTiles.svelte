<script lang="ts">
	import { appState, type UiBrowserPreview, type UiBrowserPreviewExtent } from 'golden_ui';
	import { onDestroy, untrack } from 'svelte';
	import { useBrowserPreviews } from './browser-preview.svelte';
	import { observeBrowserPreview } from './browser-visibility';
	import type { PreviewTileGroup } from './generated/PreviewTileGroup';

	let {
		groups,
		label,
		visible = true,
		lifetime = 0
	}: {
		groups: PreviewTileGroup[];
		label: string;
		visible?: boolean;
		lifetime?: string | number;
	} = $props();
	const MAX_CANVAS_EDGE = 1280;
	const session = useBrowserPreviews();
	const enabled = $derived(session?.mode !== 'disabled');
	const sources = $derived(JSON.stringify(groups.map((group) => group.source)));
	let canvas: HTMLCanvasElement | undefined = $state();
	let shown = $state(false);
	let painted = $state(0);
	let pending: number | null = null;
	let boundRequest: UiBrowserPreview | undefined;
	let boundLifetime: string | number | undefined;
	const active = new Map<string, () => void>();
	const frames = new Map<string, { image: HTMLCanvasElement; extent: UiBrowserPreviewExtent }>();

	function queuePaint() {
		if (pending !== null || !canvas) return;
		pending = requestAnimationFrame(paint);
	}

	function paint() {
		pending = null;
		const target = canvas;
		const context = target?.getContext('2d');
		if (!target || !context) return;
		const { clientWidth: width, clientHeight: height } = target;
		if (!width || !height) return;
		const scale = Math.min(devicePixelRatio, MAX_CANVAS_EDGE / Math.max(width, height));
		const pixelsWide = Math.max(1, Math.round(width * scale));
		const pixelsHigh = Math.max(1, Math.round(height * scale));
		if (target.width !== pixelsWide || target.height !== pixelsHigh) {
			target.width = pixelsWide;
			target.height = pixelsHigh;
		}
		context.setTransform(pixelsWide / width, 0, 0, pixelsHigh / height, 0, 0);
		context.clearRect(0, 0, width, height);
		let count = 0;
		if (enabled && visible && shown) {
			context.fillStyle = '#000';
			for (const group of groups) {
				const frame = frames.get(group.source);
				for (const tile of group.tiles) {
					context.fillRect(tile.x, tile.y, tile.width, tile.height);
					if (!frame) continue;
					const aspect = frame.extent.width / frame.extent.height;
					const w =
						group.fit === 'Contain' ? Math.min(tile.width, tile.height * aspect) : tile.width;
					const h =
						group.fit === 'Contain' ? Math.min(tile.height, tile.width / aspect) : tile.height;
					context.drawImage(
						frame.image,
						tile.x + (tile.width - w) / 2,
						tile.y + (tile.height - h) / 2,
						w,
						h
					);
					count++;
				}
			}
		}
		painted = count;
	}

	function clearSources() {
		for (const release of active.values()) release();
		active.clear();
		frames.clear();
	}

	$effect(() => {
		const target = canvas;
		if (!target) return;
		const stop = observeBrowserPreview(target, (value) => (shown = value));
		const resize = new ResizeObserver(queuePaint);
		resize.observe(target);
		return () => {
			stop();
			resize.disconnect();
		};
	});

	$effect(() => {
		groups;
		untrack(queuePaint);
	});

	$effect(() => {
		const target = canvas;
		const request = appState.session?.client.browserPreview;
		const owner = lifetime;
		const current = sources;
		const admitted = enabled && visible && shown;
		untrack(() => {
			if (!target || !request || !session || !admitted) {
				clearSources();
				queuePaint();
				return;
			}
			if (request !== boundRequest || owner !== boundLifetime) clearSources();
			boundRequest = request;
			boundLifetime = owner;
			const next = new Set(JSON.parse(current) as string[]);
			for (const [source, release] of active) {
				if (next.has(source)) continue;
				release();
				active.delete(source);
				frames.delete(source);
			}
			for (const source of next) {
				if (active.has(source)) continue;
				active.set(
					source,
					session.register(source, request, (bitmap, extent) => {
						const image = frames.get(source)?.image ?? target.ownerDocument.createElement('canvas');
						if (image.width !== bitmap.width || image.height !== bitmap.height) {
							image.width = bitmap.width;
							image.height = bitmap.height;
						}
						image.getContext('2d', { alpha: false })?.drawImage(bitmap, 0, 0);
						frames.set(source, { image, extent });
						queuePaint();
					})
				);
			}
			queuePaint();
		});
	});

	onDestroy(() => {
		clearSources();
		if (pending !== null) cancelAnimationFrame(pending);
	});
</script>

<canvas bind:this={canvas} aria-label={label} data-browser-preview-tiles={painted}></canvas>

<style>
	canvas {
		display: block;
		width: 100%;
		height: 100%;
		pointer-events: none;
	}
</style>
