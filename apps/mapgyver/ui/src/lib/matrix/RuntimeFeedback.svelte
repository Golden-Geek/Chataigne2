<script lang="ts">
	import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
	import type { MatrixRenderStatus } from '../preview/generated/MatrixRenderStatus';
	let {
		snapshot,
		observation
	}: { snapshot: MatrixViewportSnapshot; observation: MatrixRenderStatus | null } = $props();
	const columnName = (id: string | null) =>
		!id ? 'None' : (snapshot.columns.find((column) => column.id === id)?.label ?? id);
	const latest = $derived(
		observation?.desired.playback.request_id === snapshot.playback.request_id &&
			observation?.desired.revision === snapshot.revision
	);
	const state = $derived(
		observation?.error
			? 'Error'
			: !observation
				? 'Unobserved'
				: !latest
					? 'Pending'
					: observation.stage
	);
</script>

<details class="runtime-feedback">
	<summary aria-label="Matrix runtime details" title={`Matrix: ${state}`}>
		<span class="state" class:error={!!observation?.error} role="status">{state}</span>
	</summary>
	<dl aria-label="Matrix runtime feedback">
		<dt>Requested</dt>
		<dd>{columnName(snapshot.playback.desired_column)}</dd>
		<dt>Observed</dt>
		<dd>{latest ? 'Current' : 'Pending'}</dd>
		<dt>Renderer</dt>
		<dd>{observation?.stage ?? 'Unobserved'}</dd>
		<dt>Channel</dt>
		<dd>{observation?.availability ?? 'Unobserved'}</dd>
		<dt>Installed</dt>
		<dd>
			{observation?.installed
				? columnName(observation.installed.playback.desired_column)
				: 'Unobserved'}
		</dd>
		<dt>Submitted</dt>
		<dd>
			{observation?.submitted
				? `${columnName(observation.submitted.playback.desired_column)} · ${observation.submitted.transitioning ? 'Mix' : 'Steady'} · ${observation.submitted.columns.length} column(s)`
				: 'Unobserved'}
		</dd>
		{#if observation?.error}<dt>Error</dt>
			<dd role="alert">{observation.error}</dd>{/if}
	</dl>
</details>

<style>
	.runtime-feedback {
		position: relative;
		max-width: 100%;
		font-size: 0.8rem;
	}
	summary {
		cursor: pointer;
		white-space: nowrap;
	}
	.state.error {
		color: var(--gc-color-error);
	}
	dl {
		position: absolute;
		left: 0;
		top: 100%;
		z-index: 6;
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 0.35rem 0.6rem;
		width: min(24rem, 65vw);
		padding: 0.6rem;
		margin: 0.25rem 0 0;
		background: var(--gc-color-panel);
		border: 0.05rem solid var(--gc-color-border);
		color: var(--gc-color-text);
		overflow-wrap: anywhere;
	}
	dd {
		margin: 0;
	}
</style>
