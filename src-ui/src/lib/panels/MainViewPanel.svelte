<script lang="ts">
	import type { DockPanelProps, DockPanelState } from "$lib/dockview/panel-types";

	const initialProps: DockPanelProps = $props();
	const api = initialProps.api;

	let panel = $state<DockPanelState>({
		panelId: initialProps.panelId,
		title: initialProps.title,
		params: initialProps.params
	});
	let mode = $state("graph");
	let nodeCount = $state(3);
	let publishedTitle = $state("");

	const availableModes = ["graph", "timeline", "mix"];

	const normalizeCount = (value: unknown): number => {
		if (typeof value !== "number" || !Number.isFinite(value)) {
			return 3;
		}

		return Math.max(1, Math.round(value));
	};

	const applyParams = (nextParams: Record<string, unknown>): void => {
		const nextMode = nextParams.mode;
		if (typeof nextMode === "string" && availableModes.includes(nextMode)) {
			mode = nextMode;
		}

		nodeCount = normalizeCount(nextParams.nodeCount);
	};

	applyParams(initialProps.params);

	const dynamicTitle = $derived(`${panel.title} ${mode} (${nodeCount})`);
	const nodes = $derived(
		Array.from({ length: nodeCount }, (_, index) => `${mode}-node-${index + 1}`)
	);

	const setMode = (nextMode: string): void => {
		mode = nextMode;
	};

	const addNode = (): void => {
		nodeCount += 1;
	};

	const removeNode = (): void => {
		nodeCount = Math.max(1, nodeCount - 1);
	};

	$effect(() => {
		if (dynamicTitle === publishedTitle) {
			return;
		}

		api.setTitle(dynamicTitle);
		publishedTitle = dynamicTitle;
	});

	export function setDockPanelState(next: DockPanelState): void {
		panel = next;
		applyParams(next.params);
	}
</script>

<section class="panel main-view">
	<header class="panel-header">
		<h2>{dynamicTitle}</h2>
		<p>{panel.panelId}</p>
	</header>

	<div class="toolbar">
		<div class="mode-switch">
			{#each availableModes as candidate}
				<button
					type="button"
					class:active={candidate === mode}
					onclick={() => setMode(candidate)}
				>
					{candidate}
				</button>
			{/each}
		</div>
		<div class="count-switch">
			<button type="button" onclick={removeNode}>-</button>
			<span>{nodeCount} nodes</span>
			<button type="button" onclick={addNode}>+</button>
		</div>
	</div>

	<div class="viewport">
		{#each nodes as node}
			<div class="node">{node}</div>
		{/each}
	</div>
</section>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 1rem;
		box-sizing: border-box;
		color: var(--gc-color-text);
		background:
			linear-gradient(145deg, rgba(255, 255, 255, 0.04), transparent 65%),
			var(--gc-color-panel);
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

	.toolbar {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		justify-content: space-between;
		align-items: center;
	}

	.mode-switch {
		display: flex;
		gap: 0.3rem;
	}

	.mode-switch button,
	.count-switch button {
		padding: 0.3rem 0.5rem;
		border: 0.0625rem solid var(--gc-color-panel-outline);
		border-radius: 0.35rem;
		background: var(--gc-color-panel-row);
		color: inherit;
		text-transform: capitalize;
		cursor: pointer;
	}

	.mode-switch button.active {
		background: var(--gc-color-focus-muted);
		border-color: var(--gc-color-focus);
	}

	.count-switch {
		display: flex;
		align-items: center;
		gap: 0.45rem;
		font-size: 0.78rem;
	}

	.viewport {
		flex: 1;
		min-block-size: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(7rem, 1fr));
		gap: 0.6rem;
		padding: 0.65rem;
		border-radius: 0.5rem;
		overflow: auto;
		background:
			radial-gradient(circle at 20% 10%, rgba(255, 255, 255, 0.08), transparent 60%),
			radial-gradient(circle at 80% 90%, rgba(255, 255, 255, 0.06), transparent 60%),
			var(--gc-color-panel-alt);
	}

	.node {
		padding: 0.6rem 0.5rem;
		border-radius: 0.4rem;
		text-align: center;
		font-size: 0.78rem;
		background:
			linear-gradient(90deg, rgba(255, 255, 255, 0.07), transparent 80%),
			var(--gc-color-panel-row);
	}
</style>
