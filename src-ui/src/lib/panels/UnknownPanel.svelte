<script lang="ts">
	import type { DockPanelProps, DockPanelState } from "$lib/dockview/panel-types";

	const initialProps: DockPanelProps = $props();
	const api = initialProps.api;

	let panel = $state<DockPanelState>({
		panelId: initialProps.panelId,
		title: initialProps.title,
		params: initialProps.params
	});
	let publishedTitle = $state("");

	const dynamicTitle = $derived(`Unknown ${panel.title}`);

	$effect(() => {
		if (dynamicTitle === publishedTitle) {
			return;
		}

		api.setTitle(dynamicTitle);
		publishedTitle = dynamicTitle;
	});

	export function setDockPanelState(next: DockPanelState): void {
		panel = next;
	}
</script>

<section class="panel unknown">
	<h2>Unknown panel renderer</h2>
	<p>component: {panel.title}</p>
	<p>id: {panel.panelId}</p>
</section>

<style>
	.panel {
		display: grid;
		align-content: center;
		justify-items: start;
		gap: 0.4rem;
		inline-size: 100%;
		block-size: 100%;
		padding: 1rem;
		box-sizing: border-box;
		color: var(--gc-color-text);
		background:
			linear-gradient(150deg, rgba(255, 255, 255, 0.04), transparent 65%),
			var(--gc-color-panel-alt);
	}

	h2,
	p {
		margin: 0;
	}

	h2 {
		font-size: 0.95rem;
	}

	p {
		font-size: 0.82rem;
		opacity: 0.8;
	}
</style>
