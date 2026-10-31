<script lang="ts">
	import type { ApplicationRuntimeSnapshot } from '../media/generated/ApplicationRuntimeSnapshot';
	import type { ApplicationSnapshot } from '../media/generated/ApplicationSnapshot';
	import type { ApplicationPage } from '../media/generated/ApplicationPage';
	import { configurationLabel } from './runtime-presentation';
	let {
		snapshot,
		selected,
		catalog,
		live,
		loading,
		error,
		disabled,
		refresh
	}: {
		snapshot: ApplicationRuntimeSnapshot | null;
		selected: ApplicationSnapshot | null;
		catalog: ApplicationPage | null;
		live: boolean;
		loading: boolean;
		error: string;
		disabled: boolean;
		refresh(): unknown;
	} = $props();
</script>

<section aria-label="Application runtime">
	<header>
		<h3>Application runtime</h3>
		<button disabled={disabled || loading} onclick={refresh}>Refresh runtime status</button>
	</header>
	<p>
		{live ? 'Desktop updates' : 'Snapshot observations; refresh to update'} · Configuration editing and
		column launch are separate operations.
	</p>
	{#if error}<p role="alert">{error}</p>{/if}
	{#if snapshot}
		<small>Observation {snapshot.revision} · {snapshot.elapsed_ms} ms since runtime start</small>
		{#each snapshot.applications as application (application.application)}
			<article aria-label={`Runtime ${application.application}`}>
				<strong
					>{catalog?.applications.find((entry) => entry.id === application.application)?.label ??
						application.application}</strong>
				<dl>
					<dt>Phase</dt>
					<dd>{application.phase}</dd>
					<dt>Configuration</dt>
					<dd>
						{configurationLabel(
							application,
							selected?.id === application.application ? selected : null
						)}
					</dd>
					<dt>Process ID</dt>
					<dd>{application.process_id ?? '—'}</dd>
					<dt>Launch</dt>
					<dd>{application.launch ?? '—'}</dd>
					<dt>Failures</dt>
					<dd>{application.failures}</dd>
					<dt>Next action</dt>
					<dd>
						{application.next_action_ms === null
							? '—'
							: `${application.next_action_ms} ms since runtime start`}
					</dd>
					{#if application.exit_code !== null}<dt>Exit code</dt>
						<dd>{application.exit_code}</dd>{/if}
				</dl>
				{#if application.message}<p>{application.message}</p>{/if}
			</article>
		{:else}<p>No application process is admitted for this project.</p>{/each}
	{:else if loading}<p>Loading runtime observations…</p>{:else}<p>
			No current project observation.
		</p>{/if}
</section>

<style>
	section {
		margin: 1rem 0;
	}
	header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
	}
	article {
		border: 0.05rem solid currentColor;
		padding: 0.5rem;
		margin: 0.5rem 0;
	}
	dl {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 0.3rem 0.6rem;
	}
	dd {
		margin: 0;
		overflow-wrap: anywhere;
	}
	small {
		opacity: 0.8;
	}
</style>
