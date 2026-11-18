<script lang="ts">
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	import { rowOffset, type projectGrid } from './viewport';
	import RowParameter from './RowParameter.svelte';
	let {
		rows,
		projection,
		height
	}: {
		rows: MatrixViewRow[];
		projection: ReturnType<typeof projectGrid>;
		height: number;
	} = $props();
	const bodyHeight = $derived(Math.max(0, height - projection.header));
</script>

<div class="opacity-rail" role="group" aria-label="Layer opacity">
	<div
		class="opacity-strip"
		style:top={`${projection.header}rem`}
		style:height={`${bodyHeight}rem`}>
		{#each rows as row (row.item.id.id)}
			{@const top =
				rowOffset(row, projection) + projection.rowOrigin - projection.top - projection.header}
			{#if row.item.id.kind === 'Layer' && top < bodyHeight && top + projection.row > 0}
				<div
					class="opacity-lane"
					data-opacity-layer={row.item.id.id}
					style:top={`${top + 0.3}rem`}
					style:height={`${projection.row - 0.6}rem`}>
					<RowParameter uuid={row.item.id.id} label={row.item.label} field="opacity" />
				</div>
			{/if}
		{/each}
	</div>
</div>

<style>
	.opacity-rail {
		position: relative;
		min-height: 0;
		background: var(--gc-color-panel);
		border-inline-start: 0.05rem solid var(--gc-color-border);
	}
	.opacity-strip {
		position: absolute;
		inset-inline: 0;
		overflow: clip;
	}
	.opacity-lane {
		position: absolute;
		inset-inline: 0.35rem;
	}
</style>
