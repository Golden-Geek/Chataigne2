<script lang="ts">
	import type { SoundCardRouteRow } from './sound-card-editor-model';

	let {
		rows,
		active = true,
		emptyLabel = 'No routes are authored.',
		inactiveLabel = 'Input is disabled. Authored routes remain stored but carry no live signal.'
	} = $props<{
		rows: readonly SoundCardRouteRow[];
		active?: boolean;
		emptyLabel?: string;
		inactiveLabel?: string;
	}>();
</script>

<div class="route-table" class:inactive={!active}>
	{#if !active}
		<p class="inactive-notice" role="status">{inactiveLabel}</p>
	{/if}
	{#if rows.length > 0}
		<table>
			<thead>
				<tr>
					<th scope="col">Source</th>
					<th scope="col">Destination</th>
					<th scope="col">Gain</th>
				</tr>
			</thead>
			<tbody>
				{#each rows as row (row.id)}
					<tr title={row.label}>
						<td>{row.source}</td>
						<td>{row.destination}</td>
						<td>{row.gainDb === null ? '—' : `${row.gainDb.toFixed(1)} dB`}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{:else}
		<p class="empty">{emptyLabel}</p>
	{/if}
</div>

<style>
	.route-table {
		max-block-size: 18rem;
		overflow: auto;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.35rem;
	}

	table {
		inline-size: 100%;
		border-collapse: collapse;
		font-size: 0.76rem;
	}

	th,
	td {
		padding: 0.42rem 0.55rem;
		border-block-end: 0.0625rem solid var(--gc-color-border);
		text-align: start;
		overflow-wrap: anywhere;
	}

	th {
		position: sticky;
		inset-block-start: 0;
		z-index: 1;
		background: var(--gc-color-bg-lighter);
		color: var(--gc-color-text-muted);
		font-weight: 600;
	}

	tbody tr {
		content-visibility: auto;
		contain-intrinsic-block-size: 2.2rem;
	}

	tbody tr:last-child td {
		border-block-end: none;
	}

	.inactive table {
		opacity: 0.62;
	}

	.inactive-notice,
	.empty {
		margin: 0;
		padding: 0.6rem;
		color: var(--gc-color-text-muted);
		font-size: 0.76rem;
	}

	.inactive-notice {
		border-block-end: 0.0625rem solid var(--gc-color-border);
		color: #e9bd70;
	}
</style>
