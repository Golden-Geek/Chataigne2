<script lang="ts">
	import { onMount } from 'svelte';
	import type { UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import {
		processorRuntimeOverview,
		publishProcessorOverviewDemand,
		publishProcessorOverviewLaneSelection
	} from '../../state_machine/processorOverview.svelte';

	const PROCESSOR_OVERVIEW_HEARTBEAT_MS = 2000;
	const subscriptionId = `processor-preview-selector:${Date.now().toString(36)}:${Math.random().toString(36).slice(2, 10)}`;

	let { node }: { node: UiNodeDto } = $props();

	let session = $derived(appState.session);
	let overview = $derived(processorRuntimeOverview(node.uuid));

	const publishDemand = (active: boolean): void => {
		publishProcessorOverviewDemand(subscriptionId, active ? [node.uuid] : []);
	};

	const selectIndex = (value: string): void => {
		const laneCount = overview?.multiplex_lane_count ?? 0;
		const parsed = Number(value);
		if (!Number.isFinite(parsed) || laneCount === 0) return;
		const index = Math.min(laneCount, Math.max(1, Math.trunc(parsed)));
		publishProcessorOverviewLaneSelection(node, index);
	};

	$effect(() => {
		void session?.graphTransitionRevision;
		node.uuid;
		publishDemand(true);
	});

	onMount(() => {
		const heartbeat = setInterval(() => publishDemand(true), PROCESSOR_OVERVIEW_HEARTBEAT_MS);
		return () => {
			clearInterval(heartbeat);
			publishDemand(false);
		};
	});
</script>

{#if overview && overview.multiplex_lane_count > 1}
	<span
		class="processor-preview-lane-selector"
		role="presentation"
		onclick={(event) => event.stopPropagation()}
		onkeydown={(event) => event.stopPropagation()}>
		<label title={`Preview lane: ${overview.preview_lane_label}`}>
			<span>Preview</span>
			<input
				type="number"
				min="1"
				max={overview.multiplex_lane_count}
				step="1"
				value={overview.preview_index}
				aria-label="Processor preview index"
				onchange={(event) => selectIndex(event.currentTarget.value)} />
		</label>
		{#if overview.preview_overridden}
			<button
				type="button"
				class="use-default"
				title={`Use multiplex default preview index ${overview.default_preview_index}`}
				onclick={() => publishProcessorOverviewLaneSelection(node, null)}>
				Default {overview.default_preview_index}
			</button>
		{:else}
			<span
				class="default-indicator"
				title={`Using multiplex default preview index ${overview.default_preview_index}`}>
				Default
			</span>
		{/if}
	</span>
{/if}

<style>
	.processor-preview-lane-selector,
	.processor-preview-lane-selector label {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		min-inline-size: 0;
	}

	.processor-preview-lane-selector {
		color: color-mix(in srgb, var(--gc-color-text) 72%, transparent);
		font-size: 0.68rem;
		font-weight: 650;
	}

	.processor-preview-lane-selector input {
		inline-size: 3.2rem;
		block-size: 1.45rem;
		padding: 0 0.25rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 80%, transparent);
		border-radius: 0.35rem;
		background: var(--gc-color-background);
		color: var(--gc-color-text);
		font: inherit;
	}

	.use-default,
	.default-indicator {
		padding: 0.1rem 0.3rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 70%, transparent);
		border-radius: 0.3rem;
		font: inherit;
		white-space: nowrap;
	}

	.use-default {
		background: transparent;
		color: var(--gc-color-text);
		cursor: pointer;
	}

	.use-default:hover {
		background: color-mix(in srgb, var(--gc-color-accent) 16%, transparent);
	}

	.default-indicator {
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
	}
</style>
