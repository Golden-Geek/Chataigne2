<script lang="ts">
	import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
	import type { MatrixRenderStatus } from '../preview/generated/MatrixRenderStatus';
	let {
		snapshot,
		observation
	}: { snapshot: MatrixViewportSnapshot; observation: MatrixRenderStatus | null } = $props();
	const columnName = (id: string | null) =>
		!id
			? 'None'
			: (snapshot.columns.find((column) => column.id === id)?.label ?? 'Outside visible columns');
	const latest = $derived(
		observation?.desired.playback.launch_id === snapshot.playback.launch_id &&
			observation?.desired.revision === snapshot.revision
	);
</script>

<section aria-label="Matrix runtime feedback">
	<h3>Runtime</h3>
	<p>Requested column: {columnName(snapshot.playback.desired_column)}</p>
	{#if observation}
		<p role="status">Renderer: {observation.stage} · Matrix channel: {observation.availability}</p>
		{#if !latest}<p>Waiting for the renderer to observe the current launch or edit.</p>{/if}
		<p>
			Installed column: {observation.installed
				? columnName(observation.installed.playback.desired_column)
				: 'No installation observed'}
		</p>
		{#if observation.submitted}
			<p>
				Frame submitted for {columnName(observation.submitted.playback.desired_column)}.
				{observation.submitted.transitioning ? 'Transition mix' : 'Steady plan'}:
				{observation.submitted.columns.length} participating column(s).
			</p>
		{:else}<p>No frame submission observed for the installed Matrix plan.</p>{/if}
		{#if observation.error}<p role="alert">{observation.error}</p>{/if}
	{:else}<p>No native Matrix observation. A launch request does not confirm rendering.</p>{/if}
	<small
		>Submitted-mix markers show renderer observations; display or receiver confirmation is separate.</small>
</section>

<style>
	h3 {
		font-size: 1rem;
	}
	p,
	small {
		font-size: 0.85rem;
	}
</style>
