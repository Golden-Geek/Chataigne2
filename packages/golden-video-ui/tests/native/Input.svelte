<script lang="ts">
	import { onDestroy } from 'svelte';
	import { NativePreview, createPreviewSession, capturePreviewPointer } from '../../index';
	let canvas: HTMLDivElement;
	let image: HTMLDivElement;
	let source = $state('original');
	let mounted = $state(true);
	let editing = false;
	const trace: { kind: string; x?: number; y?: number }[] = [];
	let activeQueries = 0;
	const report = {
		trace,
		queries: 0,
		maximumQueries: 0,
		delay: 30,
		changeSource() {
			source = 'replacement';
		},
		remove() {
			mounted = false;
		}
	};
	Object.assign(window, { __nativeInput: report });
	const session = createPreviewSession({
		available: () => true,
		async request(request) {
			if (request.kind === 'Connect')
				return {
					kind: 'Connected',
					session: crypto.randomUUID(),
					version: request.version,
					max_views: 64
				};
			if (request.kind === 'Layout' || request.kind === 'Status')
				return { kind: 'Applied', revision: request.revision, visible_views: 1, installed: true };
			if (request.kind === 'Pointer') {
				report.queries++;
				activeQueries++;
				report.maximumQueries = Math.max(report.maximumQueries, activeQueries);
				const point: [number, number] = [
					(request.point[0] - 50) / 400,
					(request.point[1] - 100) / 240
				];
				const hit =
					Math.abs(point[0] - 0.2) < 0.05 && Math.abs(point[1] - 0.5) < 0.1
						? 'left'
						: Math.abs(point[0] - 0.8) < 0.05 && Math.abs(point[1] - 0.5) < 0.1
							? 'right'
							: null;
				await new Promise((resolve) => setTimeout(resolve, report.delay));
				activeQueries--;
				return {
					kind: 'Pointer',
					sample: { point, hit, rect: { x: 50, y: 100, width: 400, height: 240 } }
				};
			}
			return { kind: 'Disconnected' };
		}
	});
	void session.connect();
	onDestroy(() => {
		void session.dispose();
	});
	function point(event: PointerEvent) {
		const rect = image.getBoundingClientRect();
		return { x: (event.clientX - rect.x) / rect.width, y: (event.clientY - rect.y) / rect.height };
	}
	function down(event: PointerEvent) {
		editing = true;
		trace.push({ kind: 'down', ...point(event) });
		capturePreviewPointer(canvas, event);
	}
</script>

<!-- Deliberately offset the DOM image by 250 CSS pixels from the accepted native frame. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="viewport"
	data-native-preview-backdrop
	bind:this={canvas}
	onpointermove={(event) => {
		if (editing) trace.push({ kind: 'move', ...point(event) });
	}}
	onpointerup={(event) => {
		if (editing) trace.push({ kind: 'up', ...point(event) });
		editing = false;
	}}
	onpointercancel={() => {
		if (editing) trace.push({ kind: 'cancel' });
		editing = false;
	}}
>
	<div class="image" bind:this={image}>
		{#if mounted}
			<NativePreview
				{session}
				{source}
				nativeInput
				overlayCapacity={1}
				overlays={[
					{
						hit: 'left',
						shape: { kind: 'Circle', center: [0.2, 0.5], radius: 6 },
						fill: [1, 1, 1, 1],
						stroke: [0, 0, 1, 1],
						width: 1,
						dash: null
					}
				]}
			>
				<button
					data-native-hit-id="left"
					data-native-overlay-control
					style:left="20%"
					onpointerdown={down}
					onclick={() => trace.push({ kind: 'left' })}>Left</button
				>
				<button
					data-native-hit-id="right"
					data-native-overlay-control
					style:left="80%"
					onpointerdown={down}
					onclick={() => trace.push({ kind: 'right' })}>Right</button
				>
			</NativePreview>
		{/if}
	</div>
</div>

<style>
	.viewport {
		position: relative;
		width: 50rem;
		height: 30rem;
	}
	.image {
		position: absolute;
		left: 18.75rem;
		top: 6.25rem;
		width: 25rem;
		height: 15rem;
	}
	button {
		position: absolute;
		top: 50%;
		pointer-events: auto;
	}
</style>
