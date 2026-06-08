<script lang="ts">
	import { onMount } from 'svelte';
	import {
		GraphCanvas,
		type GraphConnectionRequest,
		type GraphNodePosition
	} from 'golden_alchemist_ui';
	import type { PanelProps, PanelState } from 'golden_ui';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import { graphEdgesFor, graphNodesFor } from '../graph-view-model';
	import { stateMachineStores } from '../stores/stateMachineStores.svelte';

	let props: PanelProps = $props();
	let updatedPanelState = $state<PanelState | null>(null);
	let panelState = $derived(
		updatedPanelState ?? {
			panelId: props.panelId,
			panelType: props.panelType,
			title: props.title,
			params: props.params
		}
	);
	let panelRoot: HTMLElement | null = $state(null);
	let graphCanvas: {
		frameSelection: () => boolean;
		home: () => boolean;
		focus: () => void;
	} | null = $state(null);
	let showDetails = $state(true);

	const stores = stateMachineStores;
	let graphNodes = $derived(graphNodesFor(stores));
	let graphEdges = $derived(graphEdgesFor(stores));
	let selectedNodeIds = $derived([...stores.graph.selectedNodeIds]);
	let selectedNode = $derived(
		selectedNodeIds.length === 1 ? (stores.graph.nodesById.get(selectedNodeIds[0]) ?? null) : null
	);
	let states = $derived([...stores.statechart.statesById.values()]);
	let processors = $derived([...stores.processors.processorsById.values()]);
	let diagnostics = $derived([...stores.processors.diagnosticsById.values()]);

	const panelOwnsFocus = (): boolean =>
		panelRoot !== null &&
		document.activeElement !== null &&
		panelRoot.contains(document.activeElement);

	const moveNode = (nodeId: string, position: GraphNodePosition): void => {
		stores.graph.moveNode(nodeId, position);
	};

	const connect = (connection: GraphConnectionRequest): void => {
		stores.graph.connect(connection);
	};

	export const setPanelState = (next: PanelState): void => {
		updatedPanelState = next;
	};

	onMount(() => {
		const unregisterFrame = registerCommandHandler(
			'view.frame',
			() => (panelOwnsFocus() ? (graphCanvas?.frameSelection() ?? false) : false),
			{ priority: 100 }
		);
		const unregisterHome = registerCommandHandler(
			'view.home',
			() => (panelOwnsFocus() ? (graphCanvas?.home() ?? false) : false),
			{ priority: 100 }
		);
		return () => {
			unregisterFrame();
			unregisterHome();
		};
	});
</script>

<section
	bind:this={panelRoot}
	class="state-machine-panel"
	aria-label={panelState.title}
	onpointerdown={() => graphCanvas?.focus()}>
	<header class="panel-toolbar">
		<div>
			<strong>State Machine</strong>
			<span>{graphNodes.length} nodes · {graphEdges.length} connections</span>
		</div>
		<button type="button" class:active={showDetails} onclick={() => (showDetails = !showDetails)}>
			Details
		</button>
	</header>

	<div class="workspace" class:with-details={showDetails}>
		<GraphCanvas
			bind:this={graphCanvas}
			nodes={graphNodes}
			edges={graphEdges}
			{selectedNodeIds}
			onSelectionChange={(nodeIds) => stores.graph.select(nodeIds)}
			onNodeMove={moveNode}
			onConnect={connect}
			emptyLabel="No processor graph is loaded for this state machine." />

		{#if showDetails}
			<aside>
				<section>
					<h2>Selection</h2>
					{#if selectedNode}
						<strong>{selectedNode.label}</strong>
						<code>{selectedNode.type_id}</code>
					{:else}
						<p>Select a graph node.</p>
					{/if}
				</section>

				<section>
					<h2>States</h2>
					{#if states.length === 0}
						<p>No states loaded.</p>
					{:else}
						{#each states as state (state.id)}
							<button
								type="button"
								class:active={state.active}
								onclick={() => stores.statechart.select(state.id)}>
								{state.label}
							</button>
						{/each}
					{/if}
				</section>

				<section>
					<h2>Processors</h2>
					{#if processors.length === 0}
						<p>No processors loaded.</p>
					{:else}
						{#each processors as processor (processor.id)}
							<button
								type="button"
								class:active={processor.active}
								onclick={() => stores.processors.select(processor.id)}>
								{processor.label}
							</button>
						{/each}
					{/if}
				</section>

				{#if diagnostics.length > 0}
					<section>
						<h2>Diagnostics</h2>
						{#each diagnostics as diagnostic (diagnostic.id)}
							<p class="diagnostic {diagnostic.severity}">{diagnostic.message}</p>
						{/each}
					</section>
				{/if}
			</aside>
		{/if}
	</div>
</section>

<style>
	.state-machine-panel {
		display: flex;
		flex-direction: column;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		color: var(--gc-color-text);
		background: var(--gc-color-background);
	}

	.panel-toolbar {
		z-index: 2;
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		min-block-size: 2.7rem;
		padding: 0.4rem 0.65rem;
		border-block-end: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.55);
		background: rgb(from var(--gc-color-background) r g b / 0.92);
	}

	.panel-toolbar > div {
		display: flex;
		align-items: baseline;
		gap: 0.7rem;
		min-inline-size: 0;
	}

	.panel-toolbar span {
		font-size: 0.68rem;
		opacity: 0.58;
	}

	.panel-toolbar button,
	aside button {
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.55);
		border-radius: 0.4rem;
		background: rgb(from var(--gc-color-background) r g b / 0.45);
		color: inherit;
		font: inherit;
		cursor: pointer;
	}

	.panel-toolbar button {
		padding: 0.32rem 0.65rem;
		font-size: 0.7rem;
	}

	.panel-toolbar button.active,
	aside button.active {
		border-color: var(--gc-color-selection);
		background: rgb(from var(--gc-color-selection) r g b / 0.2);
	}

	.workspace {
		position: relative;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		flex: 1 1 auto;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.workspace.with-details {
		grid-template-columns: minmax(0, 1fr) minmax(12rem, 18rem);
	}

	aside {
		z-index: 1;
		display: flex;
		flex-direction: column;
		gap: 0.85rem;
		min-block-size: 0;
		padding: 0.75rem;
		border-inline-start: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.55);
		background: rgb(from var(--gc-color-background) r g b / 0.94);
		overflow: auto;
	}

	aside section {
		display: flex;
		flex-direction: column;
		gap: 0.38rem;
	}

	aside h2 {
		margin: 0;
		font-size: 0.7rem;
		font-weight: 650;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		opacity: 0.62;
	}

	aside p {
		margin: 0;
		font-size: 0.72rem;
		opacity: 0.65;
	}

	aside code {
		font-size: 0.68rem;
		opacity: 0.62;
	}

	aside button {
		padding: 0.42rem 0.55rem;
		text-align: start;
		font-size: 0.72rem;
	}

	.diagnostic {
		padding: 0.45rem;
		border-inline-start: solid 0.16rem var(--gc-color-panel-outline);
	}

	.diagnostic.warning {
		border-color: #d8a84e;
	}

	.diagnostic.error {
		border-color: #d75b5b;
	}
</style>
