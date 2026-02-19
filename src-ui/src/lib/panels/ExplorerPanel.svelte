<script lang="ts">
	import type { DockPanelProps, DockPanelState } from "$lib/dockview/panel-types";

	const initialProps: DockPanelProps = $props();
	const api = initialProps.api;

	let panel = $state<DockPanelState>({
		panelId: initialProps.panelId,
		title: initialProps.title,
		params: initialProps.params
	});
	let filter = $state("");
	let publishedTitle = $state("");

	const defaultNodes = [
		"project/",
		"sequences/",
		"midi/",
		"audio/",
		"presets/"
	];
	const nodes = $derived((panel.params.nodes as string[] | undefined) ?? defaultNodes);
	const filteredNodes = $derived(
		nodes.filter((node) =>
			node.toLocaleLowerCase().includes(filter.trim().toLocaleLowerCase())
		)
	);
	const dynamicTitle = $derived(
		filter.trim().length > 0
			? `${panel.title} ${filteredNodes.length}/${nodes.length}`
			: `${panel.title} ${nodes.length}`
	);

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

<section class="panel explorer">
	<header class="panel-header">
		<h2>{dynamicTitle}</h2>
		<p>{panel.panelId}</p>
	</header>

	<label class="panel-filter">
		<span>Filter</span>
		<input bind:value={filter} type="text" placeholder="Search nodes..." />
	</label>

	<ul class="tree">
		{#each filteredNodes as node}
			<li>{node}</li>
		{/each}
	</ul>

	<footer class="panel-footer">
		<span>{filteredNodes.length} visible</span>
	</footer>
</section>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		inline-size: 100%;
		block-size: 100%;
		padding: 3rem;
		box-sizing: border-box;

	}

	.panel-header h2 {
		margin: 0;
		font-size: 0.95rem;
		line-height: 1.2;
	}

	.panel-header p {
		margin: 0.3rem 0 0;
		font-size: 0.7rem;
		opacity: 0.72;
	}

	.panel-filter {
		display: grid;
		gap: 0.35rem;
	}

	.panel-filter span {
		font-size: 0.72rem;
		opacity: 0.75;
	}

	.panel-filter input {
		inline-size: 100%;
		padding: 0.45rem 0.6rem;
		border: 0.0625rem solid var(--gc-color-panel-outline);
		border-radius: 0.35rem;
		outline: none;
		color: inherit;
		background: color-mix(in srgb, var(--gc-color-panel) 88%, black);
	}

	.panel-filter input:focus {
		border-color: var(--gc-color-focus);
	}

	.tree {
		margin: 0;
		padding: 0;
		list-style: none;
		display: grid;
		gap: 0.4rem;
		font-size: 0.82rem;
		overflow: auto;
	}

	.tree li {
		padding: 0.45rem 0.6rem;
		border-radius: 0.35rem;
		background:
			linear-gradient(90deg, rgba(255, 255, 255, 0.06), transparent 70%),
			var(--gc-color-panel-row);
	}

	.panel-footer {
		margin-top: auto;
		font-size: 0.72rem;
		opacity: 0.72;
	}
</style>
