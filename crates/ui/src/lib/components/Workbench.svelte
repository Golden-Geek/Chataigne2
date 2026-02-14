<script lang="ts">
	import Inspector from './Inspector.svelte';
	import NodeTree from './NodeTree.svelte';
	import type { GraphState } from '../store/graph';
	import type { NodeId, UiEditIntent } from '../types';

	let {
		state,
		status = '',
		onSelect,
		onIntent
	}: {
		state: GraphState;
		status?: string;
		onSelect: (nodeId: NodeId) => void;
		onIntent: (intent: UiEditIntent) => void;
	} = $props();
</script>

<main class="workbench">
	<header class="topbar">
		<div>
			<p class="eyebrow">Golden Core</p>
			<h1>UI Base</h1>
		</div>
		{#if status}
			<p class="status">{status}</p>
		{/if}
	</header>

	<div class="grid">
		<NodeTree
			{state}
			selectedNodeId={state.selectedNodeId}
			onSelect={(nodeId) => onSelect(nodeId)}
		/>
		<Inspector {state} {onIntent} />
	</div>
</main>

<style>
	:global(:root) {
		--accent: #ff6e2a;
		--bg-a: #121517;
		--bg-b: #1f2f36;
		--fg: #f0ece4;
		--panel-bg: #1b1f22;
		--panel-border: #2e3a42;
	}

	.workbench {
		min-height: 100dvh;
		padding: 1rem;
		color: var(--fg);
		background:
			radial-gradient(1200px 600px at 10% -10%, color-mix(in srgb, var(--accent) 22%, transparent), transparent),
			linear-gradient(145deg, var(--bg-a), var(--bg-b));
		font-family: 'Space Grotesk', 'Avenir Next', 'Segoe UI', sans-serif;
	}

	.topbar {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.6rem 0.2rem 1rem;
	}

	.eyebrow {
		margin: 0;
		text-transform: uppercase;
		letter-spacing: 0.12em;
		font-size: 0.72rem;
		opacity: 0.75;
	}

	h1 {
		margin: 0.1rem 0 0;
		font-size: clamp(1.5rem, 1.1rem + 2vw, 2.1rem);
	}

	.status {
		margin: 0;
		font-size: 0.82rem;
		opacity: 0.9;
		max-width: min(50ch, 50vw);
		text-align: right;
	}

	.grid {
		display: grid;
		grid-template-columns: minmax(280px, 1.2fr) minmax(280px, 1fr);
		gap: 0.9rem;
	}

	@media (max-width: 860px) {
		.grid {
			grid-template-columns: 1fr;
		}

		.status {
			max-width: none;
			text-align: left;
		}

		.topbar {
			flex-direction: column;
			align-items: flex-start;
		}
	}
</style>
