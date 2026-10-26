<script lang="ts">
	import { tick, untrack } from 'svelte';
	import Slider from 'golden_ui/components/common/Slider.svelte';
	import VirtualRows from 'golden_ui/components/common/virtual-rows/VirtualRows.svelte';
	import type { RelatedValue } from './types';
	import { relatedValueAmount } from './projection.svelte';
	let { rows, label, owner }: { rows: RelatedValue[]; label: string; owner: string } = $props();
	let scroller = $state<HTMLElement | null>(null);
	let virtual = $state<ReturnType<typeof VirtualRows<RelatedValue>>>();
	let query = $state('');
	let appliedOwner: string | undefined;
	let filtered = $derived(
		rows.filter((row) =>
			row.endpoint.node.meta.label.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())
		)
	);
	$effect.pre(() => {
		const nextOwner = owner;
		untrack(() => {
			if (nextOwner !== appliedOwner && scroller?.contains(document.activeElement))
				scroller.focus({ preventScroll: true });
			appliedOwner = nextOwner;
			query = '';
		});
	});
	export async function reveal(key: string) {
		if (!rows.some((row) => row.key === key)) return false;
		query = '';
		await tick();
		return virtual?.reveal(key, 'center') ?? false;
	}
</script>

<section class="related-values" aria-label="Computed values">
	<header><span>{label}</span><span>{rows.length}</span></header>
	<label class="filter">Find related value<input type="search" bind:value={query} /></label>
	<div
		class="related-value-list"
		bind:this={scroller}
		role="grid"
		aria-readonly="true"
		aria-rowcount={filtered.length}
		aria-colcount={2}
		tabindex="0"
		aria-label={label}>
		{#key owner}
			<VirtualRows
				bind:this={virtual}
				rows={filtered}
				keyOf={(row) => row.key}
				{scroller}
				rowHeightRem={3}>
				{#snippet item(row, index)}
					<div
						class="related-value-row"
						role="row"
						aria-rowindex={index + 1}
						class:disabled={!row.endpoint.enabled}
						data-related-key={row.key}
						data-endpoint-uuid={row.endpoint.node.uuid}
						data-value-node={row.valueParam?.node_id}>
						<div class="related-value-label" role="rowheader">
							<span class="swatch" style:background={row.endpoint.color}></span><span
								>{row.endpoint.node.meta.label}</span>
						</div>
						<div class="related-value-control" role="gridcell">
							{#if row.valueParam}
								<Slider
									value={relatedValueAmount(row.valueParam)}
									min={0}
									max={1}
									step={0.001}
									readOnly={true}
									disabled={!row.endpoint.enabled}
									fgColor={row.endpoint.color}
									showValue={true} />
							{:else}<span class="missing">-</span>{/if}
						</div>
					</div>
				{/snippet}
			</VirtualRows>
		{/key}
	</div>
</section>

<style>
	.related-values {
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		gap: 0.45rem;
		min-block-size: 0;
	}
	header {
		display: flex;
		justify-content: space-between;
		gap: 0.6rem;
		font-size: 0.72rem;
		font-weight: 650;
	}
	.filter {
		display: grid;
		gap: 0.25rem;
		font-size: 0.72rem;
	}
	input {
		min-inline-size: 0;
		inline-size: 100%;
		box-sizing: border-box;
		font: inherit;
	}
	.related-value-list {
		flex: 1 1 auto;
		min-block-size: 0;
		block-size: min(60vh, 72rem);
		max-block-size: 72rem;
		overflow: auto;
	}
	.related-value-row {
		display: grid;
		gap: 0.18rem;
		inline-size: 100%;
		padding: 0.18rem 0;
		font-size: 0.68rem;
	}
	.disabled {
		opacity: 0.48;
	}
	.related-value-label {
		display: inline-grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: 0.38rem;
		min-inline-size: 0;
		font-weight: 550;
	}
	.related-value-label span:last-child {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.swatch {
		inline-size: 0.58rem;
		block-size: 0.58rem;
		border-radius: 50%;
	}
	.related-value-control {
		inline-size: 100%;
		min-inline-size: 0;
		block-size: 1rem;
		overflow: hidden;
	}
	.related-value-control :global(.slider) {
		block-size: 100%;
		border-radius: 0.32rem;
	}
	.related-value-control :global(.slider-label) {
		font-size: 0.62rem;
		font-weight: 650;
	}
	.missing {
		display: block;
		text-align: end;
		font-variant-numeric: tabular-nums;
	}
</style>
