<script lang="ts">
	import type { FormulaPreviewLaneOption } from '../preview/formulaPreviewSessionStore.svelte';

	let {
		lanes,
		selectedLaneId,
		onSelect
	}: {
		lanes: readonly FormulaPreviewLaneOption[];
		selectedLaneId: string | null;
		onSelect: (laneId: string) => void;
	} = $props();
</script>

{#if lanes.length > 1}
	<label class="lane-selector">
		<span>Lane</span>
		<select
			value={selectedLaneId ?? lanes[0]?.id ?? ''}
			onchange={(event) => onSelect(event.currentTarget.value)}>
			{#each lanes as lane}
				<option value={lane.id}>
					{lane.label}{lane.diagnosticsCount > 0 ? ` (${lane.diagnosticsCount})` : ''}
				</option>
			{/each}
		</select>
	</label>
{/if}

<style>
	.lane-selector {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		min-block-size: 1.45rem;
		color: color-mix(in srgb, var(--gc-color-text) 72%, transparent);
		font-size: 0.68rem;
		font-weight: 650;
		letter-spacing: 0;
	}

	.lane-selector select {
		max-inline-size: 10rem;
		min-inline-size: 6rem;
		block-size: 1.45rem;
		padding: 0 1.5rem 0 0.45rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 80%, transparent);
		border-radius: 0.35rem;
		background: var(--gc-color-background);
		color: var(--gc-color-text);
		font: inherit;
		font-size: 0.68rem;
	}
</style>
